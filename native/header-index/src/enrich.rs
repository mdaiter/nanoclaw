//! Layer 2b: Binary enrichment — extract semantic metadata from Mach-O sections.
//!
//! Reads ObjC and Swift metadata directly from the binary, no LLM calls:
//!
//!   ObjC:  __objc_classlist → class hierarchy, method signatures, protocol conformances
//!   Swift: __swift5_types / __swift5_proto → type descriptors, protocol conformances
//!   Strings: __cstring → error messages, format strings → semantic hints
//!   Deps:  LC_LOAD_DYLIB → framework dependency graph
//!
//! All extraction is from the binary itself — zero hardcoded paths.

use std::collections::HashMap;
use std::path::Path;

// =============================================================================
// Enriched metadata types
// =============================================================================

/// ObjC type encoding character → C type.
/// See: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjCRuntimeGuide/Articles/ocrtTypeEncodings.html
macro_rules! type_encodings {
  ($($ch:literal => $ty:literal),+ $(,)?) => {
    fn decode_type_char(c: u8) -> &'static str {
      match c { $( $ch => $ty, )+ _ => "void *" }
    }
  };
}

type_encodings!(
  b'c' => "char",
  b'i' => "int",
  b's' => "short",
  b'l' => "long",
  b'q' => "long long",
  b'C' => "unsigned char",
  b'I' => "unsigned int",
  b'S' => "unsigned short",
  b'L' => "unsigned long",
  b'Q' => "unsigned long long",
  b'f' => "float",
  b'd' => "double",
  b'B' => "bool",
  b'v' => "void",
  b'*' => "char *",
  b'@' => "id",
  b'#' => "Class",
  b':' => "SEL",
  b'^' => "void *",
  b'?' => "void (*)()",
);

/// Verb extraction patterns: keyword → semantic verb.
/// Scans the lowercased name for verb keywords. Order matters — first match wins.
macro_rules! verb_patterns {
  ($name:expr, $( $keyword:literal => $verb:literal ),+ $(,)?) => {
    $( if $name.contains($keyword) { return $verb; } )+
  };
}

/// Extract the semantic verb from a function/method name.
pub fn extract_verb(name: &str) -> &'static str {
    // Strip ObjC prefix like -[Class method] → method
    let clean = if let Some(bracket) = name.find(']') {
        let space = name.rfind(' ').unwrap_or(0);
        &name[space + 1..bracket]
    } else {
        name
    };

    // Split camelCase/PascalCase into words, check each for verb match
    let words = split_camel_case(clean);
    let lower_words: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();

    // Check individual words first (most precise)
    for w in &lower_words {
        let v = match_verb_word(w);
        if v != "unknown" {
            return v;
        }
    }

    // Fallback: check if any word starts with a verb prefix
    let joined = lower_words.join("");
    verb_patterns!(joined,
      "decompress"  => "decode",
      "deserialize" => "decode",
      "compress"    => "encode",
      "serialize"   => "encode",
      "unregister"  => "observe",
    );
    "unknown"
}

/// Match a single lowercase word against the verb table (exact match).
macro_rules! verb_exact {
  ($name:expr, $( $keyword:literal => $verb:literal ),+ $(,)?) => {
    $( if $name == $keyword { return $verb; } )+
  };
}

fn match_verb_word(word: &str) -> &'static str {
    verb_exact!(word,
      "create"    => "create",
      "init"      => "create",
      "make"      => "create",
      "new"       => "create",
      "open"      => "create",
      "alloc"     => "allocate",
      "allocate"  => "allocate",
      "destroy"   => "destroy",
      "release"   => "destroy",
      "dealloc"   => "destroy",
      "free"      => "destroy",
      "close"     => "destroy",
      "delete"    => "destroy",
      "get"       => "read",
      "fetch"     => "read",
      "load"      => "read",
      "read"      => "read",
      "copy"      => "read",
      "query"     => "read",
      "set"       => "write",
      "write"     => "write",
      "update"    => "write",
      "put"       => "write",
      "store"     => "write",
      "configure" => "write",
      "encode"    => "encode",
      "compress"  => "encode",
      "serialize" => "encode",
      "decode"    => "decode",
      "decompress" => "decode",
      "deserialize" => "decode",
      "parse"     => "decode",
      "scale"     => "transform",
      "resize"    => "transform",
      "rotate"    => "transform",
      "convert"   => "transform",
      "transform" => "transform",
      "convolve"  => "transform",
      "process"   => "transform",
      "render"    => "transform",
      "submit"    => "execute",
      "commit"    => "execute",
      "flush"     => "execute",
      "finish"    => "execute",
      "dispatch"  => "execute",
      "execute"   => "execute",
      "run"       => "execute",
      "perform"   => "execute",
      "wait"      => "synchronize",
      "signal"    => "synchronize",
      "lock"      => "synchronize",
      "unlock"    => "synchronize",
      "is"        => "predicate",
      "has"       => "predicate",
      "can"       => "predicate",
      "should"    => "predicate",
      "validate"  => "predicate",
      "check"     => "predicate",
      "count"     => "measure",
      "size"      => "measure",
      "length"    => "measure",
      "register"  => "observe",
      "add"       => "observe",
      "remove"    => "observe",
      "unregister" => "observe",
    );
    "unknown"
}

/// Split a camelCase or PascalCase name into words.
/// "VTDecompressionSessionDecodeFrame" → ["VT", "Decompression", "Session", "Decode", "Frame"]
/// "vImageScale_ARGB8888" → ["v", "Image", "Scale", "ARGB8888"]
fn split_camel_case(name: &str) -> Vec<String> {
    // Handle underscore-separated first
    if name.contains('_') {
        return name
            .split('_')
            .flat_map(|part| split_camel_case_inner(part))
            .collect();
    }
    split_camel_case_inner(name)
}

