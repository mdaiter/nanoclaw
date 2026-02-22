//! Layer 4: Protocol discovery via LLM-guided dynamic probing.
//!
//! Pipeline:
//!   1. Group exports by prefix → logical API clusters
//!   2. LLM sketches an initialization protocol from naming patterns
//!   3. Sandboxed probe validates the sketch, captures crash feedback
//!   4. LLM refines based on errors (up to N iterations)
//!   5. Validated protocol stored in KB as ProtocolEntry
//!
//! Example: MTLCompiler has ~20 exports starting with MTLCodeGenService*.
//! Grouping finds the cluster. LLM recognizes Create → Build → Destroy.
//! Probe confirms Create needs CoreGraphics loaded first. Protocol saved.

use crate::kb::FrameworkKB;
use crate::probe::{self, ProbeConfig, ProbeResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Types
// =============================================================================

/// A discovered API protocol: ordered steps to use a function group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolEntry {
    pub framework: String,
    pub group: String, // prefix/cluster name (e.g. "MTLCodeGenService")
    pub prerequisites: Vec<Prerequisite>, // frameworks/libs to load first
    pub steps: Vec<ProtocolStep>, // ordered call sequence
    pub constants: Vec<DiscoveredConstant>, // magic values found during probing
    pub validated: bool, // did the full sequence succeed?
    pub iterations: usize, // how many probe rounds it took
    /// Python ctypes recipe showing how to bridge data and call the native API.
    /// Injected into the LLM prompt so it can generate correct glue code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prerequisite {
    pub kind: PrereqKind,
    pub path: String,
    pub reason: String,
}

/// Embed variant display strings directly in the enum.
macro_rules! prereq_kind {
  ($( $variant:ident => $s:literal ),+ $(,)?) => {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub enum PrereqKind { $( $variant ),+ }
    impl PrereqKind {
      pub fn as_str(&self) -> &'static str { match self { $( Self::$variant => $s ),+ } }
    }
  };
}

