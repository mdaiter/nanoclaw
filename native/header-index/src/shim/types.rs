//! Shared types and type-mapping macros for shim generation.

use crate::kb::FrameworkKB;
use crate::protocol::{ProtocolEntry, ProtocolStep, StepRole};

// =============================================================================
// Generated shim output
// =============================================================================

/// Output from the shim generator: all three layers + consumers.
#[derive(Debug, Clone)]
pub struct GeneratedShim {
    pub rust_code: String,       // compilable Rust source
    pub c_header: String,        // C header for the exported API
    pub python_consumer: String, // Python ctypes wrapper
    pub bun_consumer: String,    // Bun FFI wrapper
    pub cargo_toml: String,      // Cargo.toml for the shim crate
    pub build_rs: String,        // build.rs for framework linking
    pub lib_name: String,        // e.g. "mcgyver_shim_mtlcodegen"
}

// =============================================================================
// Type-mapping macro + tables
// =============================================================================

/// Map a type string through a lookup table with a default fallback.
macro_rules! type_map {
  ($input:expr, $default:expr, $( $from:literal => $to:literal ),+ $(,)?) => {
    match $input.trim() { $( $from => $to, )+ _ => $default }
  };
}

/// Universal C → Rust type map. Framework-specific types (vImage_Error, OSStatus, etc.)
/// are resolved through `FrameworkKB::type_aliases` BEFORE reaching this function.
/// Only standard C/POSIX types and Rust primitives belong here.
pub(super) fn to_rust_type(c_type: &str) -> &'static str {
    type_map!(c_type, "*mut std::ffi::c_void",
      // void / pointers
      "void"                => "()",
      "void *"              => "*mut std::ffi::c_void",
      "const void *"        => "*const std::ffi::c_void",
      "c_void_p"            => "*mut std::ffi::c_void",
      "pointer"             => "*mut std::ffi::c_void",
      // integers
      "int"                 => "std::ffi::c_int",
      "c_int"               => "std::ffi::c_int",
      "unsigned int"        => "std::ffi::c_uint",
      "c_uint"              => "std::ffi::c_uint",
      "long"                => "std::ffi::c_long",
      "c_long"              => "std::ffi::c_long",
      "unsigned long"       => "std::ffi::c_ulong",
      "c_ulong"             => "std::ffi::c_ulong",
      "long long"           => "std::ffi::c_longlong",
      "c_longlong"          => "std::ffi::c_longlong",
      "unsigned long long"  => "std::ffi::c_ulonglong",
      "c_ulonglong"         => "std::ffi::c_ulonglong",
      "ssize_t"             => "isize",
      "size_t"              => "usize",
      "c_size_t"            => "usize",
      "uint8_t"             => "u8",
      "int8_t"              => "i8",
      "uint16_t"            => "u16",
      "int16_t"             => "i16",
      "uint32_t"            => "u32",
      "c_uint32"            => "u32",
      "int32_t"             => "i32",
      "uint64_t"            => "u64",
      "c_uint64"            => "u64",
      "int64_t"             => "i64",
      // Rust-native (used by seed protocols)
      "usize"               => "usize",
      "isize"               => "isize",
      "u8"                  => "u8",
      "i8"                  => "i8",
      "u16"                 => "u16",
      "i16"                 => "i16",
      "u32"                 => "u32",
      "i32"                 => "i32",
      "u64"                 => "u64",
      "i64"                 => "i64",
      // floats
      "float"               => "f32",
      "c_float"             => "f32",
      "f32"                 => "f32",
      "double"              => "f64",
      "c_double"            => "f64",
      "f64"                 => "f64",
      // booleans
      "bool"                => "bool",
      "c_bool"              => "bool",
      // strings
      "const char *"        => "*const std::ffi::c_char",
      "char *"              => "*mut std::ffi::c_char",
      "c_char_p"            => "*const std::ffi::c_char",
      // typed pointers
      "unsigned char *"     => "*mut u8",
      "const unsigned char *" => "*const u8",
      "const float *"       => "*const f32",
      "const double *"      => "*const f64",
      "float *"             => "*mut f32",
      "double *"            => "*mut f64",
      "const int16_t *"     => "*const i16",
      // ObjC runtime
      "id"                  => "*mut std::ffi::c_void",
      "SEL"                 => "*const std::ffi::c_void",
      "Class"               => "*const std::ffi::c_void",
    )
}

