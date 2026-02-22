//! C test program generation for native benchmarking.

use super::types::*;
use crate::kb::FrameworkKB;
use crate::protocol::ProtocolEntry;

/// Generate a C `main()` that benchmarks calling the shim's exported functions.
///
/// For stateless protocols (single Use step): calls the function directly N times.
/// For lifecycle protocols: create → use N times → destroy.
///
/// The output prints `{"times": [...]}` JSON to stdout, matching the bench harness format.
pub fn generate_test_program(proto: &ProtocolEntry, _kb: &FrameworkKB, iterations: u32) -> String {
    let mod_name = snake_case(&proto.group);
    let mut out = String::with_capacity(1024);
    out.push_str("#include <stdio.h>\n#include <time.h>\n#include <stdlib.h>\n\n");

    let has_cr = has_create(proto);
    let has_ds = has_destroy(proto);

    // Extern declarations
    if has_cr {
        out.push_str(&format!("extern void* {mod_name}_create();\n"));
    }
    for step in use_steps(proto) {
        let method = method_name(step, &proto.group);
        if has_cr {
            out.push_str(&format!("extern int {mod_name}_{method}(void* ctx);\n"));
        } else {
            out.push_str(&format!("extern int {mod_name}_{method}();\n"));
        }
    }
    if has_ds {
        out.push_str(&format!("extern void {mod_name}_destroy(void* ctx);\n"));
    }
    out.push('\n');

    // main()
    out.push_str("int main() {\n");
    if has_cr {
        out.push_str(&format!("  void* ctx = {mod_name}_create();\n"));
        out.push_str("  if (!ctx) { fprintf(stderr, \"create failed\\n\"); return 1; }\n");
    }
    out.push_str("  printf(\"{\\\"times\\\": [\");\n");
    out.push_str(&format!("  for (int i = 0; i < {iterations}; i++) {{\n"));
    out.push_str("    struct timespec start, end;\n    clock_gettime(CLOCK_MONOTONIC, &start);\n");

    for step in use_steps(proto) {
        let method = method_name(step, &proto.group);
        if has_cr {
            out.push_str(&format!("    {mod_name}_{method}(ctx);\n"));
        } else {
            out.push_str(&format!("    {mod_name}_{method}();\n"));
        }
    }

    out.push_str("    clock_gettime(CLOCK_MONOTONIC, &end);\n");
    out.push_str("    double ms = (end.tv_sec - start.tv_sec) * 1000.0 + (end.tv_nsec - start.tv_nsec) / 1e6;\n");
    out.push_str("    if (i > 0) printf(\",\");\n    printf(\"%.6f\", ms);\n  }\n");
    out.push_str("  printf(\"]}\");\n");
    if has_ds {
        out.push_str(&format!("  {mod_name}_destroy(ctx);\n"));
    }
    out.push_str("  return 0;\n}\n");
    out
}
