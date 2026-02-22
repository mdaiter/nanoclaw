//! Layer 1: Framework extraction.
//!
//! Four language families, all extracted from one binary:
//!   C       — plain symbols, stripped leading underscore
//!   C++     — mangled __Z* symbols, demangled via c++filt
//!   ObjC    — OBJC_CLASS_$_, OBJC_IVAR_$_, protocols, metaclasses
//!   Swift   — _$s* mangled symbols, demangled via swift-demangle
//!
//! Three extraction paths:
//!   1. dyld_info -exports  — dyld shared cache (private frameworks, most of macOS)
//!   2. object crate        — standalone Mach-O binaries on disk
//!   3. clang -ast-dump=json — public framework headers (typed C/ObjC signatures)
//!
//! Inspired by clang AST autogen patterns (clang AST + SDK headers) and
//! MachOSwiftSection (Mach-O section parsing + Swift demangling).

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

// =============================================================================
// Types
// =============================================================================

/// A raw symbol export from a framework binary.
#[derive(Debug, Clone)]
pub struct RawExport {
    pub name: String,    // demangled (or stripped) name
    pub mangled: String, // original mangled symbol
    pub offset: u64,
    pub kind: ExportKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExportKind {
    Function,      // C function
    CppFunction,   // C++ function (demangled)
    CppMethod,     // C++ method (demangled, includes Class::method)
    Data,          // global data / constant
    ObjCClass,     // _OBJC_CLASS_$_ClassName
    ObjCMetaclass, // _OBJC_METACLASS_$_ClassName
    ObjCProtocol,  // _OBJC_PROTOCOL_$_ProtocolName
    ObjCIvar,      // _OBJC_IVAR_$_Class.field
    SwiftFunction, // Swift function (demangled)
    SwiftType,     // Swift type descriptor
    SwiftProtocol, // Swift protocol descriptor
}

/// A parsed declaration from clang AST (C, ObjC).
#[derive(Debug, Clone)]
pub struct ParsedDecl {
    pub name: String,
    pub kind: DeclKind,
    pub return_type: String,
    pub parameters: Vec<(String, String)>, // (name, type)
    pub is_variadic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclKind {
    Function,
    ObjCMethod {
        class: String,
        is_class_method: bool,
    },
    Typedef,
}

// =============================================================================
// Subprocess timeout helper
// =============================================================================

/// Run a subprocess with a timeout. Spawns a thread for `.wait_with_output()`,
/// uses a channel with `recv_timeout` to enforce the deadline.
/// Kills the child on timeout via SIGKILL.
fn run_with_timeout(cmd: Command, timeout_secs: u64) -> Option<std::process::Output> {
    use std::sync::{mpsc, Arc, Mutex};

    let child_pid: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let pid_w = child_pid.clone();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let mut c = cmd;
        match c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
            Ok(ch) => {
                *pid_w.lock().unwrap() = Some(ch.id());
                let _ = tx.send(ch.wait_with_output().ok());
            }
            Err(_) => {
                let _ = tx.send(None);
            }
        }
    });

    match rx.recv_timeout(std::time::Duration::from_secs(timeout_secs)) {
        Ok(result) => result,
        Err(_) => {
            // Timeout — kill via SIGKILL
            if let Some(pid) = *child_pid.lock().unwrap() {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
            None
        }
    }
}

// =============================================================================
// dyld_info extraction (private frameworks + dyld shared cache)
// =============================================================================

