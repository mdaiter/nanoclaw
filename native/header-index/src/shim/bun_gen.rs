//! Bun FFI consumer generation.

use super::types::*;
use crate::kb::FrameworkKB;
use crate::protocol::{ProtocolEntry, ProtocolStep};

/// Generate Bun FFI wrapper for the shim.
pub(super) fn generate_bun(proto: &ProtocolEntry, lib_name: &str, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(512);
    let group = &proto.group;
    let mod_name = snake_case(group);

    // Header + dlopen
    out.push_str(&format!(
        "// Auto-generated Bun FFI bindings for {}.{}\n",
        proto.framework, group
    ));
    out.push_str("const { dlopen, FFIType, ptr } = require(\"bun:ffi\");\n");
    out.push_str(&format!(
        "const {{ symbols }} = dlopen(\"./lib{lib_name}.dylib\", {{\n"
    ));

    // Symbol declarations
    if let Some(step) = create_step(proto) {
        out.push_str(&format!(
            "  {mod_name}_create: {{ args: [{}], returns: FFIType.ptr }},\n",
            bun_argtypes(step)
        ));
    }
    for step in use_steps(proto) {
        let method = method_name(step, group);
        let args = format!("FFIType.ptr{}", bun_extra_argtypes(step));
        out.push_str(&format!(
            "  {mod_name}_{method}: {{ args: [{args}], returns: {} }},\n",
            resolve_bun_ffi(&step.returns, kb)
        ));
    }
    if has_destroy(proto) {
        out.push_str(&format!(
            "  {mod_name}_destroy: {{ args: [FFIType.ptr], returns: FFIType.void }},\n"
        ));
    }
    out.push_str("});\n\n");

    // JS class
    out.push_str(&format!("class {group} {{\n"));
    out.push_str(&format!("  constructor({}) {{\n", bun_ctor_params(proto)));
    out.push_str(&format!(
        "    this._ptr = symbols.{mod_name}_create({});\n",
        bun_ctor_params(proto)
    ));
    out.push_str("    if (!this._ptr) throw new Error(\"create failed\");\n  }\n\n");

    for step in use_steps(proto) {
        let method = method_name(step, group);
        let params = bun_method_params(step);
        let args = bun_method_args(step);
        out.push_str(&format!("  {method}({params}) {{\n"));
        out.push_str(&format!(
            "    return symbols.{mod_name}_{method}(this._ptr{args});\n  }}\n\n"
        ));
    }

    out.push_str(&format!("  close() {{\n    if (this._ptr) {{ symbols.{mod_name}_destroy(this._ptr); this._ptr = null; }}\n  }}\n"));
    out.push_str("}\n\n");
    out.push_str(&format!("module.exports = {{ {group} }};\n"));

    out
}

// =============================================================================
// Bun-specific param builders
// =============================================================================

fn bun_argtypes(step: &ProtocolStep) -> String {
    if step.args_hint.is_empty() {
        return String::new();
    }
    step.args_hint
        .split(',')
        .map(|arg| {
            let arg = arg.trim();
            if arg.starts_with("b\"") {
                "FFIType.cstring"
            } else if arg == "handle" {
                "FFIType.ptr"
            } else {
                "FFIType.i32"
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn bun_extra_argtypes(step: &ProtocolStep) -> String {
    let args: Vec<_> = step
        .args_hint
        .split(',')
        .filter(|arg| arg.trim() != "handle")
        .map(|arg| {
            if arg.trim().starts_with("b\"") {
                "FFIType.cstring"
            } else {
                "FFIType.i32"
            }
        })
        .collect();
    if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    }
}

fn bun_ctor_params(proto: &ProtocolEntry) -> String {
    let step = match create_step(proto) {
        Some(s) => s,
        None => return String::new(),
    };
    if step.args_hint.is_empty() {
        return String::new();
    }
    (0..step.args_hint.split(',').count())
        .map(|i| format!("arg{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn bun_method_params(step: &ProtocolStep) -> String {
    non_handle_args(step)
        .map(|(i, _)| format!("arg{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn bun_method_args(step: &ProtocolStep) -> String {
    let args: Vec<_> = non_handle_args(step)
        .map(|(i, _)| format!(", arg{i}"))
        .collect();
    args.join("")
}