/// Universal C → Python ctypes map. Resolved after KB type_aliases.
pub(super) fn to_ctypes(c_type: &str) -> &'static str {
    type_map!(c_type, "ctypes.c_void_p",
      "void"                => "None",
      "void *"              => "ctypes.c_void_p",
      "const void *"        => "ctypes.c_void_p",
      "c_void_p"            => "ctypes.c_void_p",
      "pointer"             => "ctypes.c_void_p",
      "int"                 => "ctypes.c_int",
      "c_int"               => "ctypes.c_int",
      "unsigned int"        => "ctypes.c_uint",
      "c_uint"              => "ctypes.c_uint",
      "long"                => "ctypes.c_long",
      "c_long"              => "ctypes.c_long",
      "unsigned long"       => "ctypes.c_ulong",
      "c_ulong"             => "ctypes.c_ulong",
      "long long"           => "ctypes.c_longlong",
      "c_longlong"          => "ctypes.c_longlong",
      "unsigned long long"  => "ctypes.c_ulonglong",
      "c_ulonglong"         => "ctypes.c_ulonglong",
      "bool"                => "ctypes.c_bool",
      "c_bool"              => "ctypes.c_bool",
      "float"               => "ctypes.c_float",
      "c_float"             => "ctypes.c_float",
      "f32"                 => "ctypes.c_float",
      "double"              => "ctypes.c_double",
      "c_double"            => "ctypes.c_double",
      "f64"                 => "ctypes.c_double",
      "const char *"        => "ctypes.c_char_p",
      "char *"              => "ctypes.c_char_p",
      "c_char_p"            => "ctypes.c_char_p",
      "ssize_t"             => "ctypes.c_ssize_t",
      "size_t"              => "ctypes.c_size_t",
      "c_size_t"            => "ctypes.c_size_t",
      "usize"               => "ctypes.c_size_t",
      "isize"               => "ctypes.c_ssize_t",
      "uint8_t"             => "ctypes.c_uint8",
      "int8_t"              => "ctypes.c_int8",
      "uint16_t"            => "ctypes.c_uint16",
      "int16_t"             => "ctypes.c_int16",
      "uint32_t"            => "ctypes.c_uint32",
      "c_uint32"            => "ctypes.c_uint32",
      "u32"                 => "ctypes.c_uint32",
      "int32_t"             => "ctypes.c_int32",
      "i32"                 => "ctypes.c_int32",
      "uint64_t"            => "ctypes.c_uint64",
      "c_uint64"            => "ctypes.c_uint64",
      "u64"                 => "ctypes.c_uint64",
      "int64_t"             => "ctypes.c_int64",
      "i64"                 => "ctypes.c_int64",
      "u8"                  => "ctypes.c_uint8",
      "i8"                  => "ctypes.c_int8",
      "u16"                 => "ctypes.c_uint16",
      "i16"                 => "ctypes.c_int16",
      "unsigned char *"     => "ctypes.POINTER(ctypes.c_ubyte)",
      "const unsigned char *" => "ctypes.POINTER(ctypes.c_ubyte)",
      "const float *"       => "ctypes.POINTER(ctypes.c_float)",
      "const double *"      => "ctypes.POINTER(ctypes.c_double)",
      "float *"             => "ctypes.POINTER(ctypes.c_float)",
      "double *"            => "ctypes.POINTER(ctypes.c_double)",
      "const int16_t *"     => "ctypes.POINTER(ctypes.c_int16)",
    )
}

