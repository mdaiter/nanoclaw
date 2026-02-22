//! C header generation for shim exports.

use super::types::*;
use crate::kb::FrameworkKB;
use crate::protocol::ProtocolEntry;

/// Generate a C header for the shim's exported API.
pub(super) fn generate_c_header(proto: &ProtocolEntry, kb: &FrameworkKB) -> String {
    let mut out = String::with_capacity(512);
    let group = &proto.group;
    let mod_name = snake_case(group);
    let guard = mod_name.to_uppercase();

    out.push_str(&format!(
        "// Auto-generated C header for {}.{}\n",
        proto.framework, group
    ));
    out.push_str(&format!(
        "#ifndef {guard}_H\n#define {guard}_H\n\n#include <stdint.h>\n\n"
    ));
    out.push_str(&format!("typedef struct {group} {group};\n\n"));

    if let Some(step) = create_step(proto) {
        let params = c_header_params(step, kb);
        out.push_str(&format!("{group}* {mod_name}_create({params});\n"));
    }
    for step in use_steps(proto) {
        let method = method_name(step, group);
        let ret = resolve_c_type(&step.returns, kb);
        let params = c_header_method_params(step, group, kb);
        out.push_str(&format!("{ret} {mod_name}_{method}({params});\n"));
    }
    if has_destroy(proto) {
        out.push_str(&format!("void {mod_name}_destroy({group}* ctx);\n"));
    }

    out.push_str(&format!("\n#endif // {guard}_H\n"));
    out
}

fn c_header_params(step: &crate::protocol::ProtocolStep, kb: &FrameworkKB) -> String {
    if step.args_hint.is_empty() {
        return "void".into();
    }
    let args = parse_step_args(step, kb);
    args.iter()
        .map(|(name, ty)| format!("{} {name}", resolve_c_type(ty, kb)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn c_header_method_params(
    step: &crate::protocol::ProtocolStep,
    group: &str,
    kb: &FrameworkKB,
) -> String {
    let mut params = vec![format!("{group}* ctx")];
    let args = parse_step_args(step, kb);
    for (name, ty) in &args {
        if is_handle_param(name, step) {
            continue;
        }
        params.push(format!("{} {name}", resolve_c_type(ty, kb)));
    }
    params.join(", ")
}