fn split_camel_case_inner(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let chars: Vec<char> = name.chars().collect();
    let mut start = 0;

    for i in 1..chars.len() {
        let prev_upper = chars[i - 1].is_uppercase();
        let curr_upper = chars[i].is_uppercase();
        let curr_lower = chars[i].is_lowercase();

        // Split at: lowercase→uppercase (camelCase) or uppercase→uppercase+lowercase (IOSurface → IO + Surface)
        let split = (!prev_upper && curr_upper)
            || (prev_upper && curr_upper && i + 1 < chars.len() && chars[i + 1].is_lowercase())
            || (prev_upper && curr_lower && i - start > 1);

        if split {
            let word: String = chars[start..i].iter().collect();
            if !word.is_empty() {
                words.push(word);
            }
            start = i;
        }
    }
    let last: String = chars[start..].iter().collect();
    if !last.is_empty() {
        words.push(last);
    }
    words
}

// =============================================================================
// ObjC runtime metadata from Mach-O sections
// =============================================================================

/// An ObjC class extracted from binary metadata.
#[derive(Debug, Clone, Default)]
pub struct ObjCClassInfo {
    pub name: String,
    pub superclass: String,     // empty if root (NSObject)
    pub protocols: Vec<String>, // protocol conformances
    pub instance_methods: Vec<ObjCMethodInfo>,
    pub class_methods: Vec<ObjCMethodInfo>,
    pub properties: Vec<ObjCPropertyInfo>,
    pub ivars: Vec<String>,
}

/// An ObjC method extracted from binary metadata.
#[derive(Debug, Clone)]
pub struct ObjCMethodInfo {
    pub selector: String,
    pub type_encoding: String,    // raw ObjC type encoding
    pub return_type: String,      // decoded C type
    pub param_types: Vec<String>, // decoded C types
    pub is_class_method: bool,
}

/// An ObjC property.
#[derive(Debug, Clone)]
pub struct ObjCPropertyInfo {
    pub name: String,
    pub type_encoding: String,
    pub attributes: String, // raw attributes string (T@"NSString",R,N)
}

/// Parse ObjC type encoding string into (return_type, [param_types]).
/// Format: return_type + frame_size + (param_type + offset)*
/// Example: "v24@0:8@16" → void, [id, SEL, id]
pub fn decode_objc_type_encoding(encoding: &str) -> (String, Vec<String>) {
    let bytes = encoding.as_bytes();
    if bytes.is_empty() {
        return ("void".into(), vec![]);
    }

    let mut types = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // Skip digits (stack offsets/sizes)
            b'0'..=b'9' => {
                i += 1;
            }
            // Pointer prefix — peek at next
            b'^' => {
                i += 1;
                if i < bytes.len() {
                    let base = decode_type_char(bytes[i]);
                    types.push(format!("{base} *"));
                    i += 1;
                }
            }
            // Object with class: @"ClassName"
            b'@' if i + 1 < bytes.len() && bytes[i + 1] == b'"' => {
                i += 2; // skip @"
                let start = i;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
                let class: String = bytes[start..i].iter().map(|&b| b as char).collect();
                types.push(format!("{class} *"));
                if i < bytes.len() {
                    i += 1;
                } // skip closing "
            }
            // Struct: {name=fields}
            b'{' => {
                let start = i + 1;
                let mut depth = 1;
                i += 1;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == b'{' {
                        depth += 1;
                    }
                    if bytes[i] == b'}' {
                        depth -= 1;
                    }
                    i += 1;
                }
                let inner: String = bytes[start..i.saturating_sub(1)]
                    .iter()
                    .map(|&b| b as char)
                    .collect();
                let struct_name = inner.split('=').next().unwrap_or("?");
                types.push(format!("struct {struct_name}"));
            }
            // Block: @? or just ?
            b'?' => {
                types.push("void (^)()".into());
                i += 1;
            }
            // Simple type
            ch => {
                types.push(decode_type_char(ch).into());
                i += 1;
            }
        }
    }

    let return_type = types.first().cloned().unwrap_or_else(|| "void".into());
    // params[0] is return, params[1] is self(id), params[2] is _cmd(SEL), rest are real params
    let params = if types.len() > 3 {
        types[3..].to_vec()
    } else {
        vec![]
    };
    (return_type, params)
}

/// Decode an ObjC property attributes string to extract the type.
/// Format: T@"NSString",R,N → type = "NSString *"
/// Format: Ti,N → type = "int"
pub fn decode_property_type(attributes: &str) -> String {
    let type_part = attributes.split(',').next().unwrap_or("");
    if let Some(rest) = type_part.strip_prefix("T@\"") {
        rest.strip_suffix('"').unwrap_or(rest).to_string() + " *"
    } else if let Some(rest) = type_part.strip_prefix('T') {
        if rest.is_empty() {
            "id".into()
        } else {
            decode_type_char(rest.as_bytes()[0]).into()
        }
    } else {
        "id".into()
    }
}

// =============================================================================
// Swift metadata types
// =============================================================================

/// A Swift type extracted from binary metadata.
#[derive(Debug, Clone)]
pub struct SwiftTypeInfo {
    pub name: String,
    pub mangled_name: String,
    pub kind: SwiftTypeKind,
    pub module: String,
    pub superclass: String,     // for classes
    pub protocols: Vec<String>, // protocol conformances
    pub fields: Vec<SwiftFieldInfo>,
    pub methods: Vec<String>, // demangled method names
}

