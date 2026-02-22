//! Layer 2: Signature inference and capability detection.
//!
//! Two paths, in priority order:
//! 1. **Typed**: clang AST gave us a real `ParsedDecl` → use it (confidence 0.95)
//! 2. **Heuristic**: no typed data → infer from naming conventions (confidence 0.1–0.9)
//!
//! Ported from python/mcgyver/header_index/private.py (_infer_signature)
//! and python/mcgyver/header_index/query.py (CAPABILITY_PATTERNS).

use crate::extract::{ParsedDecl, RawExport};

/// An inferred function signature.
#[derive(Debug, Clone)]
pub struct InferredSig {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<InferredParam>,
    pub confidence: f64, // 0.0–1.0, how sure we are about the *shape* (pattern + params)
    pub return_typed: bool, // true = return type from clang/probe, false = guessed from naming
    pub pattern: &'static str, // which pattern matched
}

#[derive(Debug, Clone)]
pub struct InferredParam {
    pub name: String,
    pub type_name: String,
    pub is_pointer: bool,
}

impl InferredSig {
    /// Render as a C-style function signature.
    /// Guessed return types are suffixed with `/*?*/` so downstream consumers know.
    pub fn to_c_signature(&self) -> String {
        let params: Vec<String> = self
            .parameters
            .iter()
            .map(|p| {
                if p.is_pointer {
                    format!("{} *{}", p.type_name, p.name)
                } else {
                    format!("{} {}", p.type_name, p.name)
                }
            })
            .collect();
        let params_str = if params.is_empty() {
            "void".into()
        } else {
            params.join(", ")
        };
        let ret = if self.return_typed || self.confidence >= 0.80 {
            self.return_type.clone()
        } else {
            format!("{}/*?*/", self.return_type)
        };
        format!("{} {}({})", ret, self.name, params_str)
    }

    /// Build from a real clang ParsedDecl — typed, not guessed.
    pub fn from_parsed(decl: &ParsedDecl) -> Self {
        Self {
            name: decl.name.clone(),
            return_type: decl.return_type.clone(),
            parameters: decl
                .parameters
                .iter()
                .map(|(pname, ptype)| InferredParam {
                    name: if pname.is_empty() {
                        format!("arg{}", 0)
                    } else {
                        pname.clone()
                    },
                    type_name: ptype.replace(" *", "").replace('*', ""),
                    is_pointer: ptype.contains('*'),
                })
                .collect(),
            confidence: 0.95,
            return_typed: true,
            pattern: "clang_ast",
        }
    }
}

// =============================================================================
// Signature inference: typed first, heuristic fallback
// =============================================================================

/// Infer a function's signature.
/// If a `ParsedDecl` from clang is available, use it — real types beat guesswork.
/// Otherwise, fall back to naming-convention heuristics.
pub fn infer_signature(name: &str) -> InferredSig {
    infer_signature_with(name, None)
}

/// Infer with optional typed declaration from clang AST.
pub fn infer_signature_with(name: &str, decl: Option<&ParsedDecl>) -> InferredSig {
    // Path 1: real typed data from clang
    if let Some(d) = decl {
        return InferredSig::from_parsed(d);
    }

    // Path 2: heuristic from naming conventions
    let (prefix, suffix) = extract_type_prefix(name);
    let ref_type = if prefix.is_empty() {
        "void *".to_string()
    } else {
        format!("{}Ref", prefix)
    };

    // Try each pattern
    if let Some(sig) = match_suffix(name, &prefix, &ref_type, &suffix) {
        return sig;
    }

    // Fallback: unknown signature
    InferredSig {
        name: name.into(),
        return_type: "void".into(),
        parameters: vec![],
        confidence: 0.1,
        return_typed: false,
        pattern: "unknown",
    }
}

// =============================================================================
// Pattern table: suffix → (return_type, params_shape, confidence, pattern_name)
//
// Confidence values:
//   0.90  Destroy/Retain — verb is unambiguous, signature is near-certain
//   0.85  Create/Predicate — very likely but args might differ
//   0.80  Count — return type might be size_t or int, not always uint32_t
//   0.75  Submit/Copy — usually returns int/ref but some return void
//   0.70  Get/Set/Alloc/Sync — return type is a guess from property name keywords
//   0.60  Init/Callback — arg list shape varies widely
//   0.10  unknown — no pattern matched
// =============================================================================