pub(super) fn to_bun_ffi(c_type: &str) -> &'static str {
    type_map!(c_type, "FFIType.ptr",
      "void"              => "FFIType.void",
      "void *"            => "FFIType.ptr",
      "c_void_p"          => "FFIType.ptr",
      "int"               => "FFIType.i32",
      "c_int"             => "FFIType.i32",
      "unsigned int"      => "FFIType.u32",
      "c_uint"            => "FFIType.u32",
      "long"              => "FFIType.i64",
      "c_long"            => "FFIType.i64",
      "unsigned long"     => "FFIType.u64",
      "c_ulong"           => "FFIType.u64",
      "long long"         => "FFIType.i64",
      "c_longlong"        => "FFIType.i64",
      "unsigned long long" => "FFIType.u64",
      "c_ulonglong"       => "FFIType.u64",
      "bool"              => "FFIType.bool",
      "c_bool"            => "FFIType.bool",
      "float"             => "FFIType.f32",
      "c_float"           => "FFIType.f32",
      "f32"               => "FFIType.f32",
      "double"            => "FFIType.f64",
      "c_double"          => "FFIType.f64",
      "f64"               => "FFIType.f64",
      "const char *"      => "FFIType.cstring",
      "char *"            => "FFIType.cstring",
      "c_char_p"          => "FFIType.cstring",
      "ssize_t"           => "FFIType.i64",
      "size_t"            => "FFIType.u64",
      "c_size_t"          => "FFIType.u64",
      "usize"             => "FFIType.u64",
      "isize"             => "FFIType.i64",
      "uint32_t"          => "FFIType.u32",
      "c_uint32"          => "FFIType.u32",
      "u32"               => "FFIType.u32",
      "uint64_t"          => "FFIType.u64",
      "c_uint64"          => "FFIType.u64",
      "u64"               => "FFIType.u64",
      "int32_t"           => "FFIType.i32",
      "i32"               => "FFIType.i32",
      "int64_t"           => "FFIType.i64",
      "i64"               => "FFIType.i64",
      "uint8_t"           => "FFIType.u8",
      "int8_t"            => "FFIType.i8",
      "uint16_t"          => "FFIType.u16",
      "int16_t"           => "FFIType.i16",
      "u8"                => "FFIType.u8",
      "i8"                => "FFIType.i8",
      "u16"               => "FFIType.u16",
      "i16"               => "FFIType.i16",
      "pointer"           => "FFIType.ptr",
    )
}

pub(super) fn to_c_type(type_str: &str) -> &'static str {
    type_map!(type_str, "void*",
      "void"              => "void",
      "void *"            => "void*",
      "c_void_p"          => "void*",
      "int"               => "int",
      "c_int"             => "int",
      "unsigned int"      => "unsigned int",
      "c_uint"            => "unsigned int",
      "long"              => "long",
      "c_long"            => "long",
      "unsigned long"     => "unsigned long",
      "c_ulong"           => "unsigned long",
      "long long"         => "long long",
      "c_longlong"        => "long long",
      "unsigned long long" => "unsigned long long",
      "c_ulonglong"       => "unsigned long long",
      "bool"              => "int",
      "c_bool"            => "int",
      "float"             => "float",
      "c_float"           => "float",
      "f32"               => "float",
      "double"            => "double",
      "c_double"          => "double",
      "f64"               => "double",
      "const char *"      => "const char*",
      "char *"            => "char*",
      "c_char_p"          => "const char*",
      "ssize_t"           => "ssize_t",
      "size_t"            => "size_t",
      "c_size_t"          => "size_t",
      "usize"             => "size_t",
      "isize"             => "ssize_t",
      "uint32_t"          => "uint32_t",
      "c_uint32"          => "uint32_t",
      "u32"               => "uint32_t",
      "uint64_t"          => "uint64_t",
      "c_uint64"          => "uint64_t",
      "u64"               => "uint64_t",
      "int32_t"           => "int32_t",
      "i32"               => "int32_t",
      "int64_t"           => "int64_t",
      "i64"               => "int64_t",
      "uint8_t"           => "uint8_t",
      "int8_t"            => "int8_t",
      "uint16_t"          => "uint16_t",
      "int16_t"           => "int16_t",
      "u8"                => "uint8_t",
      "i8"                => "int8_t",
      "u16"               => "uint16_t",
      "i16"               => "int16_t",
      "pointer"           => "void*",
    )
}