/// Swift type classification from mangling.
macro_rules! swift_type_kinds {
  ($( $variant:ident => $s:literal ),+ $(,)?) => {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SwiftTypeKind { $( $variant ),+ }
    impl SwiftTypeKind {
      pub fn as_str(&self) -> &'static str { match self { $( Self::$variant => $s ),+ } }
    }
  };
}

swift_type_kinds!(
  Class       => "class",
  Struct      => "struct",
  Enum        => "enum",
  Protocol    => "protocol",
  Extension   => "extension",
  Unknown     => "unknown",
);

/// A Swift field/property.
#[derive(Debug, Clone)]
pub struct SwiftFieldInfo {
    pub name: String,
    pub type_name: String,
    pub is_var: bool, // var vs let
}

// =============================================================================
// Mach-O section reading (using `object` crate)
// =============================================================================

/// All enrichment data extracted from a single binary.
#[derive(Debug, Clone, Default)]
pub struct BinaryEnrichment {
    pub objc_classes: Vec<ObjCClassInfo>,
    pub swift_types: Vec<SwiftTypeInfo>,
    pub string_constants: Vec<String>,             // from __cstring
    pub framework_deps: Vec<String>,               // from LC_LOAD_DYLIB
    pub objc_protocols: Vec<String>,               // from __objc_protolist
    pub swift_conformances: Vec<(String, String)>, // (type, protocol)
}