/// Extract ALL exports from a framework using `dyld_info -exports`.
/// Keeps C, C++, ObjC, and Swift symbols. Demangles C++ and Swift in batch.
pub fn dyld_info_exports(framework_path: &str) -> Vec<RawExport> {
    let mut cmd = Command::new("dyld_info");
    cmd.args(["-exports", framework_path]);
    let output = match run_with_timeout(cmd, 30) {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let (mut exports, cpp_mangled, swift_mangled) = parse_dyld_exports_raw(&stdout);

    // Batch-demangle C++ symbols
    if !cpp_mangled.is_empty() {
        let demangled = demangle_cpp_batch(&cpp_mangled);
        for export in &mut exports {
            if export.kind == ExportKind::CppFunction || export.kind == ExportKind::CppMethod {
                if let Some(dem) = demangled.get(&export.mangled) {
                    export.name = dem.clone();
                    // Classify: has "::" → method, else free function
                    export.kind = if dem.contains("::") {
                        ExportKind::CppMethod
                    } else {
                        ExportKind::CppFunction
                    };
                }
            }
        }
    }

    // Batch-demangle Swift symbols
    if !swift_mangled.is_empty() {
        let demangled = demangle_swift_batch(&swift_mangled);
        for export in &mut exports {
            if matches!(
                export.kind,
                ExportKind::SwiftFunction | ExportKind::SwiftType | ExportKind::SwiftProtocol
            ) {
                if let Some(dem) = demangled.get(&export.mangled) {
                    export.name = dem.clone();
                    // Classify based on demangled output
                    export.kind = classify_swift_demangled(dem);
                }
            }
        }
    }

    exports
}

/// Parse raw dyld_info output, returning exports + lists of mangled symbols needing demangling.
fn parse_dyld_exports_raw(output: &str) -> (Vec<RawExport>, Vec<String>, Vec<String>) {
    let mut exports = Vec::new();
    let mut cpp_mangled = Vec::new();
    let mut swift_mangled = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("offset")
            || line.starts_with('-')
            || line.ends_with(':')
            || line.starts_with("/")
            || line.starts_with("-exports")
        {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            continue;
        }

        let offset = parts[0]
            .trim()
            .strip_prefix("0x")
            .and_then(|h| u64::from_str_radix(h, 16).ok())
            .unwrap_or(0);
        let raw = parts[1].trim();
        if raw.is_empty() {
            continue;
        }

        let (name, kind) = classify_symbol(raw);
        if name.is_empty() {
            continue;
        }

        // Collect mangled symbols for batch demangling
        match &kind {
            ExportKind::CppFunction | ExportKind::CppMethod => cpp_mangled.push(raw.to_string()),
            ExportKind::SwiftFunction | ExportKind::SwiftType | ExportKind::SwiftProtocol => {
                swift_mangled.push(raw.to_string())
            }
            _ => {}
        }

        exports.push(RawExport {
            name,
            mangled: raw.to_string(),
            offset,
            kind,
        });
    }

    (exports, cpp_mangled, swift_mangled)
}

/// Try each prefix in order; return (stripped_name, kind) on first match.
macro_rules! strip_classify {
  ($raw:expr, $( $prefix:literal => $kind:expr ),+ $(,)?) => {
    $( if let Some(name) = $raw.strip_prefix($prefix) { return (name.into(), $kind); } )+
  };
}

/// Classify a raw symbol. Returns (display_name, kind).
/// Does NOT demangle — that happens in batch after classification.
fn classify_symbol(raw: &str) -> (String, ExportKind) {
    // ObjC prefixes — pure table
    strip_classify!(raw,
      "_OBJC_CLASS_$_"     => ExportKind::ObjCClass,
      "_OBJC_METACLASS_$_" => ExportKind::ObjCMetaclass,
      "_OBJC_PROTOCOL_$_"  => ExportKind::ObjCProtocol,
      "_OBJC_IVAR_$_"      => ExportKind::ObjCIvar,
    );
    // Swift mangled: _$s or _$S or __$s → placeholder until batch demangling
    if raw.starts_with("_$s") || raw.starts_with("_$S") || raw.starts_with("__$s") {
        return (raw.to_string(), classify_swift_mangled(raw));
    }
    // C++ mangled: __Z or ___Z → placeholder until batch demangling
    if raw.starts_with("__Z") || raw.starts_with("___Z") {
        return (raw.to_string(), ExportKind::CppFunction);
    }
    // Data symbols: _k... constants (e.g. _kIOSurfaceAllocSize)
    if raw.starts_with("_k") && raw.chars().nth(2).is_some_and(|c| c.is_uppercase()) {
        return (
            raw.strip_prefix('_').unwrap_or(raw).into(),
            ExportKind::Data,
        );
    }
    // Regular C function: strip leading underscore
    let name = raw.strip_prefix('_').unwrap_or(raw);
    if name.is_empty() || name.starts_with('.') {
        return (String::new(), ExportKind::Data);
    }
    (name.into(), ExportKind::Function)
}

