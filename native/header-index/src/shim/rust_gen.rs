//! Rust FFI shim generation: extern bindings + safe wrapper + C ABI exports.

use super::types::*;
use crate::kb::FrameworkKB;
use crate::protocol::{ProtocolEntry, ProtocolStep};

/// Generate the full Rust source: extern block + safe wrapper + C ABI exports.
pub(super) fn generate_rust(proto: &ProtocolEntry, kb: &FrameworkKB) -> String {
    let is_stateless = create_step(proto).is_none() && !has_destroy(proto);
    if is_stateless {
        generate_rust_stateless(proto, kb)
    } else {
        generate_rust_lifecycle(proto, kb)
    }
}

/// Generate Rust shim for stateless protocols (direct function calls, no struct lifecycle).
fn generate_rust_stateless(proto: &ProtocolEntry, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(2048);
    let group = &proto.group;
    let mod_name = snake_case(group);

    // File header
    out.push_str(&format!(
        "//! Auto-generated FFI shim for {}.{} (stateless)\n",
        proto.framework, group
    ));
    out.push_str("//! Compile: cargo build --release\n");
    out.push_str("//! Output: target/release/lib*.dylib (macOS) or lib*.so (Linux)\n\n");
    out.push_str("#![allow(non_snake_case, non_camel_case_types, unused_imports)]\n\n");
    out.push_str("use std::ffi::c_void;\nuse std::ptr;\n\n");

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

    // --- Layer 1: extern "C" bindings ---
    emit_section(&mut out, "Layer 1: Raw FFI bindings (extern block)");
    for prereq in &proto.prerequisites {
        let lib_name = prereq.path.rsplit('/').next().unwrap_or(&prereq.path);
        out.push_str(&format!(
            "#[link(name = \"{lib_name}\", kind = \"framework\")]\nextern \"C\" {{}}\n\n"
        ));
    }
    if let Some(kind) = link_kind_for(&proto.framework) {
        out.push_str(&format!(
            "#[link(name = \"{}\", kind = \"{kind}\")]\n",
            proto.framework
        ));
    }
    out.push_str("extern \"C\" {\n");
    for step in &proto.steps {
        let params = build_rust_params(step, kb);
        out.push_str(&format!(
            "  fn {}({}) -> {};\n",
            step.function,
            params,
            resolve_rust_type(&step.returns, kb)
        ));
    }
    out.push_str("}\n\n");

    // --- Layer 2: Stateless safe wrappers ---
    emit_section(&mut out, "Layer 2: Safe wrappers (stateless — no struct)");
    for step in use_steps(proto) {
        let method = method_name(step, group);
        let ret = resolve_rust_type(&step.returns, kb);
        let params = build_rust_params(step, kb);
        let args = call_args(step, kb);
        if !step.notes.is_empty() {
            out.push_str(&format!("/// {}\n", step.notes));
        }
        out.push_str(&format!(
            "pub fn {method}({params}) -> {ret} {{\n  unsafe {{ {}({args}) }}\n}}\n\n",
            step.function
        ));
    }

    // --- Layer 3: C ABI exports (same as safe wrappers for stateless) ---
    emit_section(
        &mut out,
        "Layer 3: C ABI exports (consumed by Python/Bun/Ruby/etc.)",
    );
    for step in use_steps(proto) {
        let method = method_name(step, group);
        let ret = resolve_rust_type(&step.returns, kb);
        let params = build_rust_params(step, kb);
        let args = call_args(step, kb);
        out.push_str(&format!(
            "#[no_mangle]\npub extern \"C\" fn {mod_name}_{method}({params}) -> {ret} {{\n"
        ));
        out.push_str(&format!("  unsafe {{ {}({args}) }}\n}}\n\n", step.function));
    }

    out
}

