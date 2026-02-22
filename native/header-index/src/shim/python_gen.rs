//! Python ctypes consumer generation.

use super::types::*;
use crate::kb::FrameworkKB;
use crate::protocol::{ProtocolEntry, ProtocolStep};

/// Generate Python ctypes wrapper for the shim.
pub(super) fn generate_python(proto: &ProtocolEntry, lib_name: &str, kb: &FrameworkKB) -> String {
    let is_stateless = create_step(proto).is_none() && !has_destroy(proto);
    if is_stateless {
        generate_python_stateless(proto, lib_name, kb)
    } else {
        generate_python_lifecycle(proto, lib_name, kb)
    }
}

/// Generate Python wrapper for stateless protocols (direct function calls, no class lifecycle).
fn generate_python_stateless(proto: &ProtocolEntry, lib_name: &str, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(512);
    let group = &proto.group;
    let mod_name = snake_case(group);

    // Header + CDLL load
    out.push_str(&format!(
        "\"\"\"Auto-generated Python bindings for {}.{} (stateless)\"\"\"\n",
        proto.framework, group
    ));
    out.push_str("import ctypes\nimport pathlib\n\n");
    out.push_str(&format!(
        "_lib = ctypes.CDLL(str(pathlib.Path(__file__).parent / \"lib{lib_name}.dylib\"))\n\n"
    ));

    // Function declarations (no ctx pointer for stateless)
    for step in use_steps(proto) {
        let method = method_name(step, group);
        out.push_str(&format!(
            "_lib.{mod_name}_{method}.restype = {}\n",
            resolve_ctypes(&step.returns, kb)
        ));
        let at = py_stateless_argtypes(step, kb);
        if !at.is_empty() {
            out.push_str(&format!("_lib.{mod_name}_{method}.argtypes = [{at}]\n"));
        }
    }
    out.push('\n');

    // Module-level functions (no class needed)
    for step in use_steps(proto) {
        let method = method_name(step, group);
        let (params, args) = py_named_params_and_args(step, kb);
        // Strip leading ", " from params for standalone function
        let fn_params = if params.starts_with(", ") {
            &params[2..]
        } else {
            &params
        };
        let fn_args = if args.starts_with(", ") {
            &args[2..]
        } else {
            &args
        };
        out.push_str(&format!("def {method}({fn_params}):\n"));
        if !step.notes.is_empty() {
            out.push_str(&format!("  \"\"\"{}\"\"\"  # noqa\n", step.notes));
        }
        out.push_str(&format!("  return _lib.{mod_name}_{method}({fn_args})\n\n"));
    }

    // Also provide a class wrapper for API compatibility (no-op lifecycle)
    out.push_str(&format!("class {group}:\n"));
    out.push_str("  \"\"\"Stateless wrapper — no lifecycle, methods call directly.\"\"\"\n");
    out.push_str("  def __init__(self): pass\n\n");

    for step in use_steps(proto) {
        let method = method_name(step, group);
        let (params, args) = py_named_params_and_args(step, kb);
        let fn_args = if args.starts_with(", ") {
            &args[2..]
        } else {
            &args
        };
        out.push_str(&format!("  def {method}(self{params}):\n"));
        out.push_str(&format!(
            "    return _lib.{mod_name}_{method}({fn_args})\n\n"
        ));
    }

    out.push_str("  def close(self): pass\n");
    out.push_str("  def __enter__(self): return self\n");
    out.push_str("  def __exit__(self, *a): pass\n");

    out
}