/// Parameter shapes used by the pattern table.
enum Params<'a> {
    None,                 // ()
    MaybeRef,             // (ref) if prefix exists, else ()
    Ref,                  // (ref)
    RefAndValue(&'a str), // (ref, value) — Set pattern
    BufAndSize,           // (buf*, size) — Init pattern
    SizeOnly,             // (size) — Alloc pattern
    Callback,             // (ref, callback, context) — Register/Handler pattern
}

/// Build the parameter list from a shape.
fn build_params(shape: &Params, ref_type: &str, prefix: &str) -> Vec<InferredParam> {
    match shape {
        Params::None => vec![],
        Params::MaybeRef => {
            if prefix.is_empty() {
                vec![]
            } else {
                vec![param(ref_type, "ref", true)]
            }
        }
        Params::Ref => vec![param(ref_type, "ref", true)],
        Params::RefAndValue(vt) => vec![
            param(ref_type, "ref", true),
            param(vt, "value", vt.contains('*')),
        ],
        Params::BufAndSize => vec![param("void", "buf", true), param("size_t", "size", false)],
        Params::SizeOnly => vec![param("size_t", "size", false)],
        Params::Callback => vec![
            param(ref_type, "ref", true),
            param("void (*)(void *)", "callback", false),
            param("void", "context", true),
        ],
    }
}

/// Infer type from a Get/Set property name (e.g. "GetBufferCount" → "Count" → "uint32_t").
macro_rules! prop_type_rules {
  ($prop:expr, $( [ $( $kw:literal ),+ ] => $ty:literal ),+ , _ => $default:literal $(,)?) => {
    $( if $( $prop.contains($kw) )||+ { $ty } else )+ { $default }
  };
}

fn infer_property_type(prop: &str) -> &'static str {
    prop_type_rules!(prop,
      ["Count", "Size", "Length", "Index"] => "uint32_t",
      ["Name", "String", "Path"]          => "const char *",
      ["ID", "Id"]                        => "uint64_t",
      ["Float", "Scale", "Alpha"]         => "double",
      ["Bool", "Enabled", "Valid"]        => "bool",
      _ => "void *",
    )
}