prereq_kind!(
  FrameworkLoad => "framework_load",
  DylibLoad     => "dylib_load",
  ServiceCreate => "service_create",
  DevicePath    => "device_path",
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStep {
    pub order: usize,
    pub function: String,
    pub role: StepRole,
    pub args_hint: String, // e.g. "b\"mcgyver\"" or "None"
    pub returns: String,   // e.g. "c_void_p handle" or "void"
    pub notes: String,
}

macro_rules! step_roles {
  ($( $variant:ident => $s:literal ),+ $(,)?) => {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub enum StepRole { $( $variant ),+ }
    impl StepRole {
      pub fn as_str(&self) -> &'static str { match self { $( Self::$variant => $s ),+ } }
    }
  };
}

step_roles!(
  Init    => "init",
  Create  => "create",
  Use     => "use",
  Destroy => "destroy",
  Query   => "query",
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredConstant {
    pub name: String,
    pub value: String,
    pub source: String, // "error message", "binary string", "LLM inference"
}

impl ProtocolEntry {
    /// Create a protocol for a single stateless function (no create/destroy lifecycle).
    pub fn stateless(framework: &str, function: &str, args_hint: &str, returns: &str) -> Self {
        Self {
            framework: framework.into(),
            group: function.into(),
            prerequisites: vec![],
            steps: vec![ProtocolStep {
                order: 0,
                function: function.into(),
                role: StepRole::Use,
                args_hint: args_hint.into(),
                returns: returns.into(),
                notes: String::new(),
            }],
            constants: vec![],
            validated: true,
            iterations: 0,
            recipe: None,
        }
    }

    /// Whether all steps reference C-linkage functions (valid Rust identifiers).
    /// Returns false for protocols with ObjC selectors (contain `:`) or other
    /// names that cannot appear in an `extern "C"` block.
    pub fn is_c_ffi_compatible(&self) -> bool {
        self.steps.iter().all(|s| {
            let f = &s.function;
            !f.is_empty()
                && !f.contains(':')
                && !f.contains(' ')
                && f.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
    }

    /// Whether this protocol uses ObjC selectors (method names contain `:`).
    /// Complement of `is_c_ffi_compatible()` for non-empty validated protocols.
    pub fn is_objc_protocol(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().any(|s| s.function.contains(':'))
    }

    /// Whether this protocol is a stateless single-call API (no create/destroy lifecycle).
    /// Stateless functions should be called directly via `ctypes.CDLL` — the LLM should
    /// NOT generate class wrappers with create/call/destroy lifecycle for these.
    pub fn is_stateless(&self) -> bool {
        !self.steps.is_empty()
            && !self
                .steps
                .iter()
                .any(|s| matches!(s.role, StepRole::Create | StepRole::Init))
            && !self
                .steps
                .iter()
                .any(|s| matches!(s.role, StepRole::Destroy))
    }

    /// Create a stateless protocol with semantic notes and a buffer bridge recipe.
    pub fn stateless_noted(
        framework: &str,
        function: &str,
        args_hint: &str,
        returns: &str,
        notes: &str,
        recipe: &str,
    ) -> Self {
        Self {
            framework: framework.into(),
            group: function.into(),
            prerequisites: vec![],
            steps: vec![ProtocolStep {
                order: 0,
                function: function.into(),
                role: StepRole::Use,
                args_hint: args_hint.into(),
                returns: returns.into(),
                notes: notes.into(),
            }],
            constants: vec![],
            validated: true,
            iterations: 0,
            recipe: Some(recipe.into()),
        }
    }
}

// =============================================================================
// Grouping: cluster exports by prefix
// =============================================================================

/// Group a framework's exports into logical API clusters by shared prefix.
/// Returns prefix → list of function names.
pub fn group_by_prefix(kb: &FrameworkKB) -> HashMap<String, Vec<String>> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let names: Vec<&str> = kb.functions.keys().map(|s| s.as_str()).collect();

    // Find shared prefixes: try progressively shorter prefixes for each name
    for name in &names {
        let prefix = find_group_prefix(name, &names);
        groups.entry(prefix).or_default().push(name.to_string());
    }

    // Drop singleton groups and groups that are just the framework name
    groups.retain(|prefix, members| members.len() >= 2 && !prefix.is_empty());

    // Sort members within each group
    for members in groups.values_mut() {
        members.sort();
    }
    groups
}

/// Find the best group prefix for a function name among its peers.
fn find_group_prefix(name: &str, all_names: &[&str]) -> String {
    // Try longest common prefix with at least one other name
    let chars: Vec<char> = name.chars().collect();
    let mut best_prefix = String::new();
    let mut best_count = 0;

    // Walk from length 3..name.len(), find the prefix that maximizes group size
    // but stop at word boundaries (uppercase letter after lowercase)
    for end in 3..chars.len() {
        // Only break at word boundaries: end at the char before an uppercase
        if end > 3 && chars[end].is_lowercase() && !chars[end - 1].is_uppercase() {
            continue;
        }
        let candidate: String = chars[..end].iter().collect();
        let count = all_names
            .iter()
            .filter(|n| n.starts_with(&candidate))
            .count();
        if count >= 2 && count > best_count {
            best_prefix = candidate;
            best_count = count;
        }
    }
    best_prefix
}

// =============================================================================
// LLM prompt construction (macro-driven templates)
// =============================================================================

/// Build a prompt string from template fragments. Avoids format!() soup.
macro_rules! prompt {
  ($($part:expr),+ $(,)?) => { [$( $part ),+].join("") };
}

/// Build the initial protocol sketch prompt.
fn sketch_prompt(
    framework: &str,
    group: &str,
    members: &[String],
    signatures: &[(&str, &str)],
) -> String {
    let fn_list: String = members.iter().map(|n| format!("  - {n}\n")).collect();
    let sig_list: String = signatures
        .iter()
        .map(|(n, s)| format!("  - {n}: {s}\n"))
        .collect();
    prompt!(
    "You are analyzing a macOS private framework API group to discover its initialization protocol.\n\n",
    "Framework: ", framework, "\n",
    "API group prefix: ", group, "\n\n",
    "Functions in this group:\n", &fn_list, "\n",
    "Known signatures:\n", &sig_list, "\n",
    "Based on the naming patterns, determine:\n",
    "1. What prerequisite frameworks need to be loaded first (ctypes.CDLL)\n",
    "2. The correct initialization order (Create before Use, Use before Destroy)\n",
    "3. Any likely magic constants or required arguments\n\n",
    "Respond with ONLY a JSON object (no markdown fences):\n",
    "{\n",
    "  \"prerequisites\": [{\"path\": \"/System/Library/Frameworks/X.framework/X\", \"reason\": \"...\"}],\n",
    "  \"steps\": [\n",
    "    {\"function\": \"name\", \"role\": \"create|init|use|query|destroy\", ",
    "\"args\": \"Python ctypes args\", \"returns\": \"return type\", \"notes\": \"...\"}\n",
    "  ],\n",
    "  \"constants\": [{\"name\": \"NAME\", \"value\": \"13\", \"reason\": \"...\"}]\n",
    "}\n",
  )
}

/// Build a refinement prompt after a failed probe.
fn refine_prompt(
    framework: &str,
    group: &str,
    attempt: usize,
    script: &str,
    result: &ProbeResult,
) -> String {
    prompt!(
        "Probe attempt ",
        &attempt.to_string(),
        " for ",
        framework,
        ".",
        group,
        " FAILED.\n\n",
        "Script that was run:\n```python\n",
        script,
        "\n```\n\n",
        "Result: ",
        &result.summary(),
        "\n",
        "Stderr:\n```\n",
        &result.stderr.chars().take(1000).collect::<String>(),
        "\n```\n\n",
        "Fix the protocol. Common issues:\n",
        "- Missing prerequisite framework load (ctypes.CDLL)\n",
        "- Wrong argument types (use ctypes.c_void_p, ctypes.c_char_p, etc.)\n",
        "- Need to create a service/context handle before calling other functions\n",
        "- Magic constant values needed (check error messages for hints)\n\n",
        "Respond with the SAME JSON format as before, corrected.\n",
    )
}

// =============================================================================
// Probe script generation
// =============================================================================

/// Generate a Python probe script from a protocol sketch.
pub fn generate_probe_script(framework: &str, sketch: &ProtocolSketch) -> String {
    let mut lines = vec![
        "import ctypes, sys".to_string(),
        String::new(),
        "# Prerequisites".to_string(),
    ];

    for prereq in &sketch.prerequisites {
        lines.push(format!("ctypes.CDLL(\"{}\")", prereq.path));
    }

    lines.push(String::new());
    lines.push(format!("# Load framework"));
    lines.push(format!("lib = ctypes.CDLL(\"/System/Library/PrivateFrameworks/{framework}.framework/{framework}\")"));

    // Constants
    if !sketch.constants.is_empty() {
        lines.push(String::new());
        lines.push("# Constants".into());
        for c in &sketch.constants {
            lines.push(format!("{} = {}", c.name, c.value));
        }
    }

    // Steps
    lines.push(String::new());
    lines.push("# Protocol steps".into());
    for (i, step) in sketch.steps.iter().enumerate() {
        lines.push(format!(
            "# Step {}: {} ({})",
            i + 1,
            step.function,
            step.role
        ));
        match step.role.as_str() {
            "create" | "init" => {
                lines.push(format!("lib.{}.restype = ctypes.c_void_p", step.function));
                lines.push(format!("handle_{i} = lib.{}({})", step.function, step.args));
                lines.push(format!(
                    "assert handle_{i}, \"{}() returned NULL\"",
                    step.function
                ));
                lines.push(format!(
                    "print(f\"{} ok: handle={{handle_{i}:#x}}\")",
                    step.function
                ));
            }
            "destroy" => {
                let prev = if i > 0 {
                    format!("handle_{}", i - 1)
                } else {
                    "None".into()
                };
                lines.push(format!(
                    "lib.{}({})",
                    step.function,
                    if step.args.is_empty() {
                        &prev
                    } else {
                        &step.args
                    }
                ));
                lines.push(format!("print(\"{}() ok\")", step.function));
            }
            "query" => {
                lines.push(format!("result_{i} = lib.{}({})", step.function, step.args));
                lines.push(format!("print(f\"{} = {{result_{i}}}\")", step.function));
            }
            _ => {
                // Generic "use" step
                let prev = if i > 0 {
                    format!("handle_{}", i - 1)
                } else {
                    "None".into()
                };
                let args = if step.args.is_empty() {
                    &prev
                } else {
                    &step.args
                };
                lines.push(format!("result_{i} = lib.{}({})", step.function, args));
                lines.push(format!("print(f\"{} = {{result_{i}}}\")", step.function));
            }
        }
        lines.push(String::new());
    }

    lines.push("print(\"PROTOCOL_OK\")".into());
    lines.join("\n")
}

// =============================================================================
// LLM sketch parsing
// =============================================================================

/// Intermediate sketch parsed from LLM JSON response.
#[derive(Debug, Clone)]
pub struct ProtocolSketch {
    pub prerequisites: Vec<SketchPrereq>,
    pub steps: Vec<SketchStep>,
    pub constants: Vec<SketchConstant>,
}

#[derive(Debug, Clone)]
pub struct SketchPrereq {
    pub path: String,
    pub reason: String,
}
#[derive(Debug, Clone)]
pub struct SketchStep {
    pub function: String,
    pub role: String,
    pub args: String,
    pub returns: String,
    pub notes: String,
}
#[derive(Debug, Clone)]
pub struct SketchConstant {
    pub name: String,
    pub value: String,
    pub reason: String,
}

/// Extract a JSON string field, returning empty string if missing.
macro_rules! jstr {
    ($obj:expr, $key:literal) => {
        $obj.get($key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
}

/// Parse LLM response (JSON, possibly with markdown fences) into a sketch.
pub fn parse_sketch(response: &str) -> Option<ProtocolSketch> {
    // Strip markdown fences
    let trimmed = response.trim();
    let json_str = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(trimmed)
        .trim();

    let root: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let prerequisites: Vec<SketchPrereq> = root
        .get("prerequisites")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| SketchPrereq {
                    path: jstr!(v, "path"),
                    reason: jstr!(v, "reason"),
                })
                .collect()
        })
        .unwrap_or_default();

    let steps: Vec<SketchStep> = root
        .get("steps")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let function = jstr!(v, "function");
                    if function.is_empty() {
                        return None;
                    }
                    Some(SketchStep {
                        function,
                        role: jstr!(v, "role"),
                        args: jstr!(v, "args"),
                        returns: jstr!(v, "returns"),
                        notes: jstr!(v, "notes"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let constants: Vec<SketchConstant> = root
        .get("constants")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| SketchConstant {
                    name: jstr!(v, "name"),
                    value: jstr!(v, "value"),
                    reason: jstr!(v, "reason"),
                })
                .collect()
        })
        .unwrap_or_default();

    if steps.is_empty() {
        return None;
    }
    Some(ProtocolSketch {
        prerequisites,
        steps,
        constants,
    })
}

/// Convert a validated sketch into a permanent ProtocolEntry.
fn sketch_to_entry(
    framework: &str,
    group: &str,
    sketch: &ProtocolSketch,
    iterations: usize,
    validated: bool,
) -> ProtocolEntry {
    ProtocolEntry {
        framework: framework.into(),
        group: group.into(),
        prerequisites: sketch
            .prerequisites
            .iter()
            .map(|p| Prerequisite {
                kind: if p.path.contains(".framework") {
                    PrereqKind::FrameworkLoad
                } else {
                    PrereqKind::DylibLoad
                },
                path: p.path.clone(),
                reason: p.reason.clone(),
            })
            .collect(),
        steps: sketch
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| ProtocolStep {
                order: i,
                function: s.function.clone(),
                role: match s.role.as_str() {
                    "create" => StepRole::Create,
                    "init" => StepRole::Init,
                    "destroy" => StepRole::Destroy,
                    "query" => StepRole::Query,
                    _ => StepRole::Use,
                },
                args_hint: s.args.clone(),
                returns: s.returns.clone(),
                notes: s.notes.clone(),
            })
            .collect(),
        constants: sketch
            .constants
            .iter()
            .map(|c| DiscoveredConstant {
                name: c.name.clone(),
                value: c.value.clone(),
                source: c.reason.clone(),
            })
            .collect(),
        validated,
        iterations,
        recipe: None,
    }
}

// =============================================================================
// The probe loop
// =============================================================================

/// Configuration for protocol discovery.
pub struct DiscoverConfig {
    pub max_iterations: usize,
    pub probe: ProbeConfig,
}

impl Default for DiscoverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            probe: ProbeConfig::default(),
        }
    }
}