/// Section name constants for Mach-O.
/// All known sections are listed here for documentation, even if not yet used.
macro_rules! sections {
  ($( $name:ident = ($seg:literal, $sect:literal) ),+ $(,)?) => {
    $( #[allow(dead_code)] const $name: (&str, &str) = ($seg, $sect); )+
  };
}

sections!(
    OBJC_CLASSLIST = ("__DATA_CONST", "__objc_classlist"),
    OBJC_CATLIST = ("__DATA_CONST", "__objc_catlist"),
    OBJC_PROTOLIST = ("__DATA_CONST", "__objc_protolist"),
    OBJC_METHNAME = ("__TEXT", "__objc_methnames"),
    OBJC_CLASSNAME = ("__TEXT", "__objc_classname"),
    OBJC_METHTYPE = ("__TEXT", "__objc_methtype"),
    CSTRING = ("__TEXT", "__cstring"),
    SWIFT_TYPES = ("__TEXT", "__swift5_types"),
    SWIFT_PROTOS = ("__TEXT", "__swift5_protos"),
    SWIFT_PROTO = ("__TEXT", "__swift5_proto"),
    SWIFT_FIELDMD = ("__TEXT", "__swift5_fieldmd"),
);

/// Extract all enrichment data from a Mach-O binary on disk.
pub fn enrich_binary(binary_path: &Path) -> BinaryEnrichment {
    let data = match std::fs::read(binary_path) {
        Ok(d) => d,
        Err(_) => return BinaryEnrichment::default(),
    };
    let file = match object::File::parse(&*data) {
        Ok(f) => f,
        Err(_) => return BinaryEnrichment::default(),
    };

    let mut result = BinaryEnrichment::default();

    // Extract each piece
    result.string_constants = extract_cstrings(&file);
    result.framework_deps = extract_dylib_deps(&file);
    result.objc_classes = extract_objc_classes(&file, &data);
    result.objc_protocols = extract_objc_protocol_names(&file, &data);
    result.swift_types = extract_swift_type_names(&file, &data);
    result.swift_conformances = extract_swift_conformances(&file, &data);

    result
}

/// Extract enrichment from a binary in the dyld shared cache (via subprocess).
/// Falls back to extracting what we can from `class-dump` or ObjC runtime introspection.
pub fn enrich_cached_binary(framework: &str) -> BinaryEnrichment {
    let mut result = BinaryEnrichment::default();

    // Use `objc-dump` / `class-dump` style: runtime introspection via Python
    let script = format!(
    "import objc, json\n\
     bundle = objc.loadBundle('{fw}', bundle_path='/System/Library/PrivateFrameworks/{fw}.framework')\n\
     classes = objc.getClassList()\n\
     fw_classes = [c for c in classes if '{fw}' in str(c)]\n\
     print(json.dumps([str(c) for c in fw_classes[:200]]))\n",
    fw = framework
  );

    // Try Python objc bridge — best-effort
    if let Ok(output) = std::process::Command::new("python3")
        .arg("-c")
        .arg(&script)
        .output()
    {
        if output.status.success() {
            if let Ok(names) = serde_json::from_slice::<Vec<String>>(&output.stdout) {
                for name in names {
                    result.objc_classes.push(ObjCClassInfo {
                        name,
                        ..Default::default()
                    });
                }
            }
        }
    }

    // Also try class-dump for method signatures
    result
        .objc_classes
        .extend(extract_objc_via_classdump(framework));

    result
}

// =============================================================================
// Section extractors
// =============================================================================

/// Read null-terminated strings from __TEXT,__cstring.
fn extract_cstrings(file: &object::File) -> Vec<String> {
    use object::ObjectSection;
    let section = match find_section(file, CSTRING.0, CSTRING.1) {
        Some(s) => s,
        None => return vec![],
    };
    let data = match section.data() {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    // Split on null bytes, keep strings ≥ 4 chars that look semantic
    data.split(|&b| b == 0)
        .filter_map(|bytes| {
            let s = std::str::from_utf8(bytes).ok()?;
            (s.len() >= 4 && is_semantic_string(s)).then(|| s.to_string())
        })
        .collect()
}

/// Heuristic: is this string semantically useful (not a format specifier or path)?
fn is_semantic_string(s: &str) -> bool {
    // Skip format strings, paths, version strings, single tokens
    if s.starts_with('%') || s.starts_with('/') || s.starts_with("com.apple") {
        return false;
    }
    if s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    // Keep error messages, descriptions, key names
    s.contains(' ')
        || s.contains('_')
        || s.contains("Error")
        || s.contains("error")
        || s.contains("fail")
        || s.contains("invalid")
        || s.contains("must")
        || (s.len() > 8 && s.chars().any(|c| c.is_uppercase()))
}

/// Extract framework dependencies from LC_LOAD_DYLIB commands.
fn extract_dylib_deps(file: &object::File) -> Vec<String> {
    use object::Object;
    // The `object` crate exposes imports which include dylib dependencies
    file.imports()
        .unwrap_or_default()
        .iter()
        .filter_map(|imp| {
            let lib = std::str::from_utf8(imp.library()).ok()?;
            // Extract framework name from path: "/.../.framework/FwName" → "FwName"
            let name = lib.rsplit('/').next()?;
            Some(name.to_string())
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

/// Helper: find a section by segment + section name.
fn find_section<'a>(
    file: &'a object::File,
    segment: &str,
    section: &str,
) -> Option<object::read::Section<'a, 'a>> {
    use object::{Object, ObjectSection};
    file.sections().find(|s| {
        let seg = s.segment_name().ok().flatten().unwrap_or("");
        let name = s.name().ok().unwrap_or("");
        seg == segment && name == section
    })
}

// =============================================================================
// ObjC metadata extraction from binary sections
// =============================================================================

/// Extract ObjC classes from __objc_methnames + __objc_methtype + exports.
/// This works even without __objc_classlist (which requires relocation).
fn extract_objc_classes(file: &object::File, _data: &[u8]) -> Vec<ObjCClassInfo> {
    let mut classes: HashMap<String, ObjCClassInfo> = HashMap::new();

    // Strategy: we can't reliably follow pointers in the dyld shared cache,
    // but we CAN read the string sections and cross-reference with exports.

    // Read method names and type encodings
    let method_names = read_section_strings(file, OBJC_METHNAME.0, OBJC_METHNAME.1);
    let class_names = read_section_strings(file, OBJC_CLASSNAME.0, OBJC_CLASSNAME.1);
    let method_types = read_section_strings(file, OBJC_METHTYPE.0, OBJC_METHTYPE.1);

    // Build classes from class names section
    for name in &class_names {
        classes
            .entry(name.clone())
            .or_insert_with(|| ObjCClassInfo {
                name: name.clone(),
                ..Default::default()
            });
    }

    // Build method info from paired method names + types
    // The __objc_methnames and __objc_methtype sections are parallel in practice
    let paired_methods: Vec<ObjCMethodInfo> = method_names
        .iter()
        .zip(method_types.iter())
        .filter(|(name, _)| !name.is_empty() && !name.starts_with('.'))
        .map(|(sel, enc)| {
            let (ret, params) = decode_objc_type_encoding(enc);
            ObjCMethodInfo {
                selector: sel.clone(),
                type_encoding: enc.clone(),
                return_type: ret,
                param_types: params,
                is_class_method: false, // can't determine from strings alone
            }
        })
        .collect();

    // Associate methods with classes heuristically:
    // If a method selector starts with a known class prefix (lowercase), associate it
    for method in &paired_methods {
        for (class_name, info) in &mut classes {
            let prefix = class_name.to_lowercase();
            let sel = method.selector.to_lowercase();
            if sel.starts_with(&prefix)
                || (class_name.len() <= 3 && sel.starts_with(&class_name.to_lowercase()))
            {
                info.instance_methods.push(method.clone());
                break;
            }
        }
    }

    // Fallback: unassociated methods go to a catch-all
    let assigned: std::collections::HashSet<String> = classes
        .values()
        .flat_map(|c| c.instance_methods.iter().map(|m| m.selector.clone()))
        .collect();
    let unassigned: Vec<ObjCMethodInfo> = paired_methods
        .into_iter()
        .filter(|m| !assigned.contains(&m.selector))
        .collect();
    if !unassigned.is_empty() && !classes.is_empty() {
        // Distribute to largest class or create a "Framework" catch-all
        if let Some(biggest) = classes
            .values_mut()
            .max_by_key(|c| c.instance_methods.len())
        {
            // Only add if reasonable number
            for m in unassigned.into_iter().take(50) {
                biggest.instance_methods.push(m);
            }
        }
    }

    classes.into_values().collect()
}

/// Read null-terminated strings from a section.
fn read_section_strings(file: &object::File, segment: &str, section: &str) -> Vec<String> {
    let sect = match find_section(file, segment, section) {
        Some(s) => s,
        None => return vec![],
    };
    let data = match object::ObjectSection::data(&sect) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    data.split(|&b| b == 0)
        .filter_map(|bytes| {
            let s = std::str::from_utf8(bytes).ok()?;
            (!s.is_empty()).then(|| s.to_string())
        })
        .collect()
}

/// Extract ObjC protocol names from __objc_protolist.
fn extract_objc_protocol_names(file: &object::File, _data: &[u8]) -> Vec<String> {
    // Protocol names are in __objc_classname section (shared with class names)
    // We identify them by cross-referencing with OBJC_PROTOCOL_$_ exports
    use object::Object;
    let mut protocols = Vec::new();
    for export in file.exports().unwrap_or_default() {
        let raw = std::str::from_utf8(export.name()).unwrap_or("");
        if let Some(name) = raw.strip_prefix("_OBJC_PROTOCOL_$_") {
            protocols.push(name.to_string());
        }
    }
    protocols
}

/// Extract ObjC via class-dump subprocess (fallback for shared cache binaries).
fn extract_objc_via_classdump(framework: &str) -> Vec<ObjCClassInfo> {
    let path = format!("/System/Library/PrivateFrameworks/{framework}.framework/{framework}");
    let output = match std::process::Command::new("class-dump")
        .arg("--arch")
        .arg("arm64e")
        .arg(&path)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };
    parse_classdump_output(&String::from_utf8_lossy(&output.stdout))
}

/// Parse class-dump text output into ObjCClassInfo structs.
fn parse_classdump_output(output: &str) -> Vec<ObjCClassInfo> {
    let mut classes = Vec::new();
    let mut current: Option<ObjCClassInfo> = None;

    for line in output.lines() {
        let line = line.trim();
        // @interface ClassName : SuperClass <Protocol1, Protocol2>
        if line.starts_with("@interface") {
            if let Some(cls) = current.take() {
                classes.push(cls);
            }
            current = Some(parse_interface_line(line));
        } else if line == "@end" {
            if let Some(cls) = current.take() {
                classes.push(cls);
            }
        } else if let Some(ref mut cls) = current {
            // Method: - (ReturnType)method:(ParamType)param;
            if line.starts_with("- (") || line.starts_with("+ (") {
                if let Some(method) = parse_classdump_method(line) {
                    if method.is_class_method {
                        cls.class_methods.push(method);
                    } else {
                        cls.instance_methods.push(method);
                    }
                }
            }
            // Property: @property (attrs) Type name;
            if line.starts_with("@property") {
                if let Some(prop) = parse_classdump_property(line) {
                    cls.properties.push(prop);
                }
            }
        }
    }
    if let Some(cls) = current {
        classes.push(cls);
    }
    classes
}

/// Parse: @interface ClassName : SuperClass <Proto1, Proto2>
fn parse_interface_line(line: &str) -> ObjCClassInfo {
    let rest = line.strip_prefix("@interface").unwrap_or(line).trim();
    let mut info = ObjCClassInfo::default();

    // Extract protocols in angle brackets
    if let Some(proto_start) = rest.find('<') {
        if let Some(proto_end) = rest.find('>') {
            let protos = &rest[proto_start + 1..proto_end];
            info.protocols = protos
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    // Extract class name and superclass
    let name_part = rest.split('<').next().unwrap_or(rest).trim();
    let parts: Vec<&str> = name_part.split(':').map(|s| s.trim()).collect();
    info.name = parts.first().unwrap_or(&"").to_string();
    info.superclass = parts.get(1).unwrap_or(&"").to_string();
    info
}

/// Parse: - (ReturnType)selectorPart:(ParamType)param otherPart:(ParamType)param;
fn parse_classdump_method(line: &str) -> Option<ObjCMethodInfo> {
    let is_class = line.starts_with('+');
    // Extract return type between parens
    let ret_start = line.find('(')? + 1;
    let ret_end = line.find(')')?;
    let return_type = line[ret_start..ret_end].trim().to_string();

    // Extract selector: everything after ) up to ; with param types removed
    let after_ret = &line[ret_end + 1..].trim_end_matches(';').trim();
    let mut selector = String::new();
    let mut param_types = Vec::new();

    let mut rest = *after_ret;
    loop {
        // Find next colon (selector component)
        if let Some(colon_pos) = rest.find(':') {
            selector.push_str(rest[..colon_pos + 1].trim());
            rest = &rest[colon_pos + 1..];
            // Find param type in parens
            if let Some(p_start) = rest.find('(') {
                if let Some(p_end) = rest.find(')') {
                    param_types.push(rest[p_start + 1..p_end].trim().to_string());
                    rest = &rest[p_end + 1..];
                    // Skip param name (next word)
                    rest = rest.trim_start();
                    if let Some(space) =
                        rest.find(|c: char| c.is_whitespace() || c == ':' || c == ';')
                    {
                        rest = &rest[space..];
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            // No more colons — remaining is selector (no-arg method)
            let rem = rest.trim().trim_end_matches(';');
            if !rem.is_empty() {
                selector.push_str(rem);
            }
            break;
        }
    }

    Some(ObjCMethodInfo {
        selector: selector.trim().to_string(),
        type_encoding: String::new(), // class-dump doesn't give raw encodings
        return_type,
        param_types,
        is_class_method: is_class,
    })
}

/// Parse: @property (nonatomic, readonly) NSString *name;
fn parse_classdump_property(line: &str) -> Option<ObjCPropertyInfo> {
    let rest = line.strip_prefix("@property")?.trim();
    // Skip attributes in parens
    let after_attrs = if rest.starts_with('(') {
        rest.find(')').map(|i| &rest[i + 1..]).unwrap_or(rest)
    } else {
        rest
    };
    let after_attrs = after_attrs.trim().trim_end_matches(';').trim();

    // Last word is the name, everything before is the type
    let parts: Vec<&str> = after_attrs
        .rsplitn(2, |c: char| c.is_whitespace() || c == '*')
        .collect();
    let name = parts
        .first()
        .unwrap_or(&"")
        .trim_start_matches('*')
        .to_string();
    let type_name = if parts.len() > 1 {
        parts[1].trim().to_string()
    } else {
        "id".into()
    };
    let type_name = if after_attrs.contains('*') && !type_name.contains('*') {
        format!("{type_name} *")
    } else {
        type_name
    };

    Some(ObjCPropertyInfo {
        name,
        type_encoding: String::new(),
        attributes: type_name.clone(),
    })
}

// =============================================================================
// Swift metadata extraction
// =============================================================================

/// Extract Swift type names from __swift5_types section.
/// We can't fully parse the relative pointers without relocation info,
/// but we can extract names from the demangled exports.
fn extract_swift_type_names(file: &object::File, _data: &[u8]) -> Vec<SwiftTypeInfo> {
    use object::Object;
    let mut types = Vec::new();

    // Extract from exports: Swift type metadata accessors
    for export in file.exports().unwrap_or_default() {
        let raw = std::str::from_utf8(export.name()).unwrap_or("");
        if !raw.starts_with("_$s") && !raw.starts_with("_$S") {
            continue;
        }
        let m = raw.trim_start_matches('_');

        let kind = classify_swift_kind(m);
        if kind == SwiftTypeKind::Unknown {
            continue;
        }

        let module = extract_swift_module(m);
        types.push(SwiftTypeInfo {
            name: raw.to_string(), // will be demangled later
            mangled_name: raw.to_string(),
            kind,
            module,
            superclass: String::new(),
            protocols: vec![],
            fields: vec![],
            methods: vec![],
        });
    }

    // Batch-demangle
    let mangled: Vec<String> = types.iter().map(|t| t.mangled_name.clone()).collect();
    if !mangled.is_empty() {
        let demangled = crate::extract::demangle_swift_batch(&mangled);
        for t in &mut types {
            if let Some(dem) = demangled.get(&t.mangled_name) {
                t.name = dem.clone();
                // Extract module.Type from demangled name
                if let Some(dot) = dem.rfind('.') {
                    t.module = dem[..dot].to_string();
                }
            }
        }
    }

    types
}

/// Classify Swift mangling suffix → type kind.
macro_rules! swift_suffix_kinds {
  ($m:expr, $( $suffix:literal => $kind:expr ),+ $(,)?) => {
    $( if $m.ends_with($suffix) { return $kind; } )+
  };
}

fn classify_swift_kind(mangled: &str) -> SwiftTypeKind {
    swift_suffix_kinds!(mangled,
      "CN"  => SwiftTypeKind::Class,     // class nominal type descriptor
      "VN"  => SwiftTypeKind::Struct,    // struct nominal type descriptor
      "ON"  => SwiftTypeKind::Enum,      // enum nominal type descriptor
      "Mp"  => SwiftTypeKind::Protocol,  // protocol descriptor
      "Mn"  => SwiftTypeKind::Protocol,  // protocol nominal descriptor
      "Ma"  => SwiftTypeKind::Class,     // metadata accessor
      "Mc"  => SwiftTypeKind::Class,     // metadata completion
      "MF"  => SwiftTypeKind::Struct,    // metadata full type
      "N"   => SwiftTypeKind::Class,     // type metadata (generic)
    );
    SwiftTypeKind::Unknown
}

/// Extract module name from Swift mangling: $sXXYYY → XX is module length + name.
fn extract_swift_module(mangled: &str) -> String {
    let m = mangled
        .strip_prefix("$s")
        .or_else(|| mangled.strip_prefix("$S"))
        .unwrap_or(mangled);
    // Module name is length-prefixed: "7SwiftUI" = "SwiftUI"
    let mut i = 0;
    let bytes = m.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return String::new();
    }
    let len: usize = m[..i].parse().unwrap_or(0);
    if i + len <= m.len() {
        m[i..i + len].to_string()
    } else {
        String::new()
    }
}

/// Extract Swift protocol conformances from __swift5_proto section.
fn extract_swift_conformances(file: &object::File, _data: &[u8]) -> Vec<(String, String)> {
    use object::Object;
    let mut conformances = Vec::new();

    // Extract from exports: protocol conformance descriptors end with "Mc" or contain "WP"
    for export in file.exports().unwrap_or_default() {
        let raw = std::str::from_utf8(export.name()).unwrap_or("");
        if !raw.contains("WP") && !raw.contains("Mc") {
            continue;
        }
        // The demangled form will show: "protocol conformance descriptor for Type : Protocol"
        conformances.push((raw.to_string(), String::new()));
    }

    // Batch demangle to extract the real type/protocol names
    let mangled: Vec<String> = conformances.iter().map(|(m, _)| m.clone()).collect();
    if !mangled.is_empty() {
        let demangled = crate::extract::demangle_swift_batch(&mangled);
        conformances = conformances
            .into_iter()
            .map(|(m, _)| {
                let dem = demangled.get(&m).cloned().unwrap_or_default();
                // Parse: "... for TypeName : ProtocolName in ModuleName"
                let (type_name, proto_name) = parse_conformance_demangled(&dem);
                (type_name, proto_name)
            })
            .filter(|(t, p)| !t.is_empty() && !p.is_empty())
            .collect();
    }

    conformances
}

/// Parse a demangled conformance string: "protocol conformance descriptor for X : Y in Z"
fn parse_conformance_demangled(dem: &str) -> (String, String) {
    // Common forms:
    //   "protocol conformance descriptor for Module.Type : Module.Protocol in Module"
    //   "lazy protocol witness table accessor ... Type : Protocol"
    let for_idx = dem.find(" for ").map(|i| i + 5);
    let rest = for_idx.map(|i| &dem[i..]).unwrap_or(dem);
    if let Some(colon) = rest.find(" : ") {
        let type_name = rest[..colon]
            .trim()
            .strip_prefix("type ")
            .unwrap_or(rest[..colon].trim())
            .to_string();
        let after_colon = &rest[colon + 3..];
        let proto_name = after_colon
            .split(" in ")
            .next()
            .unwrap_or(after_colon)
            .trim()
            .to_string();
        (type_name, proto_name)
    } else {
        (String::new(), String::new())
    }
}

// =============================================================================
// Enrichment → ApiEntry fields
// =============================================================================

/// Data format info for a function/method: what it consumes and produces.
#[derive(Debug, Clone, Default)]
pub struct DataFormats {
    pub input_formats: Vec<String>,
    pub output_formats: Vec<String>,
}

/// Derive input/output data formats from an ObjC method.
pub fn method_data_formats(method: &ObjCMethodInfo) -> DataFormats {
    DataFormats {
        input_formats: method.param_types.clone(),
        output_formats: if method.return_type == "void" {
            vec![]
        } else {
            vec![method.return_type.clone()]
        },
    }
}

/// Derive semantic hints from string constants.
/// Returns (keyword → related strings) mapping.
pub fn semantic_hints_from_strings(strings: &[String]) -> HashMap<String, Vec<String>> {
    let mut hints: HashMap<String, Vec<String>> = HashMap::new();

    macro_rules! keyword_buckets {
    ($s:expr, $hints:expr, $( $keyword:literal => $bucket:literal ),+ $(,)?) => {
      let lower = $s.to_lowercase();
      $( if lower.contains($keyword) { $hints.entry($bucket.to_string()).or_default().push($s.to_string()); } )+
    };
  }

    for s in strings {
        keyword_buckets!(s, hints,
          "decode" => "decode",
          "encode" => "encode",
          "compress" => "compress",
          "decompress" => "decompress",
          "frame"  => "video",
          "pixel"  => "image",
          "buffer" => "buffer",
          "surface" => "surface",
          "texture" => "gpu",
          "shader"  => "gpu",
          "kernel"  => "gpu",
          "neural"  => "ml",
          "model"   => "ml",
          "inference" => "ml",
          "matrix"  => "linalg",
          "vector"  => "linalg",
        );
    }

    hints
}

/// Build a class hierarchy map from ObjC classes: class_name → Vec<ancestors>.
pub fn build_class_hierarchy(classes: &[ObjCClassInfo]) -> HashMap<String, Vec<String>> {
    let parent_map: HashMap<&str, &str> = classes
        .iter()
        .filter(|c| !c.superclass.is_empty())
        .map(|c| (c.name.as_str(), c.superclass.as_str()))
        .collect();

    let mut hierarchy = HashMap::new();
    for class in classes {
        let mut chain = Vec::new();
        let mut current = class.name.as_str();
        while let Some(&parent) = parent_map.get(current) {
            chain.push(parent.to_string());
            current = parent;
            if chain.len() > 20 {
                break;
            } // cycle guard
        }
        hierarchy.insert(class.name.clone(), chain);
    }
    hierarchy
}

/// Build a type → protocol map from Swift conformances.
pub fn build_protocol_map(conformances: &[(String, String)]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (type_name, proto) in conformances {
        map.entry(type_name.clone())
            .or_default()
            .push(proto.clone());
    }
    map
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_decode_type_char() {
        assert_eq!(decode_type_char(b'i'), "int");
        assert_eq!(decode_type_char(b'@'), "id");
        assert_eq!(decode_type_char(b'v'), "void");
        assert_eq!(decode_type_char(b'B'), "bool");
        assert_eq!(decode_type_char(b'Q'), "unsigned long long");
        assert_eq!(decode_type_char(b'd'), "double");
    }

    #[test]
    fn test_decode_objc_type_encoding_simple() {
        // v24@0:8@16 → void, params: [id] (self and _cmd are stripped)
        let (ret, params) = decode_objc_type_encoding("v24@0:8@16");
        assert_eq!(ret, "void");
        assert_eq!(params, vec!["id"]);
    }

    #[test]
    fn test_decode_objc_type_encoding_return_id() {
        // @24@0:8@16 → id, params: [id]
        let (ret, params) = decode_objc_type_encoding("@24@0:8@16");
        assert_eq!(ret, "id");
        assert_eq!(params, vec!["id"]);
    }

    #[test]
    fn test_decode_objc_type_encoding_bool() {
        // B16@0:8 → bool, no params
        let (ret, params) = decode_objc_type_encoding("B16@0:8");
        assert_eq!(ret, "bool");
        assert!(params.is_empty());
    }

    #[test]
    fn test_decode_objc_type_encoding_with_class() {
        // @"NSString"24@0:8@"NSData"16 → NSString *, params: [NSData *]
        let (ret, params) = decode_objc_type_encoding("@\"NSString\"24@0:8@\"NSData\"16");
        assert_eq!(ret, "NSString *");
        assert_eq!(params, vec!["NSData *"]);
    }

    #[test]
    fn test_decode_objc_type_encoding_struct() {
        // {CGRect=dddd}32@0:8 → struct CGRect, no params
        let (ret, params) = decode_objc_type_encoding("{CGRect=dddd}32@0:8");
        assert_eq!(ret, "struct CGRect");
        assert!(params.is_empty());
    }

    #[test]
    fn test_decode_property_type() {
        assert_eq!(decode_property_type("T@\"NSString\",R,N"), "NSString *");
        assert_eq!(decode_property_type("Ti,N"), "int");
        assert_eq!(decode_property_type("TB,N"), "bool");
        assert_eq!(decode_property_type("T@\"NSArray\",C,N"), "NSArray *");
    }

    #[test]
    fn test_extract_verb() {
        assert_eq!(extract_verb("IOSurfaceCreate"), "create");
        assert_eq!(extract_verb("VTDecompressionSessionDecodeFrame"), "decode");
        assert_eq!(extract_verb("-[MTLDevice newLibraryWithSource:]"), "create");
        assert_eq!(extract_verb("vImageScale_ARGB8888"), "transform");
        assert_eq!(extract_verb("IOSurfaceGetBaseAddress"), "read");
        assert_eq!(extract_verb("MTLCommandBufferCommit"), "execute");
        assert_eq!(extract_verb("IOSurfaceIsInUse"), "predicate");
        assert_eq!(extract_verb("ANECompressModel"), "encode");
    }

    #[test]
    fn test_split_camel_case() {
        assert_eq!(
            split_camel_case("IOSurfaceCreate"),
            vec!["IO", "Surface", "Create"]
        );
        assert_eq!(
            split_camel_case("VTDecompressionSessionDecodeFrame"),
            vec!["VT", "Decompression", "Session", "Decode", "Frame"]
        );
        assert_eq!(
            split_camel_case("vImageScale_ARGB8888"),
            vec!["v", "Image", "Scale", "ARGB8888"]
        );
        assert_eq!(
            split_camel_case("decodeFrameWithData"),
            vec!["decode", "Frame", "With", "Data"]
        );
        assert_eq!(split_camel_case("isInUse"), vec!["is", "In", "Use"]);
    }

    #[test]
    fn test_is_semantic_string() {
        assert!(is_semantic_string("Failed to decode frame"));
        assert!(is_semantic_string("invalid_buffer_size"));
        assert!(is_semantic_string("IOSurfaceCreate must be called first"));
        assert!(!is_semantic_string("%d"));
        assert!(!is_semantic_string("/usr/lib/foo"));
        assert!(!is_semantic_string("1.0.3"));
        assert!(!is_semantic_string("com.apple.metal"));
    }

    #[test]
    fn test_semantic_hints_from_strings() {
        let strings = vec![
            "Failed to decode video frame".to_string(),
            "pixel buffer underrun".to_string(),
            "shader compilation error".to_string(),
            "neural network inference timeout".to_string(),
        ];
        let hints = semantic_hints_from_strings(&strings);
        assert!(hints.contains_key("decode"));
        assert!(hints.contains_key("video"));
        assert!(hints.contains_key("image"));
        assert!(hints.contains_key("gpu"));
        assert!(hints.contains_key("ml"));
    }

    #[test]
    fn test_build_class_hierarchy() {
        let classes = vec![
            ObjCClassInfo {
                name: "MTLDevice".into(),
                superclass: "NSObject".into(),
                ..Default::default()
            },
            ObjCClassInfo {
                name: "MTLGPUDevice".into(),
                superclass: "MTLDevice".into(),
                ..Default::default()
            },
            ObjCClassInfo {
                name: "NSObject".into(),
                ..Default::default()
            },
        ];
        let hierarchy = build_class_hierarchy(&classes);
        assert_eq!(hierarchy["MTLGPUDevice"], vec!["MTLDevice", "NSObject"]);
        assert_eq!(hierarchy["MTLDevice"], vec!["NSObject"]);
        assert!(hierarchy["NSObject"].is_empty());
    }

    #[test]
    fn test_classify_swift_kind() {
        assert_eq!(
            classify_swift_kind("$s5Metal9MTLDeviceCN"),
            SwiftTypeKind::Class
        );
        assert_eq!(
            classify_swift_kind("$s5Metal10MTLDeviceMp"),
            SwiftTypeKind::Protocol
        );
        assert_eq!(
            classify_swift_kind("$s5Metal12GPUCountersVN"),
            SwiftTypeKind::Struct
        );
        assert_eq!(
            classify_swift_kind("$s5Metal9ErrorKindON"),
            SwiftTypeKind::Enum
        );
    }

    #[test]
    fn test_extract_swift_module() {
        assert_eq!(extract_swift_module("$s7SwiftUI4ViewP"), "SwiftUI");
        assert_eq!(extract_swift_module("$s5Metal10MTLDeviceCN"), "Metal");
        assert_eq!(extract_swift_module("$s10Foundation4DataV"), "Foundation");
    }

    #[test]
    fn test_parse_conformance_demangled() {
        let (t, p) = parse_conformance_demangled(
            "protocol conformance descriptor for Metal.GPUDevice : Metal.MTLDevice in Metal",
        );
        assert_eq!(t, "Metal.GPUDevice");
        assert_eq!(p, "Metal.MTLDevice");

        let (t, p) =
            parse_conformance_demangled("lazy protocol witness table accessor for type Foo : Bar");
        assert_eq!(t, "Foo");
        assert_eq!(p, "Bar");
    }

    #[test]
    fn test_parse_interface_line() {
        let info = parse_interface_line(
            "@interface VTDecompressionSession : VTSession <VTFrameDecoder, NSCoding>",
        );
        assert_eq!(info.name, "VTDecompressionSession");
        assert_eq!(info.superclass, "VTSession");
        assert_eq!(info.protocols, vec!["VTFrameDecoder", "NSCoding"]);
    }

    #[test]
    fn test_parse_classdump_method() {
        let method = parse_classdump_method(
            "- (void)decodeFrame:(NSData *)data completionHandler:(void (^)(void))handler;",
        )
        .unwrap();
        assert_eq!(method.return_type, "void");
        assert!(!method.is_class_method);
        assert!(method.selector.contains("decodeFrame:"));

        let method = parse_classdump_method(
            "+ (instancetype)sessionWithConfiguration:(NSDictionary *)config;",
        )
        .unwrap();
        assert_eq!(method.return_type, "instancetype");
        assert!(method.is_class_method);
    }

    #[test]
    fn test_parse_classdump_property() {
        let prop =
            parse_classdump_property("@property (nonatomic, readonly) NSString *name;").unwrap();
        assert_eq!(prop.name, "name");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_enrich_iosurface() {
        let path = std::path::Path::new(
            "/System/Library/Frameworks/IOSurface.framework/Versions/A/IOSurface",
        );
        if !path.exists() {
            return;
        } // skip on non-macOS
        let enrichment = enrich_binary(path);
        // IOSurface should have string constants
        eprintln!(
            "IOSurface enrichment: {} strings, {} deps, {} objc classes",
            enrichment.string_constants.len(),
            enrichment.framework_deps.len(),
            enrichment.objc_classes.len()
        );
        // Should find at least some strings
        assert!(
            enrichment.string_constants.len() > 0 || enrichment.framework_deps.len() > 0,
            "should extract something from IOSurface"
        );
    }
}