fn match_suffix(name: &str, prefix: &str, ref_type: &str, suffix: &str) -> Option<InferredSig> {
    let s = suffix;

    // Get/Set need special handling: return type depends on property name
    if s.starts_with("Get") {
        let ret = infer_property_type(&s[3..]);
        return Some(InferredSig {
            name: name.into(),
            return_type: ret.into(),
            parameters: build_params(&Params::MaybeRef, ref_type, prefix),
            confidence: 0.70,
            return_typed: false,
            pattern: "Get",
        });
    }
    if s.starts_with("Set") {
        let val_type = infer_property_type(&s[3..]);
        return Some(InferredSig {
            name: name.into(),
            return_type: "void".into(),
            parameters: build_params(&Params::RefAndValue(val_type), ref_type, prefix),
            confidence: 0.70,
            return_typed: false,
            pattern: "Set",
        });
    }

    // Table-driven patterns: (match_fn, return_type, params, confidence, name)
    // "ref" and "Ref" in return_type are replaced with the actual ref_type at the end
    struct Rule {
        matches: fn(&str) -> bool,
        ret: &'static str,
        params: fn() -> Params<'static>,
        conf: f64,
        pattern: &'static str,
    }
    let rules: &[Rule] = &[
        // confidence  pattern        verb match                                                            return     params
        Rule {
            conf: 0.90,
            pattern: "Destroy",
            matches: |s| matches!(s, "Destroy" | "Release" | "Close" | "Free"),
            ret: "void",
            params: || Params::Ref,
        },
        Rule {
            conf: 0.90,
            pattern: "Retain",
            matches: |s| s == "Retain",
            ret: "Ref",
            params: || Params::Ref,
        },
        Rule {
            conf: 0.85,
            pattern: "Create",
            matches: |s| s == "Create" || s == "Open" || s.starts_with("Create"),
            ret: "Ref",
            params: || Params::None,
        },
        Rule {
            conf: 0.85,
            pattern: "Predicate",
            matches: |s| s.starts_with("Is") || s.starts_with("Has") || s.starts_with("Can"),
            ret: "bool",
            params: || Params::MaybeRef,
        },
        Rule {
            conf: 0.80,
            pattern: "Count",
            matches: |s| s == "Count" || s.ends_with("Count"),
            ret: "uint32_t",
            params: || Params::MaybeRef,
        },
        Rule {
            conf: 0.75,
            pattern: "Submit",
            matches: |s| matches!(s, "Flush" | "Finish" | "Submit" | "Commit"),
            ret: "int",
            params: || Params::MaybeRef,
        },
        Rule {
            conf: 0.75,
            pattern: "Copy",
            matches: |s| s.starts_with("Copy"),
            ret: "Ref",
            params: || Params::Ref,
        },
        Rule {
            conf: 0.70,
            pattern: "Allocate",
            matches: |s| s.starts_with("Alloc") || s.starts_with("Request"),
            ret: "void *",
            params: || Params::SizeOnly,
        },
        Rule {
            conf: 0.70,
            pattern: "Synchronization",
            matches: |s| matches!(s, "Signal" | "Wait" | "Lock" | "Unlock"),
            ret: "int",
            params: || Params::MaybeRef,
        },
        Rule {
            conf: 0.60,
            pattern: "Initialize",
            matches: |s| s == "Initialize" || s.starts_with("Init"),
            ret: "int",
            params: || Params::BufAndSize,
        },
        Rule {
            conf: 0.60,
            pattern: "Callback",
            matches: |s| {
                s.ends_with("Callback") || s.ends_with("Handler") || s.starts_with("Register")
            },
            ret: "void",
            params: || Params::Callback,
        },
    ];

    for rule in rules {
        if (rule.matches)(s) {
            let return_type = if rule.ret == "Ref" {
                ref_type.to_string()
            } else {
                rule.ret.to_string()
            };
            return Some(InferredSig {
                name: name.into(),
                return_type,
                parameters: build_params(&(rule.params)(), ref_type, prefix),
                confidence: rule.conf,
                return_typed: false,
                pattern: rule.pattern,
            });
        }
    }

    None
}

/// Extract a type prefix from a function name.
/// E.g., "IOAccelCLContextCreate" → ("IOAccelCLContext", "Create")
/// E.g., "SLSMainConnectionID" → ("SLS", "MainConnectionID")
fn extract_type_prefix(name: &str) -> (String, String) {
    // Look for the last transition from uppercase run to Uppercase-then-lowercase
    // that corresponds to a known verb
    let verbs = [
        "Create",
        "Destroy",
        "Release",
        "Retain",
        "Open",
        "Close",
        "Free",
        "Get",
        "Set",
        "Is",
        "Has",
        "Can",
        "Count",
        "Initialize",
        "Init",
        "Flush",
        "Finish",
        "Submit",
        "Commit",
        "Allocate",
        "Alloc",
        "Request",
        "Copy",
        "Signal",
        "Wait",
        "Lock",
        "Unlock",
        "Register",
    ];

    for verb in verbs {
        if let Some(pos) = name.find(verb) {
            if pos > 0 {
                return (name[..pos].to_string(), name[pos..].to_string());
            }
        }
    }

    // Fallback: find a 2-4 char uppercase prefix (SLS, ANE, MTL, IO, etc.)
    let chars: Vec<char> = name.chars().collect();
    for i in 2..chars.len().min(6) {
        if chars[i].is_lowercase() && chars[i - 1].is_uppercase() {
            // Found the boundary: ABCfoo → prefix="ABC" (not counting the lowercase)
            // But we want the full camelCase token, so go back to the last uppercase run start
            return (name[..i - 1].to_string(), name[i - 1..].to_string());
        }
    }

    (String::new(), name.to_string())
}

