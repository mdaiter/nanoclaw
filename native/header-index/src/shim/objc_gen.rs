//! ObjC shim generation: objc2-based Rust code that wraps ObjC selectors behind C ABI.
//!
//! Protocols with ObjC selectors (containing `:`) are not C-FFI-compatible.
//! This module generates Rust code using `objc2::msg_send!` to call ObjC methods,
//! wrapped behind `#[no_mangle] pub extern "C" fn` exports so Python/Bun consumers
//! can still call via ctypes/FFI — the ObjC complexity is hidden inside the Rust shim.

use super::types::*;
use crate::kb::FrameworkKB;
use crate::protocol::{ProtocolEntry, ProtocolStep, StepRole};

// =============================================================================
// Selector parsing
// =============================================================================

/// Parse an ObjC selector into its keyword parts.
///
/// `"initWithContentsOfURL:error:"` → `["initWithContentsOfURL", "error"]`
/// `"count"` → `["count"]`  (unary selector)
pub(super) fn parse_objc_selector(function: &str) -> Vec<String> {
    if !function.contains(':') {
        return vec![function.to_string()];
    }
    function
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// =============================================================================
// ObjC Rust code generation
// =============================================================================

/// Generate objc2-based Rust source for an ObjC protocol.
/// Produces the same 3-layer structure as rust_gen but using objc2 message sends
/// instead of extern "C" blocks.
pub(super) fn generate_objc_rust(proto: &ProtocolEntry, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(4096);
    let group = &proto.group;
    let mod_name = snake_case(group);

    // File header
    out.push_str(&format!(
        "//! Auto-generated ObjC FFI shim for {}.{}\n",
        proto.framework, group
    ));
    out.push_str("//! Uses objc2 message sends wrapped behind C ABI exports.\n");
    out.push_str("//! Compile: cargo build --release\n");
    out.push_str("//! Output: target/release/lib*.dylib (macOS)\n\n");
    out.push_str("#![allow(non_snake_case, non_camel_case_types, unused_unsafe)]\n\n");

    // --- Layer 1: ObjC bridge (replaces extern "C" block) ---
    emit_section(&mut out, "Layer 1: ObjC bridge (objc2 message sends)");
    out.push_str("use objc2::runtime::{AnyClass, AnyObject};\n");
    out.push_str("use objc2::rc::Retained;\n");
    out.push_str("use objc2::{msg_send, class};\n");
    out.push_str("use std::ffi::c_void;\n");
    out.push_str("use std::ptr;\n\n");

    // Constants
    for c in &proto.constants {
        out.push_str(&format!(
            "pub const {}: i64 = {};  // {}\n",
            c.name, c.value, c.source
        ));
    }
    if !proto.constants.is_empty() {
        out.push('\n');
    }

    // Helper: get class by name
    out.push_str("/// Look up an ObjC class by name at runtime.\n");
    out.push_str("fn get_class(name: &str) -> &'static AnyClass {\n");
    out.push_str(
        "  AnyClass::get(name).unwrap_or_else(|| panic!(\"ObjC class '{}' not found\", name))\n",
    );
    out.push_str("}\n\n");

    // --- Layer 2: Safe wrapper ---
    emit_section(&mut out, "Layer 2: Safe wrapper with ObjC object lifecycle");
    out.push_str(&format!("pub struct {group} {{\n"));
    out.push_str("  obj: Retained<AnyObject>,\n");
    out.push_str("}\n\n");

    out.push_str(&format!("impl {group} {{\n"));

    // Constructor from Create/Init step
    if let Some(step) = create_step(proto) {
        let sel_parts = parse_objc_selector(&step.function);
        let args = parse_objc_args(step, kb);
        let params_str = objc_method_params(&args);

        out.push_str(&format!("  /// {} — {}\n", step.function, step.notes));
        out.push_str(&format!(
            "  pub fn new({params_str}) -> Result<Self, &'static str> {{\n"
        ));
        out.push_str(&format!("    let cls = get_class(\"{group}\");\n"));
        out.push_str("    let obj: *mut AnyObject = unsafe { msg_send![cls, alloc] };\n");

        // Build the init msg_send with selector parts + args
        let msg_send_init = build_msg_send("obj", &sel_parts, &args);
        out.push_str(&format!(
            "    let obj: Retained<AnyObject> = unsafe {{ {msg_send_init} }};\n"
        ));
        out.push_str("    Ok(Self { obj })\n");
        out.push_str("  }\n\n");
    }

    // Use/Query methods
    for step in use_steps(proto) {
        let method = method_name(step, group);
        let sel_parts = parse_objc_selector(&step.function);
        let args = parse_objc_args(step, kb);
        let extra_params = objc_method_extra_params(&args);
        let ret = objc_return_type(&step.returns);

        if !step.notes.is_empty() {
            out.push_str(&format!("  /// {}\n", step.notes));
        }
        out.push_str(&format!(
            "  pub fn {method}(&self{extra_params}) -> {ret} {{\n"
        ));
        let msg_send = build_msg_send("&self.obj", &sel_parts, &args);
        out.push_str(&format!("    unsafe {{ {msg_send} }}\n"));
        out.push_str("  }\n\n");
    }

    out.push_str("}\n\n");

    // --- Layer 3: C ABI exports ---
    emit_section(
        &mut out,
        "Layer 3: C ABI exports (consumed by Python/Bun/Ruby/etc.)",
    );

    if has_create(proto) {
        if let Some(step) = create_step(proto) {
            let args = parse_objc_args(step, kb);
            let c_params = objc_c_export_params(&args);
            let c_args = objc_c_call_args(&args);
            out.push_str("#[no_mangle]\n");
            out.push_str(&format!(
                "pub extern \"C\" fn {mod_name}_create({c_params}) -> *mut {group} {{\n"
            ));
            out.push_str(&format!("  match {group}::new({c_args}) {{\n"));
            out.push_str("    Ok(ctx) => Box::into_raw(Box::new(ctx)),\n");
            out.push_str("    Err(_) => ptr::null_mut(),\n");
            out.push_str("  }\n}\n\n");
        }
    }

    for step in use_steps(proto) {
        let method = method_name(step, group);
        let ret = objc_return_type(&step.returns);
        let args = parse_objc_args(step, kb);
        let c_params = objc_c_export_method_params(group, &args);
        let c_args = objc_c_method_call_args(&args);
        out.push_str("#[no_mangle]\n");
        out.push_str(&format!(
            "pub extern \"C\" fn {mod_name}_{method}({c_params}) -> {ret} {{\n"
        ));
        out.push_str("  let ctx = unsafe { &*ctx };\n");
        out.push_str(&format!("  ctx.{method}({c_args})\n"));
        out.push_str("}\n\n");
    }

    out
}

// =============================================================================
// ObjC Cargo.toml generation
// =============================================================================

/// Generate Cargo.toml for an ObjC shim crate (includes objc2 dependencies).
pub(super) fn generate_objc_cargo_toml(lib_name: &str, proto: &ProtocolEntry) -> String {
    format!(
        "[package]\n\
     name = \"{lib_name}\"\n\
     version = \"0.1.0\"\n\
     edition = \"2021\"\n\
     # Auto-generated ObjC shim for {fw}.{group}\n\n\
     [lib]\n\
     crate-type = [\"cdylib\"]\n\n\
     [dependencies]\n\
     objc2 = \"0.6\"\n\
     objc2-foundation = \"0.3\"\n",
        fw = proto.framework,
        group = proto.group,
    )
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Parsed ObjC argument: name + Rust type for the C ABI layer.
struct ObjcArg {
    name: String,
    rust_type: String,
}

/// Parse step args for ObjC methods, filtering out "error" args (which are
/// NSError** out-params handled internally by the shim).
fn parse_objc_args(step: &ProtocolStep, kb: &FrameworkKB) -> Vec<ObjcArg> {
    if step.args_hint.is_empty() {
        return vec![];
    }
    step.args_hint
        .split(',')
        .enumerate()
        .filter(|(_, a)| {
            let trimmed = a.trim();
            // Filter out ObjC error out-params — handled internally
            trimmed != "error" && trimmed != "err"
        })
        .map(|(i, arg)| {
            let arg = arg.trim();
            let rust_type = objc_arg_to_rust_type(arg, kb);
            let name = if arg.chars().all(|c| c.is_alphanumeric() || c == '_') {
                format!("arg_{arg}")
            } else {
                format!("arg{i}")
            };
            ObjcArg { name, rust_type }
        })
        .collect()
}

/// Map an ObjC arg hint to a Rust type for the C ABI boundary.
fn objc_arg_to_rust_type(arg: &str, kb: &FrameworkKB) -> String {
    let arg = arg.trim();
    // Try KB resolution first
    let resolved = kb.resolve_type(arg);
    let mapped = to_rust_type(resolved);
    // If the universal map returned a non-default, use it
    if mapped != "*mut std::ffi::c_void" || arg == "void *" || arg == "c_void_p" {
        return mapped.to_string();
    }
    // ObjC-specific fallbacks
    match arg {
        "url" | "URL" | "path" => "*const std::ffi::c_char".to_string(),
        "features" | "input" | "data" | "options" => "*mut std::ffi::c_void".to_string(),
        _ => "*mut std::ffi::c_void".to_string(),
    }
}

/// Map an ObjC return type to Rust. ObjC methods returning `id` get `*mut c_void`.
fn objc_return_type(returns: &str) -> &'static str {
    match returns.trim() {
        "id" => "*mut c_void",
        "void" => "()",
        "BOOL" | "bool" => "bool",
        "int" => "std::ffi::c_int",
        "NSInteger" | "long" => "isize",
        "NSUInteger" | "unsigned long" => "usize",
        "float" => "f32",
        "double" => "f64",
        _ => "*mut c_void",
    }
}

/// Build a `msg_send!` expression.
///
/// For a selector like `initWithContentsOfURL:error:` with args `[url_arg]`
/// (error is filtered), produces:
/// `msg_send![obj, initWithContentsOfURL: arg_url]`
fn build_msg_send(receiver: &str, sel_parts: &[String], args: &[ObjcArg]) -> String {
    if sel_parts.len() == 1 && args.is_empty() {
        // Unary selector: msg_send![obj, count]
        return format!("msg_send![{receiver}, {}]", sel_parts[0]);
    }

    // Keyword selector: match selector parts to non-error args
    let mut parts = Vec::new();
    let mut arg_idx = 0;
    for (i, sel) in sel_parts.iter().enumerate() {
        // "error" selector keyword gets a null pointer (NSError** out-param)
        if sel == "error" || sel == "err" {
            parts.push(format!("{sel}: ptr::null_mut::<AnyObject>()"));
        } else if arg_idx < args.len() {
            parts.push(format!("{sel}: {}", args[arg_idx].name));
            arg_idx += 1;
        } else if i == 0 && args.is_empty() {
            // First part with no args — just the selector keyword
            parts.push(sel.clone());
        } else {
            parts.push(format!("{sel}: ptr::null_mut::<AnyObject>()"));
        }
    }

    format!("msg_send![{receiver}, {}]", parts.join(" "))
}

/// Build parameter list for the safe wrapper constructor.
fn objc_method_params(args: &[ObjcArg]) -> String {
    if args.is_empty() {
        return String::new();
    }
    args.iter()
        .map(|a| format!("{}: {}", a.name, a.rust_type))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build extra parameter list for use/query methods (prepend ", ").
fn objc_method_extra_params(args: &[ObjcArg]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let params = args
        .iter()
        .map(|a| format!("{}: {}", a.name, a.rust_type))
        .collect::<Vec<_>>()
        .join(", ");
    format!(", {params}")
}

/// C ABI export params for constructor.
fn objc_c_export_params(args: &[ObjcArg]) -> String {
    if args.is_empty() {
        return String::new();
    }
    args.iter()
        .map(|a| format!("{}: {}", a.name, a.rust_type))
        .collect::<Vec<_>>()
        .join(", ")
}

/// C ABI export params for method (includes ctx pointer).
fn objc_c_export_method_params(group: &str, args: &[ObjcArg]) -> String {
    let mut params = vec![format!("ctx: *const {group}")];
    for a in args {
        params.push(format!("{}: {}", a.name, a.rust_type));
    }
    params.join(", ")
}

/// Call args for forwarding to safe wrapper constructor.
fn objc_c_call_args(args: &[ObjcArg]) -> String {
    args.iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Call args for forwarding to safe wrapper method.
fn objc_c_method_call_args(args: &[ObjcArg]) -> String {
    args.iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn emit_section(out: &mut String, title: &str) {
    out.push_str(
        "// =============================================================================\n",
    );
    out.push_str(&format!("// {title}\n"));
    out.push_str(
        "// =============================================================================\n\n",
    );
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_objc_selector_keyword() {
        let parts = parse_objc_selector("initWithContentsOfURL:error:");
        assert_eq!(parts, vec!["initWithContentsOfURL", "error"]);
    }

    #[test]
    fn test_parse_objc_selector_unary() {
        let parts = parse_objc_selector("count");
        assert_eq!(parts, vec!["count"]);
    }

    #[test]
    fn test_parse_objc_selector_single_keyword() {
        let parts = parse_objc_selector("predictionFromFeatures:");
        assert_eq!(parts, vec!["predictionFromFeatures"]);
    }

    #[test]
    fn test_parse_objc_selector_multi() {
        let parts = parse_objc_selector("initWithModelDescription:error:");
        assert_eq!(parts, vec!["initWithModelDescription", "error"]);
    }
}