/// Generate Rust shim for lifecycle protocols (create/use/destroy pattern).
fn generate_rust_lifecycle(proto: &ProtocolEntry, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(2048);
    let group = &proto.group;
    let mod_name = snake_case(group);
    let ht = handle_type_resolved(proto, kb);

    // File header
    out.push_str(&format!(
        "//! Auto-generated FFI shim for {}.{}\n",
        proto.framework, group
    ));
    out.push_str("//! Compile: cargo build --release\n");
    out.push_str("//! Output: target/release/lib*.dylib (macOS) or lib*.so (Linux)\n\n");
    out.push_str("#![allow(non_snake_case, non_camel_case_types)]\n\n");
    out.push_str("use std::ffi::c_void;\nuse std::ptr;\n\n");

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

    // --- Layer 1: extern "C" bindings ---
    emit_section(&mut out, "Layer 1: Raw FFI bindings (extern block)");
    for prereq in &proto.prerequisites {
        let lib_name = prereq.path.rsplit('/').next().unwrap_or(&prereq.path);
        out.push_str(&format!(
            "#[link(name = \"{lib_name}\", kind = \"framework\")]\nextern \"C\" {{}}\n\n"
        ));
    }
    if let Some(kind) = link_kind_for(&proto.framework) {
        out.push_str(&format!(
            "#[link(name = \"{}\", kind = \"{kind}\")]\n",
            proto.framework
        ));
    }
    out.push_str("extern \"C\" {\n");
    for step in &proto.steps {
        let params = build_rust_params(step, kb);
        out.push_str(&format!(
            "  fn {}({}) -> {};\n",
            step.function,
            params,
            resolve_rust_type(&step.returns, kb)
        ));
    }
    out.push_str("}\n\n");

    // --- Layer 2: Safe wrapper ---
    emit_section(&mut out, "Layer 2: Safe wrapper with lifecycle management");
    out.push_str(&format!("pub struct {group} {{\n  handle: {ht},\n}}\n\n"));
    out.push_str(&format!("impl {group} {{\n"));

    if let Some(step) = create_step(proto) {
        let params = build_rust_params(step, kb);
        let args = call_args(step, kb);
        out.push_str(&format!(
            "  pub fn new({params}) -> Result<Self, &'static str> {{\n"
        ));
        out.push_str(&format!(
            "    let handle = unsafe {{ {}({args}) }};\n",
            step.function
        ));
        if ht.contains('*') {
            out.push_str(&format!(
                "    if handle.is_null() {{ return Err(\"{}() returned null\"); }}\n",
                step.function
            ));
        }
        out.push_str("    Ok(Self { handle })\n  }\n\n");
    }

    for step in use_steps(proto) {
        let method = method_name(step, group);
        let ret = resolve_rust_type(&step.returns, kb);
        let extra = method_extra_params(step, kb);
        let args = method_call_args(step, kb);
        if !step.notes.is_empty() {
            out.push_str(&format!("  /// {}\n", step.notes));
        }
        out.push_str(&format!("  pub fn {method}(&self{extra}) -> {ret} {{\n"));
        out.push_str(&format!(
            "    unsafe {{ {}({args}) }}\n  }}\n\n",
            step.function
        ));
    }

    if let Some(step) = destroy_step(proto) {
        out.push_str("  pub fn destroy(&mut self) {\n");
        if ht.contains('*') {
            out.push_str(&format!("    if !self.handle.is_null() {{\n      unsafe {{ {}(self.handle) }};\n      self.handle = ptr::null_mut();\n    }}\n", step.function));
        } else {
            out.push_str(&format!(
                "    unsafe {{ {}(self.handle) }};\n",
                step.function
            ));
        }
        out.push_str("  }\n");
    }
    out.push_str("}\n\n");

    if has_destroy(proto) {
        out.push_str(&format!(
            "impl Drop for {group} {{\n  fn drop(&mut self) {{\n    self.destroy();\n  }}\n}}\n\n"
        ));
    }

    // --- Layer 3: C ABI exports ---
    emit_section(
        &mut out,
        "Layer 3: C ABI exports (consumed by Python/Bun/Ruby/etc.)",
    );

    if let Some(step) = create_step(proto) {
        let c_params = c_export_params(step, kb);
        let c_args = call_args(step, kb);
        out.push_str(&format!(
            "#[no_mangle]\npub extern \"C\" fn {mod_name}_create({c_params}) -> *mut {group} {{\n"
        ));
        out.push_str(&format!("  match {group}::new({c_args}) {{\n    Ok(ctx) => Box::into_raw(Box::new(ctx)),\n    Err(_) => ptr::null_mut(),\n  }}\n}}\n\n"));
    }

    for step in use_steps(proto) {
        let method = method_name(step, group);
        let ret = resolve_rust_type(&step.returns, kb);
        let c_params = c_export_method_params(step, group, kb);
        let c_args = c_export_method_call_args(step, kb);
        out.push_str(&format!(
            "#[no_mangle]\npub extern \"C\" fn {mod_name}_{method}({c_params}) -> {ret} {{\n"
        ));
        out.push_str(&format!(
            "  let ctx = unsafe {{ &*ctx }};\n  ctx.{method}({c_args})\n}}\n\n"
        ));
    }

    if has_destroy(proto) {
        out.push_str(&format!(
            "#[no_mangle]\npub extern \"C\" fn {mod_name}_destroy(ctx: *mut {group}) {{\n"
        ));
        out.push_str("  if !ctx.is_null() {\n    unsafe { drop(Box::from_raw(ctx)) };\n  }\n}\n\n");
    }

    out
}