/// Classify a Swift mangled symbol by its mangling prefix.
/// See https://github.com/swiftlang/swift/blob/main/docs/ABI/Mangling.rst
fn classify_swift_mangled(raw: &str) -> ExportKind {
    // Strip leading underscores to get the mangling
    let m = raw.trim_start_matches('_');
    // Protocol descriptor: $s...Mp
    if m.ends_with("Mp") || m.ends_with("Mn") {
        return ExportKind::SwiftProtocol;
    }
    // Type metadata: $s...N, $s...Ma, $s...Mc
    if m.ends_with('N') || m.ends_with("Ma") || m.ends_with("Mc") {
        return ExportKind::SwiftType;
    }
    ExportKind::SwiftFunction
}

/// Classify a demangled Swift symbol.
fn classify_swift_demangled(dem: &str) -> ExportKind {
    if dem.contains("protocol descriptor") || dem.contains("protocol conformance") {
        return ExportKind::SwiftProtocol;
    }
    if dem.contains("type metadata") || dem.contains("nominal type") {
        return ExportKind::SwiftType;
    }
    ExportKind::SwiftFunction
}

// =============================================================================
// Batch demangling via subprocess
// =============================================================================

/// Batch-demangle symbols by piping them through a subprocess. Returns mangled → demangled map.
fn demangle_batch(cmd: &str, args: &[&str], mangled: &[String]) -> HashMap<String, String> {
    let cmd_str = cmd.to_string();
    let args_owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let input = mangled.join("\n");
    let mangled_clone = mangled.to_vec();

    let (tx, rx) = std::sync::mpsc::channel();
    let pid: std::sync::Arc<std::sync::Mutex<Option<u32>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let pid_w = pid.clone();

    std::thread::spawn(move || {
        let mut child = match Command::new(&cmd_str)
            .args(&args_owned)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => {
                let _ = tx.send(None);
                return;
            }
        };
        *pid_w.lock().unwrap() = Some(child.id());
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
        }
        let _ = tx.send(child.wait_with_output().ok());
    });

    let output = match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(Some(o)) => o,
        _ => {
            if let Some(p) = *pid.lock().unwrap() {
                unsafe {
                    libc::kill(p as i32, libc::SIGKILL);
                }
            }
            return HashMap::new();
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    mangled_clone
        .iter()
        .zip(stdout.lines())
        .filter_map(|(sym, dem)| {
            let dem = dem.trim();
            (!dem.is_empty() && dem != sym).then(|| (sym.clone(), dem.to_string()))
        })
        .collect()
}

fn demangle_cpp_batch(mangled: &[String]) -> HashMap<String, String> {
    demangle_batch("c++filt", &[], mangled)
}

pub fn demangle_swift_batch(mangled: &[String]) -> HashMap<String, String> {
    demangle_batch("xcrun", &["swift-demangle"], mangled)
}

// =============================================================================
// Mach-O direct parsing (standalone binaries on disk)
// =============================================================================

/// Parse exports from a standalone Mach-O binary using the `object` crate.
/// Returns empty if the binary is in the dyld shared cache (not on disk).
pub fn macho_exports(binary_path: &Path) -> Vec<RawExport> {
    let data = match std::fs::read(binary_path) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    let file = match object::File::parse(&*data) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    use object::Object;
    let mut exports = Vec::new();
    let mut cpp_mangled = Vec::new();
    let mut swift_mangled = Vec::new();

    for export in file.exports().unwrap_or_default() {
        let raw = std::str::from_utf8(export.name()).unwrap_or("");
        let (name, kind) = classify_symbol(raw);
        if name.is_empty() {
            continue;
        }

        match &kind {
            ExportKind::CppFunction | ExportKind::CppMethod => cpp_mangled.push(raw.to_string()),
            ExportKind::SwiftFunction | ExportKind::SwiftType | ExportKind::SwiftProtocol => {
                swift_mangled.push(raw.to_string())
            }
            _ => {}
        }

        exports.push(RawExport {
            name,
            mangled: raw.to_string(),
            offset: export.address(),
            kind,
        });
    }

    // Batch-demangle
    if !cpp_mangled.is_empty() {
        let demangled = demangle_cpp_batch(&cpp_mangled);
        for export in &mut exports {
            if export.kind == ExportKind::CppFunction || export.kind == ExportKind::CppMethod {
                if let Some(dem) = demangled.get(&export.mangled) {
                    export.name = dem.clone();
                    export.kind = if dem.contains("::") {
                        ExportKind::CppMethod
                    } else {
                        ExportKind::CppFunction
                    };
                }
            }
        }
    }
    if !swift_mangled.is_empty() {
        let demangled = demangle_swift_batch(&swift_mangled);
        for export in &mut exports {
            if matches!(
                export.kind,
                ExportKind::SwiftFunction | ExportKind::SwiftType | ExportKind::SwiftProtocol
            ) {
                if let Some(dem) = demangled.get(&export.mangled) {
                    export.name = dem.clone();
                    export.kind = classify_swift_demangled(dem);
                }
            }
        }
    }

    exports
}