fn param(type_name: &str, name: &str, is_pointer: bool) -> InferredParam {
    InferredParam {
        name: name.into(),
        type_name: type_name.into(),
        is_pointer,
    }
}

// =============================================================================
// Capability inference from function names
// =============================================================================

/// Define an enum with an `as_str()` method mapping each variant to a snake_case string.
macro_rules! enum_str {
  ($name:ident { $( $variant:ident => $s:literal ),+ $(,)? }) => {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum $name { $( $variant ),+ }
    impl $name {
      pub fn as_str(&self) -> &'static str {
        match self { $( Self::$variant => $s ),+ }
      }
    }
  };
}

// Capability tag that can be inferred from a function name.
enum_str!(CapabilityTag {
  ImageProcessing    => "image_processing",
  LinearAlgebra      => "linear_algebra",
  MemoryBuffer       => "memory_buffer",
  GpuCompute         => "gpu_compute",
  VideoProcessing    => "video_processing",
  NeuralNetwork      => "neural_network",
  SystemIO           => "system_io",
  DisplayCompositing => "display_compositing",
  Synchronization    => "synchronization",
  Compression        => "compression",
  Cryptography       => "cryptography",
});

/// Match a lowercased name against keyword lists; push matching tags.
macro_rules! capability_rules {
  ($lower:expr, $caps:expr, $( $tag:expr => [ $( $kw:literal ),+ ] ),+ $(,)?) => {
    $( if $( $lower.contains($kw) )||+ { $caps.push($tag); } )+
  };
}

/// Infer capability tags from a function name.
pub fn infer_capabilities(name: &str) -> Vec<CapabilityTag> {
    let lower = name.to_lowercase();
    let mut caps = Vec::new();
    capability_rules!(lower, caps,
      CapabilityTag::ImageProcessing    => ["image", "vimage", "pixel", "bitmap", "argb", "convolv",
                                            "scale", "rotate", "histogram", "morpholog", "alpha"],
      CapabilityTag::LinearAlgebra      => ["blas", "lapack", "veclib", "bnns", "matrix", "gemm",
                                            "sparse", "eigenvector", "fft"],
      CapabilityTag::MemoryBuffer       => ["buffer", "iosurface", "alloc", "mmap", "memory", "zerocopy"],
      CapabilityTag::GpuCompute         => ["metal", "mtl", "gpu", "shader", "compute", "kernel",
                                             "command", "render", "pipeline", "mps"],
      CapabilityTag::VideoProcessing    => ["video", "vtdecomp", "vtcomp", "vtencode", "vtdecode",
                                            "h264", "hevc", "codec", "frame"],
      CapabilityTag::NeuralNetwork      => ["neural", "ane", "coreml", "bnns", "espresso",
                                            "inference", "convolution", "mlmodel"],
      CapabilityTag::SystemIO           => ["iokit", "ioservice", "ioconnect", "ioctl", "mach_port", "dispatch"],
      CapabilityTag::DisplayCompositing => ["sls", "skylight", "display", "window", "screen", "composit"],
      CapabilityTag::Synchronization    => ["fence", "semaphore", "mutex", "barrier", "signal", "wait"],
      CapabilityTag::Compression        => ["compress", "lzfse", "lz4", "zlib", "brotli"],
      CapabilityTag::Cryptography       => ["crypt", "aes", "sha", "hmac", "digest", "cipher"],
    );
    caps
}

// =============================================================================
// Batch inference
// =============================================================================