/// Generate Python wrapper for lifecycle protocols (create/use/destroy pattern).
fn generate_python_lifecycle(proto: &ProtocolEntry, lib_name: &str, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(512);
    let group = &proto.group;
    let mod_name = snake_case(group);

    // Header + CDLL load
    out.push_str(&format!(
        "\"\"\"Auto-generated Python bindings for {}.{}\"\"\"\n",
        proto.framework, group
    ));
    out.push_str("import ctypes\nimport pathlib\n\n");
    out.push_str(&format!(
        "_lib = ctypes.CDLL(str(pathlib.Path(__file__).parent / \"lib{lib_name}.dylib\"))\n\n"
    ));

    // Function declarations
    if let Some(step) = create_step(proto) {
        out.push_str(&format!(
            "_lib.{mod_name}_create.restype = ctypes.c_void_p\n"
        ));
        let at = py_argtypes(step);
        if !at.is_empty() {
            out.push_str(&format!("_lib.{mod_name}_create.argtypes = [{at}]\n"));
        }
    }
    for step in use_steps(proto) {
        let method = method_name(step, group);
        out.push_str(&format!(
            "_lib.{mod_name}_{method}.restype = {}\n",
            resolve_ctypes(&step.returns, kb)
        ));
        let at = format!("ctypes.c_void_p{}", py_extra_argtypes(step));
        out.push_str(&format!("_lib.{mod_name}_{method}.argtypes = [{at}]\n"));
    }
    if has_destroy(proto) {
        out.push_str(&format!("_lib.{mod_name}_destroy.restype = None\n"));
        out.push_str(&format!(
            "_lib.{mod_name}_destroy.argtypes = [ctypes.c_void_p]\n"
        ));
    }
    out.push('\n');

    // Class definition
    out.push_str(&format!("class {group}:\n"));
    out.push_str(&format!("  def __init__(self{}):\n", py_init_params(proto)));
    out.push_str(&format!(
        "    self._ptr = _lib.{mod_name}_create({})\n",
        py_init_args(proto)
    ));
    out.push_str("    if not self._ptr:\n");
    out.push_str(&format!(
        "      raise RuntimeError(\"{group}: create failed\")\n\n"
    ));

    for step in use_steps(proto) {
        let method = method_name(step, group);
        let (params, args) = py_named_params_and_args(step, kb);
        out.push_str(&format!("  def {method}(self{params}):\n"));
        if !step.notes.is_empty() {
            out.push_str(&format!("    \"\"\"{}\"\"\"  # noqa\n", step.notes));
        }
        out.push_str(&format!(
            "    return _lib.{mod_name}_{method}(self._ptr{args})\n\n"
        ));
    }

    out.push_str("  def close(self):\n    if self._ptr:\n");
    out.push_str(&format!(
        "      _lib.{mod_name}_destroy(self._ptr)\n      self._ptr = None\n\n"
    ));
    out.push_str("  def __enter__(self): return self\n");
    out.push_str("  def __exit__(self, *a): self.close()\n");
    out.push_str("  def __del__(self): self.close()\n");

    out
}

// =============================================================================
// Python-specific param builders
// =============================================================================

fn py_argtypes(step: &ProtocolStep) -> String {
    if step.args_hint.is_empty() {
        return String::new();
    }
    step.args_hint
        .split(',')
        .map(|arg| {
            let arg = arg.trim();
            if arg.starts_with("b\"") || arg.starts_with("b'") {
                "ctypes.c_char_p"
            } else if arg == "handle" {
                "ctypes.c_void_p"
            } else {
                "ctypes.c_int"
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Argtypes for stateless function (no ctx pointer — all args are real params).
fn py_stateless_argtypes(step: &ProtocolStep, kb: &FrameworkKB) -> String {
    let parsed = parse_step_args(step, kb);
    if parsed.is_empty() {
        return String::new();
    }
    parsed
        .iter()
        .map(|(_, ty)| resolve_ctypes(ty, kb))
        .collect::<Vec<_>>()
        .join(", ")
}

fn py_extra_argtypes(step: &ProtocolStep) -> String {
    if step.args_hint.is_empty() {
        return String::new();
    }
    let args: Vec<_> = step
        .args_hint
        .split(',')
        .filter(|arg| arg.trim() != "handle")
        .map(|arg| {
            if arg.trim().starts_with("b\"") {
                "ctypes.c_char_p"
            } else {
                "ctypes.c_int"
            }
        })
        .collect();
    if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    }
}

fn py_init_params(proto: &ProtocolEntry) -> String {
    let step = match create_step(proto) {
        Some(s) => s,
        None => return String::new(),
    };
    if step.args_hint.is_empty() {
        return String::new();
    }
    step.args_hint
        .split(',')
        .enumerate()
        .map(|(i, arg)| {
            let arg = arg.trim();
            if arg.starts_with("b\"")
                || arg.starts_with("b'")
                || arg == "None"
                || arg == "True"
                || arg == "False"
                || arg.parse::<i64>().is_ok()
            {
                format!(", arg{i}={arg}")
            } else {
                format!(", arg{i}")
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn py_init_args(proto: &ProtocolEntry) -> String {
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

/// Build method parameter names and call args using real C declaration names.
/// Falls back to arg0, arg1, ... for non-C-declaration args.
fn py_named_params_and_args(step: &ProtocolStep, kb: &FrameworkKB) -> (String, String) {
    let parsed = parse_step_args(step, kb);
    let non_handle: Vec<_> = parsed
        .iter()
        .filter(|(name, _)| !is_handle_param(name, step))
        .collect();
    let params: String = non_handle
        .iter()
        .map(|(name, _)| format!(", {name}"))
        .collect::<Vec<_>>()
        .join("");
    let args: String = non_handle
        .iter()
        .map(|(name, _)| format!(", {name}"))
        .collect::<Vec<_>>()
        .join("");
    (params, args)
}