// =============================================================================
// KB-aware type resolution: alias → universal map
// =============================================================================

/// Resolve a type through the KB's aliases, then map to Rust.
/// This is the primary entry point for shim generators.
pub(super) fn resolve_rust_type<'a>(c_type: &'a str, kb: &'a FrameworkKB) -> &'static str {
    to_rust_type(kb.resolve_type(c_type))
}

/// Resolve a type through the KB's aliases, then map to Python ctypes.
pub(super) fn resolve_ctypes<'a>(c_type: &'a str, kb: &'a FrameworkKB) -> &'static str {
    to_ctypes(kb.resolve_type(c_type))
}

/// Resolve a type through the KB's aliases, then map to Bun FFI.
pub(super) fn resolve_bun_ffi<'a>(c_type: &'a str, kb: &'a FrameworkKB) -> &'static str {
    to_bun_ffi(kb.resolve_type(c_type))
}

/// Resolve a type through the KB's aliases, then map to C header.
pub(super) fn resolve_c_type<'a>(c_type: &'a str, kb: &'a FrameworkKB) -> &'static str {
    to_c_type(kb.resolve_type(c_type))
}

// =============================================================================
// Shared helpers
// =============================================================================

pub(super) fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut result = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            // Preserve existing underscores, avoid doubles
            if !result.ends_with('_') {
                result.push('_');
            }
            continue;
        }
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            // Insert _ before: uppercase after lowercase/digit, OR start of trailing
            // lowercase run in an uppercase sequence (e.g., "ARGB" stays "argb")
            if prev.is_lowercase() || prev.is_ascii_digit() {
                result.push('_');
            } else if prev.is_uppercase() || prev == '_' {
                // Check if next char is lowercase → acronym boundary (e.g., "SCreate" → "s_create")
                if chars.get(i + 1).map_or(false, |n| n.is_lowercase()) {
                    if !result.ends_with('_') {
                        result.push('_');
                    }
                }
            }
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// Derive method name from a step's function by stripping the group prefix.
pub(super) fn method_name(step: &ProtocolStep, group: &str) -> String {
    let raw = snake_case(&step.function.replace(group, ""));
    if raw.is_empty() || raw == "_" {
        "call".to_string()
    } else {
        raw.trim_start_matches('_').to_string()
    }
}

/// Parse args_hint into (name, type) pairs.
///
/// Handles two formats:
///   - C-style declarations: "const float *A, int M" → [("A", "const float *"), ("M", "int")]
///   - Simple labels: "src, dest, flags" → [("arg0", "void *"), ...]
pub(super) fn parse_step_args(step: &ProtocolStep, _kb: &FrameworkKB) -> Vec<(String, String)> {
    if step.args_hint.is_empty() {
        return vec![];
    }
    step.args_hint
        .split(',')
        .enumerate()
        .map(|(i, arg)| {
            let arg = arg.trim();
            // Try C-style "type name" parsing: last word is param name, rest is type
            if let Some((ty, name)) = try_parse_c_decl(arg) {
                (name, ty)
            } else {
                // Fallback: simple label inference
                let ty = infer_arg_type(arg);
                (format!("arg{i}"), ty.to_string())
            }
        })
        .collect()
}

/// Try to parse a C-style declaration like "const float *A" → ("const float *", "A").
fn try_parse_c_decl(arg: &str) -> Option<(String, String)> {
    let arg = arg.trim();
    // Must have at least 2 words to be a C declaration
    let parts: Vec<&str> = arg.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    // Last token is the param name (possibly with leading *)
    let last = *parts.last()?;
    let name = last.trim_start_matches('*');
    // Name must be a simple identifier (no punctuation, not a keyword)
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    // Type is everything except the name token, plus any leading * from the name token
    let star_prefix = &last[..last.len() - name.len()]; // e.g., "*" from "*A"
    let type_parts: Vec<&str> = parts[..parts.len() - 1].to_vec();
    let mut ty = type_parts.join(" ");
    if !star_prefix.is_empty() {
        ty.push(' ');
        ty.push_str(star_prefix);
    }
    let ty = ty.trim().to_string();
    if ty.is_empty() {
        return None;
    }
    Some((ty, name.to_string()))
}