/// Outcome of a discovery attempt, sent to the TUI for display.
#[derive(Debug, Clone)]
pub struct DiscoverResult {
    pub group: String,
    pub protocol: Option<ProtocolEntry>,
    pub history: Vec<ProbeAttempt>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProbeAttempt {
    pub iteration: usize,
    pub script: String,
    pub result: ProbeResult,
}

/// Run the full discovery loop for one API group.
/// Calls `llm_call` for each LLM interaction (injected so TUI can use blocking, tests can mock).
pub fn discover_protocol(
    framework: &str,
    group: &str,
    members: &[String],
    kb: &FrameworkKB,
    config: &DiscoverConfig,
    llm_call: &dyn Fn(&str) -> Result<String, String>,
) -> DiscoverResult {
    let mut history = Vec::new();

    // Build signature context for the LLM
    let signatures: Vec<(&str, &str)> = members
        .iter()
        .filter_map(|name| {
            kb.functions
                .get(name)
                .map(|e| (e.name.as_str(), e.signature.as_str()))
        })
        .collect();

    // Step 1: initial sketch
    let prompt = sketch_prompt(framework, group, members, &signatures);
    let response = match llm_call(&prompt) {
        Ok(r) => r,
        Err(e) => {
            return DiscoverResult {
                group: group.into(),
                protocol: None,
                history,
                error: Some(format!("LLM error: {e}")),
            }
        }
    };

    let mut sketch = match parse_sketch(&response) {
        Some(s) => s,
        None => {
            return DiscoverResult {
                group: group.into(),
                protocol: None,
                history,
                error: Some("Failed to parse LLM sketch".into()),
            }
        }
    };

    // Step 2: probe loop
    for iteration in 0..config.max_iterations {
        let script = generate_probe_script(framework, &sketch);
        let result = probe::run_probe(&script, &config.probe);

        let attempt = ProbeAttempt {
            iteration,
            script: script.clone(),
            result: result.clone(),
        };
        history.push(attempt);

        if result.success && result.stdout.contains("PROTOCOL_OK") {
            // Success — convert to permanent entry
            let entry = sketch_to_entry(framework, group, &sketch, iteration + 1, true);
            return DiscoverResult {
                group: group.into(),
                protocol: Some(entry),
                history,
                error: None,
            };
        }

        // Refine: ask LLM to fix based on error
        if iteration + 1 < config.max_iterations {
            let refine = refine_prompt(framework, group, iteration + 1, &script, &result);
            match llm_call(&refine) {
                Ok(r) => {
                    if let Some(new_sketch) = parse_sketch(&r) {
                        sketch = new_sketch;
                    }
                }
                Err(e) => {
                    return DiscoverResult {
                        group: group.into(),
                        protocol: Some(sketch_to_entry(
                            framework,
                            group,
                            &sketch,
                            iteration + 1,
                            false,
                        )),
                        history,
                        error: Some(format!("LLM refinement error: {e}")),
                    };
                }
            }
        }
    }

    // Exhausted iterations — save best effort
    let entry = sketch_to_entry(framework, group, &sketch, config.max_iterations, false);
    DiscoverResult {
        group: group.into(),
        protocol: Some(entry),
        history,
        error: Some("Max iterations reached".into()),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::crate::kb::ApiEntry;
    use crate::*;

    #[test]
    fn test_parse_sketch_basic() {
        let json = r#"{
      "prerequisites": [{"path": "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", "reason": "required"}],
      "steps": [
        {"function": "FooCreate", "role": "create", "args": "b\"test\"", "returns": "c_void_p", "notes": "creates handle"},
        {"function": "FooDestroy", "role": "destroy", "args": "", "returns": "void", "notes": ""}
      ],
      "constants": [{"name": "REQUEST_TYPE", "value": "13", "reason": "compile request"}]
    }"#;
        let sketch = parse_sketch(json).unwrap();
        assert_eq!(sketch.prerequisites.len(), 1);
        assert_eq!(sketch.steps.len(), 2);
        assert_eq!(sketch.steps[0].function, "FooCreate");
        assert_eq!(sketch.constants.len(), 1);
    }

    #[test]
    fn test_parse_sketch_with_fences() {
        let fenced = "```json\n{\"steps\": [{\"function\": \"Bar\", \"role\": \"use\", \"args\": \"\", \"returns\": \"\", \"notes\": \"\"}]}\n```";
        let sketch = parse_sketch(fenced).unwrap();
        assert_eq!(sketch.steps.len(), 1);
    }

    #[test]
    fn test_parse_sketch_empty_steps_fails() {
        let json = r#"{"steps": [], "prerequisites": []}"#;
        assert!(parse_sketch(json).is_none());
    }

    #[test]
    fn test_group_by_prefix() {
        let mut kb = FrameworkKB::new("TestFW");
        for name in [
            "FooCreate",
            "FooDestroy",
            "FooGetCount",
            "BarInit",
            "BarRelease",
            "Standalone",
        ] {
            kb.insert(ApiEntry::new_basic(
                name,
                "TestFW",
                "void",
                &format!("void {name}()"),
                vec![],
            ));
        }
        let groups = group_by_prefix(&kb);
        assert!(groups.contains_key("Foo"), "should have Foo group");
        assert!(groups.contains_key("Bar"), "should have Bar group");
        assert_eq!(groups["Foo"].len(), 3);
        assert_eq!(groups["Bar"].len(), 2);
        assert!(
            !groups
                .values()
                .any(|v| v.contains(&"Standalone".to_string())),
            "singletons should be dropped"
        );
    }

    #[test]
    fn test_generate_probe_script() {
        let sketch = ProtocolSketch {
            prerequisites: vec![SketchPrereq {
                path: "/System/Library/Frameworks/CG.framework/CG".into(),
                reason: "needed".into(),
            }],
            steps: vec![
                SketchStep {
                    function: "SvcCreate".into(),
                    role: "create".into(),
                    args: "b\"test\"".into(),
                    returns: "c_void_p".into(),
                    notes: String::new(),
                },
                SketchStep {
                    function: "SvcDestroy".into(),
                    role: "destroy".into(),
                    args: String::new(),
                    returns: "void".into(),
                    notes: String::new(),
                },
            ],
            constants: vec![SketchConstant {
                name: "MAGIC".into(),
                value: "42".into(),
                reason: "test".into(),
            }],
        };
        let script = generate_probe_script("TestFW", &sketch);
        assert!(script.contains("ctypes.CDLL(\"/System/Library/Frameworks/CG.framework/CG\")"));
        assert!(script.contains("SvcCreate"));
        assert!(script.contains("SvcDestroy"));
        assert!(script.contains("MAGIC = 42"));
        assert!(script.contains("PROTOCOL_OK"));
    }

    #[test]
    fn test_discover_with_mock_llm() {
        // Mock LLM that returns a simple protocol
        let mock_llm = |_prompt: &str| -> Result<String, String> {
            Ok(r#"{"steps": [{"function": "TestFunc", "role": "query", "args": "", "returns": "int", "notes": ""}], "prerequisites": []}"#.into())
        };

        let mut kb = FrameworkKB::new("TestFW");
        kb.insert(ApiEntry::new_basic(
            "TestFunc",
            "TestFW",
            "int",
            "int TestFunc()",
            vec![],
        ));

        let result = discover_protocol(
            "TestFW",
            "Test",
            &["TestFunc".into()],
            &kb,
            &DiscoverConfig {
                max_iterations: 1,
                probe: ProbeConfig {
                    timeout: std::time::Duration::from_secs(3),
                    sandbox: false,
                },
            },
            &mock_llm,
        );

        // The probe will fail (TestFW doesn't exist) but the loop should run
        assert_eq!(result.group, "Test");
        assert!(
            !result.history.is_empty(),
            "should have at least one probe attempt"
        );
    }

    #[test]
    fn test_stateless_protocol() {
        let proto = ProtocolEntry::stateless(
      "Accelerate", "vImageScale_ARGB8888",
      "src: *const vImage_Buffer, dest: *mut vImage_Buffer, tempBuffer: *mut c_void, flags: u32",
      "i64",
    );
        assert_eq!(proto.framework, "Accelerate");
        assert_eq!(proto.group, "vImageScale_ARGB8888");
        assert!(proto.validated);
        assert_eq!(proto.iterations, 0);
        assert_eq!(proto.steps.len(), 1);
        assert_eq!(proto.steps[0].role, StepRole::Use);
        assert_eq!(proto.steps[0].function, "vImageScale_ARGB8888");
        assert!(proto.prerequisites.is_empty());
        assert!(proto.constants.is_empty());
    }

    #[test]
    fn test_sketch_to_entry() {
        let sketch = ProtocolSketch {
            prerequisites: vec![SketchPrereq {
                path: "/System/Library/Frameworks/X.framework/X".into(),
                reason: "test".into(),
            }],
            steps: vec![SketchStep {
                function: "Create".into(),
                role: "create".into(),
                args: "".into(),
                returns: "void *".into(),
                notes: "".into(),
            }],
            constants: vec![],
        };
        let entry = sketch_to_entry("FW", "Grp", &sketch, 2, true);
        assert_eq!(entry.framework, "FW");
        assert_eq!(entry.group, "Grp");
        assert!(entry.validated);
        assert_eq!(entry.iterations, 2);
        assert_eq!(entry.prerequisites[0].kind, PrereqKind::FrameworkLoad);
        assert_eq!(entry.steps[0].role, StepRole::Create);
    }

    #[test]
    fn test_is_c_ffi_compatible() {
        // Normal C functions are compatible
        let proto = ProtocolEntry::stateless("Accelerate", "vImageScale_ARGB8888", "", "i64");
        assert!(proto.is_c_ffi_compatible());

        // ObjC selectors with colons are NOT compatible
        let mut objc_proto =
            ProtocolEntry::stateless("Metal", "MTLCreateSystemDefaultDevice", "", "void *");
        objc_proto.steps.push(ProtocolStep {
            order: 1,
            function: "newLibraryWithSource:options:error:".into(),
            role: StepRole::Create,
            args_hint: String::new(),
            returns: "void *".into(),
            notes: String::new(),
        });
        assert!(!objc_proto.is_c_ffi_compatible());

        // Mixed: one C function + one ObjC selector → not compatible
        assert!(!objc_proto.is_c_ffi_compatible());

        // Function names with spaces are not compatible
        let mut space_proto = ProtocolEntry::stateless("Test", "valid_fn", "", "void");
        space_proto.steps[0].function = "not valid".into();
        assert!(!space_proto.is_c_ffi_compatible());
    }

    #[test]
    fn test_is_stateless() {
        // Single "Use" step → stateless
        let stateless = ProtocolEntry::stateless("Accelerate", "vImageScale_ARGB8888", "", "i64");
        assert!(
            stateless.is_stateless(),
            "single Use step should be stateless"
        );

        // Create + Destroy → lifecycle (not stateless)
        let lifecycle = ProtocolEntry {
            framework: "VideoToolbox".into(),
            group: "VtDecompressSession".into(),
            prerequisites: vec![],
            steps: vec![
                ProtocolStep {
                    order: 0,
                    function: "VTDecompressionSessionCreate".into(),
                    role: StepRole::Create,
                    args_hint: String::new(),
                    returns: "OSStatus".into(),
                    notes: String::new(),
                },
                ProtocolStep {
                    order: 1,
                    function: "VTDecompressionSessionDecodeFrame".into(),
                    role: StepRole::Use,
                    args_hint: String::new(),
                    returns: "OSStatus".into(),
                    notes: String::new(),
                },
                ProtocolStep {
                    order: 2,
                    function: "VTDecompressionSessionInvalidate".into(),
                    role: StepRole::Destroy,
                    args_hint: String::new(),
                    returns: "void".into(),
                    notes: String::new(),
                },
            ],
            constants: vec![],
            validated: true,
            iterations: 1,
            recipe: None,
        };
        assert!(
            !lifecycle.is_stateless(),
            "Create+Destroy should not be stateless"
        );

        // Init without Destroy → still lifecycle (has Init)
        let init_only = ProtocolEntry {
            framework: "Test".into(),
            group: "InitOnly".into(),
            prerequisites: vec![],
            steps: vec![
                ProtocolStep {
                    order: 0,
                    function: "TestInit".into(),
                    role: StepRole::Init,
                    args_hint: String::new(),
                    returns: "void".into(),
                    notes: String::new(),
                },
                ProtocolStep {
                    order: 1,
                    function: "TestUse".into(),
                    role: StepRole::Use,
                    args_hint: String::new(),
                    returns: "int".into(),
                    notes: String::new(),
                },
            ],
            constants: vec![],
            validated: true,
            iterations: 0,
            recipe: None,
        };
        assert!(
            !init_only.is_stateless(),
            "Init step should not be stateless"
        );

        // Empty steps → not stateless (degenerate case)
        let empty = ProtocolEntry {
            framework: "Test".into(),
            group: "Empty".into(),
            prerequisites: vec![],
            steps: vec![],
            constants: vec![],
            validated: true,
            iterations: 0,
            recipe: None,
        };
        assert!(!empty.is_stateless(), "empty steps should not be stateless");
    }

    #[test]
    fn test_is_objc_protocol() {
        // ObjC protocol: steps with selectors containing `:`
        let mut objc = ProtocolEntry::stateless("AVFoundation", "AVAssetWriter", "", "void *");
        objc.steps = vec![
            ProtocolStep {
                order: 0,
                function: "initWithURL:fileType:".into(),
                role: StepRole::Init,
                args_hint: "url, file_type".into(),
                returns: "id".into(),
                notes: "ObjC init".into(),
            },
            ProtocolStep {
                order: 1,
                function: "startWriting".into(),
                role: StepRole::Use,
                args_hint: String::new(),
                returns: "BOOL".into(),
                notes: String::new(),
            },
        ];
        objc.validated = true;
        assert!(objc.is_objc_protocol());
        assert!(!objc.is_c_ffi_compatible());

        // Pure C-FFI protocol: no ObjC selectors
        let c_ffi = ProtocolEntry::stateless("Accelerate", "vImageScale", "src, dest", "i64");
        assert!(!c_ffi.is_objc_protocol());
        assert!(c_ffi.is_c_ffi_compatible());

        // Empty steps: not ObjC
        let empty = ProtocolEntry {
            framework: "Test".into(),
            group: "Empty".into(),
            prerequisites: vec![],
            steps: vec![],
            constants: vec![],
            validated: true,
            iterations: 0,
            recipe: None,
        };
        assert!(!empty.is_objc_protocol());
    }
}