/// Generate build.rs that tells cargo where to find frameworks.
pub(super) fn generate_build_rs(proto: &ProtocolEntry) -> String {
    let config = crate::config::platform();
    let mut out = String::with_capacity(256);
    out.push_str("fn main() {\n");
    // Framework search paths from platform config
    for dir in config.framework_dirs() {
        out.push_str(&format!(
            "  println!(\"cargo:rustc-link-search=framework={}\");\n",
            dir
        ));
    }
    // Prerequisite framework search paths
    for prereq in &proto.prerequisites {
        if let Some(dir) = std::path::Path::new(&prereq.path).parent() {
            if let Some(fw_dir) = dir.parent() {
                out.push_str(&format!(
                    "  println!(\"cargo:rustc-link-search=framework={}\");\n",
                    fw_dir.display()
                ));
            }
        }
    }
    out.push_str("}\n");
    out
}

/// Generate Cargo.toml for the shim crate.
pub(super) fn generate_cargo_toml(lib_name: &str, proto: &ProtocolEntry) -> String {
    format!(
        "[package]\n\
     name = \"{lib_name}\"\n\
     version = \"0.1.0\"\n\
     edition = \"2021\"\n\
     # Auto-generated shim for {fw}.{group}\n\n\
     [lib]\n\
     crate-type = [\"cdylib\"]\n",
        fw = proto.framework,
        group = proto.group,
    )
}

// =============================================================================
// Param builders (Rust-specific)
// =============================================================================

fn build_rust_params(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    if args.is_empty() {
        return String::new();
    }
    args.iter()
        .map(|(name, ty)| format!("{name}: {}", resolve_rust_type(ty, kb)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Get argument names for calling the raw FFI function from the safe wrapper constructor.
fn call_args(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    if args.is_empty() {
        return String::new();
    }
    args.iter()
        .map(|(n, _)| n.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Extra params for use/query method signatures (excludes "handle" args).
fn method_extra_params(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    let filtered: Vec<_> = args
        .iter()
        .filter(|(n, _)| !is_handle_param(n, step))
        .collect();
    if filtered.is_empty() {
        return String::new();
    }
    let params = filtered
        .iter()
        .map(|(name, ty)| format!("{name}: {}", resolve_rust_type(ty, kb)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(", {params}")
}

/// Call args for method body: handle args become self.handle, others use param name.
fn method_call_args(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    if args.is_empty() {
        return "self.handle".into();
    }
    args.iter()
        .map(|(n, _)| {
            if is_handle_param(n, step) {
                "self.handle".into()
            } else {
                n.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn c_export_params(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    if args.is_empty() {
        return String::new();
    }
    args.iter()
        .map(|(name, ty)| format!("{name}: {}", resolve_rust_type(ty, kb)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn c_export_method_params(step: &ProtocolStep, group: &str, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    let mut params = vec![format!("ctx: *const {group}")];
    for (name, ty) in &args {
        if is_handle_param(name, step) {
            continue;
        }
        params.push(format!("{name}: {}", resolve_rust_type(ty, kb)));
    }
    params.join(", ")
}

fn c_export_method_call_args(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let args = parse_step_args(step, kb);
    let names: Vec<_> = args
        .iter()
        .filter(|(n, _)| !is_handle_param(n, step))
        .map(|(n, _)| n.as_str())
        .collect();
    names.join(", ")
}

/// Determine the correct link kind for a framework/library name.
/// Most Apple APIs are frameworks, but some (CommonCrypto) are in libSystem.
/// Returns None for system-embedded libraries that need no explicit link.
fn link_kind_for(framework: &str) -> Option<&'static str> {
    match framework {
        // CommonCrypto, zlib, etc. are part of libSystem — already linked
        "CommonCrypto" | "POSIX" => None,
        "zlib" | "sqlite3" | "bz2" | "xml2" | "curl" => Some("dylib"),
        _ => Some("framework"),
    }
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