/// Infer a simple type from an args_hint token (used for non-C-declaration args).
pub(super) fn infer_arg_type(arg: &str) -> &'static str {
    let arg = arg.trim();
    if arg.starts_with("b\"") || arg.starts_with("b'") {
        "const char *"
    } else if arg == "None" || arg == "null" || arg == "NULL" {
        "void *"
    } else if arg == "True" || arg == "False" {
        "bool"
    } else if arg.starts_with("0x") {
        "uint64_t"
    } else if arg.parse::<i64>().is_ok() {
        "int"
    } else if arg == "handle" {
        "void *"
    } else {
        "void *"
    }
}

/// Check if a param name represents the opaque handle.
pub(super) fn is_handle_param(name: &str, step: &ProtocolStep) -> bool {
    name == "handle" || {
        name.strip_prefix("arg")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|idx| step.args_hint.split(',').nth(idx))
            .map(|h| h.trim() == "handle")
            .unwrap_or(false)
    }
}

/// Iterate non-handle args in a step, yielding (index, arg_text).
pub(super) fn non_handle_args(step: &ProtocolStep) -> impl Iterator<Item = (usize, &str)> {
    step.args_hint
        .split(',')
        .enumerate()
        .filter(|(_, a)| a.trim() != "handle")
}

/// Check if a protocol has a create/init step.
pub(super) fn has_create(proto: &ProtocolEntry) -> bool {
    proto
        .steps
        .iter()
        .any(|s| matches!(s.role, StepRole::Create | StepRole::Init))
}

/// Check if a protocol has a destroy step.
pub(super) fn has_destroy(proto: &ProtocolEntry) -> bool {
    proto
        .steps
        .iter()
        .any(|s| matches!(s.role, StepRole::Destroy))
}

/// Get the create/init step from a protocol.
pub(super) fn create_step(proto: &ProtocolEntry) -> Option<&ProtocolStep> {
    proto
        .steps
        .iter()
        .find(|s| matches!(s.role, StepRole::Create | StepRole::Init))
}

/// Get the destroy step from a protocol.
pub(super) fn destroy_step(proto: &ProtocolEntry) -> Option<&ProtocolStep> {
    proto
        .steps
        .iter()
        .find(|s| matches!(s.role, StepRole::Destroy))
}

/// Get use/query steps from a protocol.
pub(super) fn use_steps(proto: &ProtocolEntry) -> impl Iterator<Item = &ProtocolStep> {
    proto
        .steps
        .iter()
        .filter(|s| matches!(s.role, StepRole::Use | StepRole::Query))
}