// =============================================================================
// Clang AST dump (public framework headers → typed C/ObjC declarations)
// =============================================================================

/// Parse a public framework header using `clang -Xclang -ast-dump=json`.
/// Uses clang autogen pattern: `-xobjective-c -isysroot <SDK>`.
/// Parses FunctionDecl and ObjCMethodDecl.
#[cfg(target_os = "macos")]
pub fn clang_ast_declarations(header_path: &Path, framework: &str) -> Vec<ParsedDecl> {
    let sdk = sdk_path();
    let mut args = vec![
        "-Xclang".to_string(),
        "-ast-dump=json".to_string(),
        "-fsyntax-only".to_string(),
        "-xobjective-c".to_string(),
    ];
    if !sdk.is_empty() {
        args.push("-isysroot".to_string());
        args.push(sdk);
    }
    args.push(header_path.to_string_lossy().to_string());

    let mut cmd = Command::new("clang");
    cmd.args(&args);
    let output = match run_with_timeout(cmd, 30) {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    parse_clang_ast_json(&output.stdout, framework)
}

#[cfg(not(target_os = "macos"))]
pub fn clang_ast_declarations(_header_path: &Path, _framework: &str) -> Vec<ParsedDecl> {
    vec![]
}

/// Parse clang's JSON AST output, extracting C functions and ObjC methods.
fn parse_clang_ast_json(data: &[u8], framework: &str) -> Vec<ParsedDecl> {
    let root: serde_json::Value = match serde_json::from_slice(data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut decls = Vec::new();
    if let Some(inner) = root.get("inner").and_then(|v| v.as_array()) {
        for node in inner {
            let kind = node.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "FunctionDecl" => {
                    if let Some(decl) = parse_function_decl(node) {
                        decls.push(decl);
                    }
                }
                "ObjCInterfaceDecl" => {
                    parse_objc_interface(node, framework, &mut decls);
                }
                "ObjCCategoryDecl" => {
                    parse_objc_interface(node, framework, &mut decls);
                }
                _ => {}
            }
        }
    }
    decls
}

fn parse_function_decl(node: &serde_json::Value) -> Option<ParsedDecl> {
    let name = node.get("name").and_then(|v| v.as_str())?.to_string();
    if name.starts_with("__builtin") {
        return None;
    }

    let qual_type = node
        .get("type")
        .and_then(|t| t.get("qualType"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let return_type = qual_type
        .split('(')
        .next()
        .unwrap_or("void")
        .trim()
        .to_string();

    let mut parameters = Vec::new();
    let is_variadic = qual_type.contains("...");

    if let Some(inner) = node.get("inner").and_then(|v| v.as_array()) {
        for child in inner {
            if child.get("kind").and_then(|v| v.as_str()) == Some("ParmVarDecl") {
                let pname = child
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ptype = child
                    .get("type")
                    .and_then(|t| t.get("qualType"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("void")
                    .to_string();
                parameters.push((pname, ptype));
            }
        }
    }

    Some(ParsedDecl {
        name,
        kind: DeclKind::Function,
        return_type,
        parameters,
        is_variadic,
    })
}

/// Parse ObjC interface/category: extract class name and methods.
fn parse_objc_interface(node: &serde_json::Value, _framework: &str, decls: &mut Vec<ParsedDecl>) {
    let class = match node.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return,
    };

    let inner = match node.get("inner").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return,
    };

    for child in inner {
        let kind = child.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "ObjCMethodDecl" {
            continue;
        }

        let method_name = match child.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let is_class_method = child.get("instance").and_then(|v| v.as_bool()) == Some(false);
        let ret_type = child
            .get("returnType")
            .and_then(|t| t.get("qualType"))
            .and_then(|v| v.as_str())
            .unwrap_or("id")
            .to_string();

        let mut params = Vec::new();
        if let Some(method_inner) = child.get("inner").and_then(|v| v.as_array()) {
            for p in method_inner {
                if p.get("kind").and_then(|v| v.as_str()) == Some("ParmVarDecl") {
                    let pname = p
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let ptype = p
                        .get("type")
                        .and_then(|t| t.get("qualType"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("id")
                        .to_string();
                    params.push((pname, ptype));
                }
            }
        }

        // ObjC method: selector format "-[Class method:]" or "+[Class method:]"
        let prefix = if is_class_method { "+" } else { "-" };
        let display_name = format!("{prefix}[{class} {method_name}]");

        decls.push(ParsedDecl {
            name: display_name,
            kind: DeclKind::ObjCMethod {
                class: class.clone(),
                is_class_method,
            },
            return_type: ret_type,
            parameters: params,
            is_variadic: false,
        });
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Resolve the SDK path from platform config.
pub fn sdk_path() -> String {
    crate::config::platform().sdk_path().unwrap_or_default()
}

/// Resolve the framework binary path.
/// Tries private, then public from platform config. Uses dyld_info to check shared cache.
pub fn resolve_framework_binary(name: &str) -> Option<String> {
    let config = crate::config::platform();
    let paths = [
        config.framework_binary(name, true),  // private first
        config.framework_binary(name, false), // then public
    ];
    for path in &paths {
        if path.is_empty() {
            continue;
        }
        if Path::new(path).exists() {
            return Some(path.clone());
        }
    }
    // Check dyld shared cache — one subprocess per candidate, stop at first hit
    for path in &paths {
        if path.is_empty() {
            continue;
        }
        if is_in_dyld_cache(path) {
            return Some(path.clone());
        }
    }
    None
}

/// Check if a framework path is accessible via dyld_info (in the shared cache).
/// Times out after 5 seconds to prevent hangs on invalid paths.
fn is_in_dyld_cache(path: &str) -> bool {
    let mut child = match Command::new("dyld_info")
        .args(["-exports", path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return false,
        }
    }
}

/// Summary of extraction results by kind.
pub fn export_summary(exports: &[RawExport]) -> HashMap<ExportKind, usize> {
    let mut counts = HashMap::new();
    for e in exports {
        *counts.entry(e.kind.clone()).or_insert(0) += 1;
    }
    counts
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_symbol_c_function() {
        let (name, kind) = classify_symbol("_SLSMainConnectionID");
        assert_eq!(name, "SLSMainConnectionID");
        assert_eq!(kind, ExportKind::Function);
    }

    #[test]
    fn test_classify_symbol_objc_class() {
        let (name, kind) = classify_symbol("_OBJC_CLASS_$_NSWindow");
        assert_eq!(name, "NSWindow");
        assert_eq!(kind, ExportKind::ObjCClass);
    }

    #[test]
    fn test_classify_symbol_objc_metaclass() {
        let (name, kind) = classify_symbol("_OBJC_METACLASS_$_MTLDevice");
        assert_eq!(name, "MTLDevice");
        assert_eq!(kind, ExportKind::ObjCMetaclass);
    }

    #[test]
    fn test_classify_symbol_objc_ivar() {
        let (name, kind) = classify_symbol("_OBJC_IVAR_$_MTLCaptureScope._device");
        assert_eq!(name, "MTLCaptureScope._device");
        assert_eq!(kind, ExportKind::ObjCIvar);
    }

    #[test]
    fn test_classify_symbol_swift() {
        let (_, kind) = classify_symbol("_$s7SwiftUI4ViewPAAE0B0V");
        assert_eq!(kind, ExportKind::SwiftFunction);
    }

    #[test]
    fn test_classify_symbol_swift_protocol() {
        let (_, kind) = classify_symbol("_$s5Metal10MTLDeviceMp");
        assert_eq!(kind, ExportKind::SwiftProtocol);
    }

    #[test]
    fn test_classify_symbol_cpp() {
        let (_, kind) = classify_symbol("__Z20generateFunctionHashP22MTL4FunctionDescriptor");
        assert_eq!(kind, ExportKind::CppFunction);
    }

    #[test]
    fn test_classify_symbol_constant() {
        let (name, kind) = classify_symbol("_kIOSurfaceAllocSize");
        assert_eq!(name, "kIOSurfaceAllocSize");
        assert_eq!(kind, ExportKind::Data);
    }

    #[test]
    fn test_parse_dyld_exports_empty() {
        let (exports, _, _) = parse_dyld_exports_raw("");
        assert!(exports.is_empty());
        let (exports, _, _) = parse_dyld_exports_raw("offset  symbol\n");
        assert!(exports.is_empty());
    }

    #[test]
    fn test_parse_dyld_exports_mixed() {
        let sample = "\
/System/Library/Frameworks/Metal.framework/Metal [arm64e]:
    -exports:
        offset      symbol
        0x00001000  _FooCreate
        0x00002000  _FooDestroy
        0x00003000  _OBJC_CLASS_$_MTLDevice
        0x00004000  __Z20generateFunctionHashP22MTL4FunctionDescriptor
        0x00005000  _$s7SwiftUI4ViewPAAE0B0V
        0x00006000  _OBJC_IVAR_$_MTLCaptureScope._device
        0x00007000  _kMTLSomeConstant
";
        let (exports, cpp, swift) = parse_dyld_exports_raw(sample);
        assert_eq!(exports.len(), 7);
        assert_eq!(cpp.len(), 1);
        assert_eq!(swift.len(), 1);

        // Check each kind
        assert_eq!(exports[0].kind, ExportKind::Function);
        assert_eq!(exports[0].name, "FooCreate");
        assert_eq!(exports[2].kind, ExportKind::ObjCClass);
        assert_eq!(exports[2].name, "MTLDevice");
        assert_eq!(exports[3].kind, ExportKind::CppFunction);
        assert_eq!(exports[4].kind, ExportKind::SwiftFunction);
        assert_eq!(exports[5].kind, ExportKind::ObjCIvar);
        assert_eq!(exports[6].kind, ExportKind::Data);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_cpp_demangling() {
        let mangled = vec!["__Z20generateFunctionHashP22MTL4FunctionDescriptor".to_string()];
        let demangled = demangle_cpp_batch(&mangled);
        assert!(!demangled.is_empty(), "c++filt should demangle");
        let dem = &demangled[&mangled[0]];
        assert!(
            dem.contains("generateFunctionHash"),
            "demangled should contain function name, got: {dem}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_swift_demangling() {
        let mangled = vec!["_$s7SwiftUI4ViewP".to_string()];
        let demangled = demangle_swift_batch(&mangled);
        assert!(!demangled.is_empty(), "swift-demangle should demangle");
        let dem = &demangled[&mangled[0]];
        assert!(
            dem.contains("SwiftUI") || dem.contains("View"),
            "demangled should contain type info, got: {dem}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_dyld_info_metal_all_kinds() {
        let path = "/System/Library/Frameworks/Metal.framework/Metal";
        let exports = dyld_info_exports(path);
        assert!(!exports.is_empty(), "dyld_info should find Metal exports");

        let summary = export_summary(&exports);
        // Metal has C functions, ObjC classes, C++ functions, ivars
        assert!(
            summary.get(&ExportKind::Function).copied().unwrap_or(0) > 10,
            "should have C functions"
        );
        assert!(
            summary.get(&ExportKind::ObjCClass).copied().unwrap_or(0) > 10,
            "should have ObjC classes"
        );
        assert!(
            summary.get(&ExportKind::CppFunction).copied().unwrap_or(0)
                + summary.get(&ExportKind::CppMethod).copied().unwrap_or(0)
                > 0,
            "should have C++ symbols"
        );

        // C++ should be demangled
        let cpp: Vec<_> = exports
            .iter()
            .filter(|e| e.kind == ExportKind::CppFunction || e.kind == ExportKind::CppMethod)
            .collect();
        if !cpp.is_empty() {
            assert!(
                !cpp[0].name.starts_with("__Z"),
                "C++ should be demangled, got: {}",
                cpp[0].name
            );
        }

        eprintln!(
            "Metal: {} total exports, breakdown: {:?}",
            exports.len(),
            summary
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_dyld_info_private_framework() {
        let path = "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";
        let exports = dyld_info_exports(path);
        assert!(
            exports.len() > 100,
            "SkyLight should have 100+ exports, got {}",
            exports.len()
        );
        let sls: Vec<_> = exports
            .iter()
            .filter(|e| e.name.starts_with("SLS"))
            .collect();
        assert!(!sls.is_empty(), "SkyLight should have SLS* functions");
    }
}