/// Infer signatures for all callable exports (C, C++, ObjC methods, Swift functions),
/// using typed declarations where available.
pub fn infer_all(
    exports: &[RawExport],
    decls: &std::collections::HashMap<String, ParsedDecl>,
) -> Vec<InferredSig> {
    use crate::extract::ExportKind;
    exports
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                ExportKind::Function
                    | ExportKind::CppFunction
                    | ExportKind::CppMethod
                    | ExportKind::SwiftFunction
            )
        })
        .map(|e| infer_signature_with(&e.name, decls.get(&e.name)))
        .collect()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_infer_create() {
        let sig = infer_signature("IOAccelCLContextCreate");
        assert_eq!(sig.return_type, "IOAccelCLContextRef");
        assert_eq!(sig.confidence, 0.85);
        assert_eq!(sig.pattern, "Create");
    }

    #[test]
    fn test_infer_destroy() {
        let sig = infer_signature("IOAccelCLContextDestroy");
        assert_eq!(sig.return_type, "void");
        assert_eq!(sig.parameters.len(), 1);
        assert!(sig.parameters[0].type_name.contains("IOAccelCLContext"));
        assert_eq!(sig.pattern, "Destroy");
    }

    #[test]
    fn test_infer_get_count() {
        let sig = infer_signature("MTLDeviceGetMaxBufferCount");
        assert_eq!(sig.return_type, "uint32_t");
        assert_eq!(sig.pattern, "Get");
    }

    #[test]
    fn test_infer_set() {
        let sig = infer_signature("SLSSetWindowAlpha");
        assert_eq!(sig.return_type, "void");
        assert_eq!(sig.parameters.len(), 2);
        assert_eq!(sig.pattern, "Set");
    }

    #[test]
    fn test_infer_predicate() {
        let sig = infer_signature("IOSurfaceIsInUse");
        assert_eq!(sig.return_type, "bool");
        assert_eq!(sig.pattern, "Predicate");
    }

    #[test]
    fn test_infer_unknown() {
        let sig = infer_signature("xyzzy");
        assert_eq!(sig.confidence, 0.1);
        assert_eq!(sig.pattern, "unknown");
    }

    #[test]
    fn test_infer_from_parsed_decl() {
        use crate::extract::{DeclKind, ParsedDecl};
        let decl = ParsedDecl {
            name: "vImageScale_ARGB8888".into(),
            kind: DeclKind::Function,
            return_type: "vImage_Error".into(),
            parameters: vec![
                ("src".into(), "const vImage_Buffer *".into()),
                ("dest".into(), "vImage_Buffer *".into()),
                ("flags".into(), "vImage_Flags".into()),
            ],
            is_variadic: false,
        };
        let sig = infer_signature_with("vImageScale_ARGB8888", Some(&decl));
        assert_eq!(sig.return_type, "vImage_Error");
        assert_eq!(sig.parameters.len(), 3);
        assert!(sig.parameters[0].is_pointer);
        assert!(!sig.parameters[2].is_pointer);
        assert_eq!(sig.confidence, 0.95);
        assert_eq!(sig.pattern, "clang_ast");
    }

    #[test]
    fn test_typed_beats_heuristic() {
        use crate::extract::{DeclKind, ParsedDecl};
        // "FooCreate" would match the Create heuristic, but clang says it returns int
        let decl = ParsedDecl {
            name: "FooCreate".into(),
            kind: DeclKind::Function,
            return_type: "int".into(),
            parameters: vec![("ctx".into(), "void *".into())],
            is_variadic: false,
        };
        let sig = infer_signature_with("FooCreate", Some(&decl));
        assert_eq!(sig.return_type, "int"); // clang wins, not "FooRef"
        assert_eq!(sig.pattern, "clang_ast");
    }

    #[test]
    fn test_extract_type_prefix() {
        assert_eq!(
            extract_type_prefix("IOAccelCLContextCreate"),
            ("IOAccelCLContext".into(), "Create".into())
        );
        assert_eq!(
            extract_type_prefix("SLSSetWindowAlpha"),
            ("SLS".into(), "SetWindowAlpha".into())
        );
    }

    #[test]
    fn test_infer_capabilities() {
        let caps = infer_capabilities("vImageScale_ARGB8888");
        assert!(caps.contains(&CapabilityTag::ImageProcessing));

        let caps = infer_capabilities("MTLCreateSystemDefaultDevice");
        assert!(caps.contains(&CapabilityTag::GpuCompute));

        let caps = infer_capabilities("ANECompilerInitialize");
        assert!(caps.contains(&CapabilityTag::NeuralNetwork));
    }
}