/// KB-aware handle type for a protocol's create step.
pub(super) fn handle_type_resolved(proto: &ProtocolEntry, kb: &FrameworkKB) -> &'static str {
    create_step(proto)
        .map(|s| resolve_rust_type(&s.returns, kb))
        .unwrap_or("*mut c_void")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mappings() {
        assert_eq!(to_rust_type("void *"), "*mut std::ffi::c_void");
        assert_eq!(to_rust_type("int"), "std::ffi::c_int");
        assert_eq!(to_rust_type("void"), "()");
        assert_eq!(to_rust_type("const char *"), "*const std::ffi::c_char");
        assert_eq!(to_ctypes("void *"), "ctypes.c_void_p");
        assert_eq!(to_ctypes("int"), "ctypes.c_int");
        assert_eq!(to_ctypes("void"), "None");
        assert_eq!(to_bun_ffi("void *"), "FFIType.ptr");
        assert_eq!(to_bun_ffi("int"), "FFIType.i32");
        assert_eq!(to_bun_ffi("void"), "FFIType.void");
    }

    #[test]
    fn test_native_rust_type_mappings() {
        // These are used by ProtocolEntry::stateless() in seed KBs
        assert_eq!(to_rust_type("i64"), "i64");
        assert_eq!(to_rust_type("i32"), "i32");
        assert_eq!(to_rust_type("u32"), "u32");
        assert_eq!(to_rust_type("u64"), "u64");
        assert_eq!(to_rust_type("f32"), "f32");
        assert_eq!(to_rust_type("f64"), "f64");
        assert_eq!(to_rust_type("u8"), "u8");
        assert_eq!(to_rust_type("usize"), "usize");
        assert_eq!(to_ctypes("i64"), "ctypes.c_int64");
        assert_eq!(to_ctypes("i32"), "ctypes.c_int32");
        assert_eq!(to_ctypes("u32"), "ctypes.c_uint32");
        assert_eq!(to_ctypes("f64"), "ctypes.c_double");
        assert_eq!(to_bun_ffi("i64"), "FFIType.i64");
        assert_eq!(to_bun_ffi("u32"), "FFIType.u32");
        assert_eq!(to_bun_ffi("f64"), "FFIType.f64");
        assert_eq!(to_c_type("i64"), "int64_t");
        assert_eq!(to_c_type("u32"), "uint32_t");
        assert_eq!(to_c_type("f64"), "double");
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(snake_case("MTLCodeGenService"), "mtl_code_gen_service");
        assert_eq!(snake_case("vImageScale"), "v_image_scale");
        assert_eq!(snake_case("vImageScale_ARGB8888"), "v_image_scale_argb8888");
        assert_eq!(snake_case("IOSurfaceCreate"), "io_surface_create");
        assert_eq!(snake_case("CC_SHA256"), "cc_sha256");
        assert_eq!(snake_case("cblas_sgemm"), "cblas_sgemm");
        assert_eq!(snake_case("CCHmac"), "cc_hmac");
    }

    #[test]
    fn test_kb_type_alias_resolution() {
        use crate::super::kb::FrameworkKB;
        let mut kb = FrameworkKB::new("Accelerate");
        kb.add_type_aliases(&[
            ("vImage_Error", "ssize_t"),
            ("vImage_Flags", "uint32_t"),
            ("CBLAS_ORDER", "int"),
        ]);
        // resolve_rust_type goes through alias first, then universal map
        assert_eq!(resolve_rust_type("vImage_Error", &kb), "isize");
        assert_eq!(resolve_rust_type("vImage_Flags", &kb), "u32");
        assert_eq!(resolve_rust_type("CBLAS_ORDER", &kb), "std::ffi::c_int");
        // ctypes resolution
        assert_eq!(resolve_ctypes("vImage_Error", &kb), "ctypes.c_ssize_t");
        assert_eq!(resolve_ctypes("CBLAS_ORDER", &kb), "ctypes.c_int");
        // Unknown types still fall through to void*
        assert_eq!(
            resolve_rust_type("WeirdCustomType", &kb),
            "*mut std::ffi::c_void"
        );
        // Standard types bypass alias (no alias needed)
        assert_eq!(resolve_rust_type("int", &kb), "std::ffi::c_int");
        assert_eq!(resolve_rust_type("void *", &kb), "*mut std::ffi::c_void");
    }

    #[test]
    fn test_long_type_mappings() {
        // to_rust_type
        assert_eq!(to_rust_type("long"), "std::ffi::c_long");
        assert_eq!(to_rust_type("c_long"), "std::ffi::c_long");
        assert_eq!(to_rust_type("unsigned long"), "std::ffi::c_ulong");
        assert_eq!(to_rust_type("c_ulong"), "std::ffi::c_ulong");
        assert_eq!(to_rust_type("long long"), "std::ffi::c_longlong");
        assert_eq!(to_rust_type("c_longlong"), "std::ffi::c_longlong");
        assert_eq!(to_rust_type("unsigned long long"), "std::ffi::c_ulonglong");
        assert_eq!(to_rust_type("c_ulonglong"), "std::ffi::c_ulonglong");
        // to_ctypes
        assert_eq!(to_ctypes("long"), "ctypes.c_long");
        assert_eq!(to_ctypes("c_long"), "ctypes.c_long");
        assert_eq!(to_ctypes("unsigned long"), "ctypes.c_ulong");
        assert_eq!(to_ctypes("c_ulong"), "ctypes.c_ulong");
        assert_eq!(to_ctypes("long long"), "ctypes.c_longlong");
        assert_eq!(to_ctypes("c_longlong"), "ctypes.c_longlong");
        assert_eq!(to_ctypes("unsigned long long"), "ctypes.c_ulonglong");
        assert_eq!(to_ctypes("c_ulonglong"), "ctypes.c_ulonglong");
        // to_bun_ffi
        assert_eq!(to_bun_ffi("long"), "FFIType.i64");
        assert_eq!(to_bun_ffi("unsigned long"), "FFIType.u64");
        assert_eq!(to_bun_ffi("long long"), "FFIType.i64");
        assert_eq!(to_bun_ffi("unsigned long long"), "FFIType.u64");
        // to_c_type
        assert_eq!(to_c_type("long"), "long");
        assert_eq!(to_c_type("unsigned long"), "unsigned long");
        assert_eq!(to_c_type("long long"), "long long");
        assert_eq!(to_c_type("unsigned long long"), "unsigned long long");
    }

    /// Verify that all types covered by to_rust_type also have mappings in the other 3 maps.
    /// This prevents the gap we had where long/unsigned long were only in to_rust_type.
    #[test]
    fn test_type_map_parity() {
        let types = &[
            "void",
            "void *",
            "const void *",
            "c_void_p",
            "pointer",
            "int",
            "c_int",
            "unsigned int",
            "c_uint",
            "long",
            "c_long",
            "unsigned long",
            "c_ulong",
            "long long",
            "c_longlong",
            "unsigned long long",
            "c_ulonglong",
            "ssize_t",
            "size_t",
            "usize",
            "isize",
            "uint8_t",
            "int8_t",
            "uint16_t",
            "int16_t",
            "uint32_t",
            "c_uint32",
            "int32_t",
            "uint64_t",
            "c_uint64",
            "int64_t",
            "u8",
            "i8",
            "u16",
            "i16",
            "u32",
            "i32",
            "u64",
            "i64",
            "float",
            "c_float",
            "f32",
            "double",
            "c_double",
            "f64",
            "bool",
            "c_bool",
            "const char *",
            "char *",
            "c_char_p",
        ];
        let defaults = [
            "*mut std::ffi::c_void", // rust default
            "ctypes.c_void_p",       // ctypes default
            "FFIType.ptr",           // bun default
            "void*",                 // c default
        ];
        for &ty in types {
            let rust = to_rust_type(ty);
            let ct = to_ctypes(ty);
            let bun = to_bun_ffi(ty);
            let c = to_c_type(ty);
            // None of them should hit the default fallback (meaning "not mapped")
            // Exception: pointer types that legitimately map to pointer defaults
            let is_ptr_type = ty.contains('*')
                || ty == "c_void_p"
                || ty == "pointer"
                || ty == "id"
                || ty == "SEL"
                || ty == "Class";
            if !is_ptr_type {
                assert_ne!(rust, defaults[0], "to_rust_type missing: {ty}");
                assert_ne!(ct, defaults[1], "to_ctypes missing: {ty}");
                assert_ne!(bun, defaults[2], "to_bun_ffi missing: {ty}");
                assert_ne!(c, defaults[3], "to_c_type missing: {ty}");
            }
        }
    }

    #[test]
    fn test_try_parse_c_decl() {
        assert_eq!(
            super::try_parse_c_decl("const float *A"),
            Some(("const float *".into(), "A".into()))
        );
        assert_eq!(
            super::try_parse_c_decl("int M"),
            Some(("int".into(), "M".into()))
        );
        assert_eq!(
            super::try_parse_c_decl("CBLAS_ORDER Order"),
            Some(("CBLAS_ORDER".into(), "Order".into()))
        );
        // Single word — not a C decl
        assert_eq!(super::try_parse_c_decl("handle"), None);
        assert_eq!(super::try_parse_c_decl(""), None);
    }
}
