//! Layer 3: Framework knowledge base.
//!
//! Evidence-based API entries with compound confidence scoring,
//! JSON persistence (~/.mcgyver/kb/), and query engine for the LLM.
//!
//! Pipeline: extract (Layer 1) → infer (Layer 2) → kb (this layer)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::infer::InferredSig;

// =============================================================================
// Evidence: why we believe something about an API
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvidenceKind {
    DyldExport,      // found in dyld_info -exports
    MachoExport,     // found via object crate Mach-O parsing
    ClangAst,        // parsed from header via clang -ast-dump=json
    NamingInference, // inferred from naming conventions
    RuntimeProbe,    // dlsym/dlopen confirmed at runtime
    ProtocolProbe,   // discovered via LLM-guided sandboxed probing (Layer 4)
    Manual,          // hand-curated data
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub confidence: f64, // 0.0–1.0
    pub detail: String,  // e.g. "offset 0x1234" or "matched Create pattern"
}

// =============================================================================
// ApiEntry: a single function/class in the knowledge base
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEntry {
    pub name: String,
    pub framework: String,
    pub return_type: String,
    pub parameters: Vec<ParamEntry>,
    pub capabilities: Vec<String>, // serialized CapabilityTag
    pub evidence: Vec<Evidence>,
    pub signature: String, // C-style rendered signature

    // Enriched fields (populated from binary metadata, not hardcoded)
    #[serde(default)]
    pub verb: String, // semantic action: "create", "decode", "transform", etc.
    #[serde(default)]
    pub input_formats: Vec<String>, // data types consumed (from param types + ObjC encodings)
    #[serde(default)]
    pub output_formats: Vec<String>, // data types produced (from return type + out-params)
    #[serde(default)]
    pub class_hierarchy: Vec<String>, // ObjC/Swift inheritance chain
    #[serde(default)]
    pub protocols: Vec<String>, // ObjC/Swift protocol conformances
    #[serde(default)]
    pub related: Vec<String>, // functions sharing types (same lifecycle group)
    #[serde(default)]
    pub async_pattern: bool, // has completion handler / callback param
    #[serde(default)]
    pub semantic_hints: Vec<String>, // from __cstring: error messages, descriptions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamEntry {
    pub name: String,
    pub type_name: String,
    pub is_pointer: bool,
}

impl ApiEntry {
    /// Construct a minimal ApiEntry (for tests and direct construction).
    pub fn new_basic(
        name: &str,
        framework: &str,
        return_type: &str,
        sig: &str,
        evidence: Vec<Evidence>,
    ) -> Self {
        Self {
            name: name.into(),
            framework: framework.into(),
            return_type: return_type.into(),
            parameters: vec![],
            capabilities: vec![],
            evidence,
            signature: sig.into(),
            verb: String::new(),
            input_formats: vec![],
            output_formats: vec![],
            class_hierarchy: vec![],
            protocols: vec![],
            related: vec![],
            async_pattern: false,
            semantic_hints: vec![],
        }
    }

    /// Compound confidence: 1 - Π(1 - individual_confidence).
    /// Multiple independent evidence sources compound upward.
    pub fn confidence(&self) -> f64 {
        if self.evidence.is_empty() {
            return 0.0;
        }
        let product: f64 = self.evidence.iter().map(|e| 1.0 - e.confidence).product();
        1.0 - product
    }

    /// Build from an InferredSig + framework name + evidence.
    pub fn from_inferred(sig: &InferredSig, framework: &str, evidence: Vec<Evidence>) -> Self {
        let capabilities: Vec<String> = crate::infer::infer_capabilities(&sig.name)
            .into_iter()
            .map(|c| c.as_str().to_string())
            .collect();
        let verb = crate::enrich::extract_verb(&sig.name).to_string();
        let async_pattern = sig.parameters.iter().any(|p| {
            p.type_name.contains("callback")
                || p.type_name.contains("handler")
                || p.type_name.contains("block")
                || p.type_name.contains("(^)")
                || p.type_name.contains("completion")
        });
        let input_formats: Vec<String> = sig
            .parameters
            .iter()
            .filter(|p| p.type_name != "void" && p.name != "ref")
            .map(|p| {
                if p.is_pointer {
                    format!("{} *", p.type_name)
                } else {
                    p.type_name.clone()
                }
            })
            .collect();
        let output_formats = if sig.return_type == "void" {
            vec![]
        } else {
            vec![sig.return_type.clone()]
        };

        Self {
            name: sig.name.clone(),
            framework: framework.into(),
            return_type: sig.return_type.clone(),
            parameters: sig
                .parameters
                .iter()
                .map(|p| ParamEntry {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                    is_pointer: p.is_pointer,
                })
                .collect(),
            capabilities,
            evidence,
            signature: sig.to_c_signature(),
            verb,
            input_formats,
            output_formats,
            class_hierarchy: vec![],
            protocols: vec![],
            related: vec![],
            async_pattern,
            semantic_hints: vec![],
        }
    }
}

// =============================================================================
// FrameworkKB: per-framework knowledge base
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkKB {
    pub framework: String,
    pub functions: HashMap<String, ApiEntry>,
    /// capability tag → list of function names
    #[serde(default)]
    pub capabilities: HashMap<String, Vec<String>>,
    /// discovered API protocols (Layer 4)
    #[serde(default)]
    pub protocols: Vec<crate::protocol::ProtocolEntry>,
    /// framework-specific type aliases: custom C type → universal C type
    /// e.g. "vImage_Error" → "ssize_t", "OSStatus" → "int32_t"
    /// Resolved before the universal type maps in shim generation.
    #[serde(default)]
    pub type_aliases: HashMap<String, String>,
}

impl FrameworkKB {
    pub fn new(framework: &str) -> Self {
        Self {
            framework: framework.into(),
            functions: HashMap::new(),
            capabilities: HashMap::new(),
            protocols: Vec::new(),
            type_aliases: HashMap::new(),
        }
    }

    /// Register framework-specific type aliases in bulk.
    /// Each pair maps a custom C typedef to a universal C type.
    pub fn add_type_aliases(&mut self, aliases: &[(&str, &str)]) {
        for &(custom, universal) in aliases {
            self.type_aliases.insert(custom.into(), universal.into());
        }
    }

    /// Resolve a type string through this KB's aliases.
    /// Returns the universal type if an alias exists, otherwise the input unchanged.
    pub fn resolve_type<'a>(&'a self, ty: &'a str) -> &'a str {
        self.type_aliases.get(ty).map(|s| s.as_str()).unwrap_or(ty)
    }

    /// Insert an API entry, updating the capability index.
    pub fn insert(&mut self, entry: ApiEntry) {
        for cap in &entry.capabilities {
            self.capabilities
                .entry(cap.clone())
                .or_default()
                .push(entry.name.clone());
        }
        self.functions.insert(entry.name.clone(), entry);
    }

    /// Apply binary enrichment data: class hierarchies, protocols, semantic hints, verbs.
    /// Returns the number of entries enriched.
    pub fn apply_enrichment(&mut self, enrichment: &crate::enrich::BinaryEnrichment) -> usize {
        let hierarchy = crate::enrich::build_class_hierarchy(&enrichment.objc_classes);
        let proto_map = crate::enrich::build_protocol_map(&enrichment.swift_conformances);
        let string_hints = crate::enrich::semantic_hints_from_strings(&enrichment.string_constants);
        let mut count = 0;

        for entry in self.functions.values_mut() {
            let mut changed = false;

            // Apply verb if not already set
            if entry.verb.is_empty() || entry.verb == "unknown" {
                let verb = crate::enrich::extract_verb(&entry.name);
                if verb != "unknown" {
                    entry.verb = verb.to_string();
                    changed = true;
                }
            }

            // Apply ObjC class hierarchy
            if entry.class_hierarchy.is_empty() {
                // Check if entry name matches an ObjC class
                for class in &enrichment.objc_classes {
                    if entry.name.contains(&class.name)
                        || entry.name.starts_with(&format!("+[{}", class.name))
                        || entry.name.starts_with(&format!("-[{}", class.name))
                    {
                        if let Some(chain) = hierarchy.get(&class.name) {
                            entry.class_hierarchy = chain.clone();
                            changed = true;
                        }
                        if !class.protocols.is_empty() {
                            entry.protocols.extend(class.protocols.iter().cloned());
                            changed = true;
                        }
                        break;
                    }
                }
            }

            // Apply Swift protocol conformances
            if entry.protocols.is_empty() {
                for (type_name, protos) in &proto_map {
                    if entry.name.contains(type_name) {
                        entry.protocols = protos.clone();
                        changed = true;
                        break;
                    }
                }
            }

            // Apply semantic hints from strings
            for (bucket, strings) in &string_hints {
                if entry.capabilities.iter().any(|c| c.contains(bucket)) {
                    entry.semantic_hints.extend(strings.iter().take(3).cloned());
                    changed = true;
                }
            }

            // Check for async patterns in ObjC methods
            if !entry.async_pattern {
                for class in &enrichment.objc_classes {
                    for method in class.instance_methods.iter().chain(&class.class_methods) {
                        if method.selector == entry.name || entry.name.contains(&method.selector) {
                            if method.param_types.iter().any(|t| {
                                t.contains("(^)") || t.contains("block") || t.contains("handler")
                            }) {
                                entry.async_pattern = true;
                                changed = true;
                            }
                            // Upgrade param types from ObjC type encoding (more precise than inference)
                            if !method.param_types.is_empty() && entry.input_formats.is_empty() {
                                entry.input_formats = method.param_types.clone();
                                changed = true;
                            }
                            if method.return_type != "void" && entry.output_formats.is_empty() {
                                entry.output_formats = vec![method.return_type.clone()];
                                changed = true;
                            }
                        }
                    }
                }
            }

            if changed {
                count += 1;
            }
        }

        // Build related functions: group by shared input/output types
        let type_groups = self.build_type_groups();
        for (_, group) in &type_groups {
            if group.len() >= 2 {
                for name in group {
                    if let Some(entry) = self.functions.get_mut(name) {
                        entry.related = group
                            .iter()
                            .filter(|n| *n != name)
                            .take(5)
                            .cloned()
                            .collect();
                    }
                }
            }
        }

        count
    }

    /// Group functions by shared parameter/return types.
    fn build_type_groups(&self) -> HashMap<String, Vec<String>> {
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for entry in self.functions.values() {
            for fmt in entry.input_formats.iter().chain(&entry.output_formats) {
                // Normalize: strip pointer markers for grouping
                let key = fmt.replace(" *", "").replace('*', "");
                if key != "void" && key != "int" && key != "bool" && key.len() > 3 {
                    groups.entry(key).or_default().push(entry.name.clone());
                }
            }
        }
        groups
    }

    /// Apply feedback from a validated protocol: upgrade return types and add evidence.
    /// Returns the number of entries updated.
    pub fn apply_protocol_feedback(&mut self, proto: &crate::protocol::ProtocolEntry) -> usize {
        if !proto.validated {
            return 0;
        }
        let mut updated = 0;
        for step in &proto.steps {
            if step.returns.is_empty() || step.returns == "void" {
                continue;
            }
            if let Some(entry) = self.functions.get_mut(&step.function) {
                // Map ctypes return types to C types
                let c_type = ctypes_to_c(&step.returns);
                entry.return_type = c_type.clone();
                // Re-render signature: replace old return type (with or without /*?*/) with the confirmed type
                if let Some(name_pos) = entry.signature.find(&entry.name) {
                    let suffix = &entry.signature[name_pos..];
                    entry.signature = format!("{c_type} {suffix}");
                }
                entry.evidence.push(Evidence {
                    kind: EvidenceKind::ProtocolProbe,
                    confidence: 0.85,
                    detail: format!(
                        "protocol {} step {} returned {}",
                        proto.group, step.function, step.returns
                    ),
                });
                updated += 1;
            }
        }
        updated
    }

    /// Add evidence to all entries in bulk (e.g., after enrichment pass).
    pub fn insert_evidence_bulk(&mut self, kind: EvidenceKind, confidence: f64, detail: &str) {
        for entry in self.functions.values_mut() {
            if !entry.evidence.iter().any(|e| e.kind == kind) {
                entry.evidence.push(Evidence {
                    kind: kind.clone(),
                    confidence,
                    detail: detail.into(),
                });
            }
        }
    }

    /// Rebuild capability index from functions (e.g. after deserializing).
    pub fn rebuild_index(&mut self) {
        self.capabilities.clear();
        for entry in self.functions.values() {
            for cap in &entry.capabilities {
                self.capabilities
                    .entry(cap.clone())
                    .or_default()
                    .push(entry.name.clone());
            }
        }
    }

    // ===========================================================================
    // Persistence
    // ===========================================================================

    /// Cache format version. Bump this when KB schema or seed data changes
    /// to auto-invalidate stale caches on next launch.
    pub const CACHE_VERSION: u32 = 3;

    /// Default cache directory from platform config.
    pub fn cache_dir() -> PathBuf {
        crate::config::platform().kb_cache_dir()
    }

    /// Check if cache is current version. If not, wipe and return false.
    pub fn validate_cache(dir: &Path) -> bool {
        let version_file = dir.join(".version");
        if let Ok(data) = std::fs::read_to_string(&version_file) {
            if data.trim().parse::<u32>() == Ok(Self::CACHE_VERSION) {
                return true;
            }
        }
        // Stale or missing version — wipe all JSONs and write new version
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "json") {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(version_file, Self::CACHE_VERSION.to_string());
        false
    }

    /// Save to JSON at ~/.mcgyver/kb/{framework}.json
    pub fn save(&self, dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.json", self.framework));
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load from JSON at dir/{framework}.json
    pub fn load(dir: &Path, framework: &str) -> anyhow::Result<Self> {
        let path = dir.join(format!("{framework}.json"));
        let data = std::fs::read_to_string(path)?;
        let mut kb: Self = serde_json::from_str(&data)?;
        kb.rebuild_index();
        Ok(kb)
    }

    // ===========================================================================
    // Query
    // ===========================================================================

    /// Exact name lookup.
    pub fn query_name(&self, name: &str) -> Option<&ApiEntry> {
        self.functions.get(name)
    }

    /// All functions with a given capability tag.
    pub fn query_capability(&self, cap: &str) -> Vec<&ApiEntry> {
        self.capabilities
            .get(cap)
            .map(|names| names.iter().filter_map(|n| self.functions.get(n)).collect())
            .unwrap_or_default()
    }

    /// Natural language query: tokenize input, score each entry.
    /// Weights: exact name = 3.0, capability match = 2.0, partial name = 0.5.
    pub fn query_natural(&self, text: &str) -> Vec<(&ApiEntry, f64)> {
        let tokens: Vec<String> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() >= 2)
            .map(|t| t.to_lowercase())
            .collect();
        if tokens.is_empty() {
            return vec![];
        }

        let mut scored: Vec<(&ApiEntry, f64)> = self
            .functions
            .values()
            .filter_map(|entry| {
                let name_lower = entry.name.to_lowercase();
                let mut score = 0.0_f64;
                for tok in &tokens {
                    if name_lower == *tok {
                        score += 3.0;
                    } else if name_lower.contains(tok.as_str()) {
                        score += 0.5;
                    }
                    for cap in &entry.capabilities {
                        if cap.contains(tok.as_str()) {
                            score += 2.0;
                        }
                    }
                }
                if score > 0.0 {
                    Some((entry, score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    // ===========================================================================
    // LLM context generation
    // ===========================================================================

    /// Generate markdown context for the LLM about a specific API.
    pub fn to_llm_context(&self, name: &str) -> Option<String> {
        let entry = self.functions.get(name)?;
        let mut md = String::with_capacity(512);
        md.push_str(&format!("## `{}`\n\n", entry.name));
        md.push_str(&format!("**Framework**: {}\n\n", entry.framework));
        md.push_str(&format!("**Signature**: `{}`\n\n", entry.signature));

        if !entry.parameters.is_empty() {
            md.push_str("**Parameters**:\n");
            for p in &entry.parameters {
                let ptr = if p.is_pointer { " *" } else { "" };
                md.push_str(&format!("- `{}`: `{}{}`\n", p.name, p.type_name, ptr));
            }
            md.push('\n');
        }

        md.push_str(&format!("**Returns**: `{}`\n\n", entry.return_type));

        if !entry.capabilities.is_empty() {
            md.push_str(&format!(
                "**Capabilities**: {}\n\n",
                entry.capabilities.join(", ")
            ));
        }

        if !entry.verb.is_empty() && entry.verb != "unknown" {
            md.push_str(&format!("**Action**: {}\n\n", entry.verb));
        }
        if !entry.input_formats.is_empty() {
            md.push_str(&format!(
                "**Input formats**: {}\n\n",
                entry.input_formats.join(", ")
            ));
        }
        if !entry.output_formats.is_empty() {
            md.push_str(&format!(
                "**Output formats**: {}\n\n",
                entry.output_formats.join(", ")
            ));
        }
        if !entry.class_hierarchy.is_empty() {
            md.push_str(&format!(
                "**Inherits**: {}\n\n",
                entry.class_hierarchy.join(" → ")
            ));
        }
        if !entry.protocols.is_empty() {
            md.push_str(&format!(
                "**Protocols**: {}\n\n",
                entry.protocols.join(", ")
            ));
        }
        if entry.async_pattern {
            md.push_str("**Async**: yes (completion handler)\n\n");
        }
        if !entry.related.is_empty() {
            md.push_str(&format!("**Related**: {}\n\n", entry.related.join(", ")));
        }
        if !entry.semantic_hints.is_empty() {
            md.push_str("**Hints from binary**:\n");
            for hint in entry.semantic_hints.iter().take(5) {
                md.push_str(&format!("- \"{hint}\"\n"));
            }
            md.push('\n');
        }

        let conf = entry.confidence();
        md.push_str(&format!("**Confidence**: {:.0}%\n\n", conf * 100.0));

        if !entry.evidence.is_empty() {
            md.push_str("**Evidence**:\n");
            for e in &entry.evidence {
                md.push_str(&format!(
                    "- {:?} ({:.0}%): {}\n",
                    e.kind,
                    e.confidence * 100.0,
                    e.detail
                ));
            }
        }

        Some(md)
    }

    /// Generate bulk LLM context for all functions matching a capability.
    pub fn to_llm_context_for_capability(&self, cap: &str) -> String {
        let entries = self.query_capability(cap);
        if entries.is_empty() {
            return format!("No APIs found for capability `{cap}`.\n");
        }
        let mut md = format!("# {} APIs: {}\n\n", self.framework, cap);
        for entry in entries {
            md.push_str(&format!(
                "- `{}` — {}\n",
                entry.signature, entry.return_type
            ));
        }
        md
    }

    /// Return protocol recipes (working ctypes code) for functions matching a capability.
    /// These are production-quality code snippets from the KB seed data.
    /// Matches protocols by group name OR by any step function that appears in the
    /// capability's function list. This ensures e.g. MetalDispatchHarness surfaces via
    /// "gpu_compute" because its step `MTLCreateSystemDefaultDevice` is in that capability.
    pub fn recipes_for_capability(&self, cap: &str) -> Vec<(&str, &str)> {
        let func_names = match self.capabilities.get(cap) {
            Some(names) => names,
            None => return vec![],
        };
        self.protocols
            .iter()
            .filter(|p| {
                // Match if the protocol group name is in the capability function list
                func_names.iter().any(|n| p.group == *n)
                // OR if any step function in the protocol is in the capability function list
                || p.steps.iter().any(|s| func_names.iter().any(|n| s.function == *n))
            })
            .filter_map(|p| p.recipe.as_deref().map(|r| (p.group.as_str(), r)))
            .collect()
    }
}

/// Map Python ctypes return type strings to C types.
fn ctypes_to_c(ctypes_str: &str) -> String {
    match ctypes_str.trim() {
        "c_void_p" | "ctypes.c_void_p" => "void *".into(),
        "c_int" | "ctypes.c_int" | "int" => "int".into(),
        "c_uint" | "ctypes.c_uint" => "unsigned int".into(),
        "c_long" | "ctypes.c_long" => "long".into(),
        "c_char_p" | "ctypes.c_char_p" => "const char *".into(),
        "c_bool" | "ctypes.c_bool" | "bool" => "bool".into(),
        "c_double" | "ctypes.c_double" => "double".into(),
        "c_float" | "ctypes.c_float" => "float".into(),
        "c_size_t" | "ctypes.c_size_t" => "size_t".into(),
        "c_uint32" | "ctypes.c_uint32" => "uint32_t".into(),
        "c_uint64" | "ctypes.c_uint64" => "uint64_t".into(),
        "void" => "void".into(),
        other => other.to_string(), // pass through unknown types
    }
}

// =============================================================================
// Seeds: JSON-based curated API entries for known frameworks
// =============================================================================

/// JSON-serializable seed file format. One per framework (or sub-framework).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedFile {
    pub framework: String,
    #[serde(default)]
    pub type_aliases: Vec<(String, String)>,
    pub entries: Vec<SeedEntry>,
    #[serde(default)]
    pub protocols: Vec<crate::protocol::ProtocolEntry>,
    #[serde(default)]
    pub cross_link: bool,
}

/// A single API entry in seed JSON. Flattened for readability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedEntry {
    pub name: String,
    pub verb: String,
    pub capabilities: Vec<String>,
    /// Each param is [name, type, is_pointer].
    pub params: Vec<(String, String, bool)>,
    pub return_type: String,
    pub signature: String,
    #[serde(default)]
    pub input_formats: Vec<String>,
    #[serde(default)]
    pub output_formats: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub async_pattern: bool,
}

/// Load a JSON seed file into a FrameworkKB.
pub fn load_seed_json(json: &str, kb: &mut FrameworkKB) {
    let seed: SeedFile = serde_json::from_str(json).expect("invalid seed JSON");
    for (alias, target) in &seed.type_aliases {
        kb.type_aliases.insert(alias.clone(), target.clone());
    }
    for e in &seed.entries {
        let evidence_detail = if e.notes.is_empty() {
            "curated seed".to_string()
        } else {
            e.notes.clone()
        };
        kb.insert(ApiEntry {
            name: e.name.clone(),
            framework: seed.framework.clone(),
            return_type: e.return_type.clone(),
            parameters: e
                .params
                .iter()
                .map(|(n, t, p)| ParamEntry {
                    name: n.clone(),
                    type_name: t.clone(),
                    is_pointer: *p,
                })
                .collect(),
            capabilities: e.capabilities.clone(),
            evidence: vec![Evidence {
                kind: EvidenceKind::Manual,
                confidence: 1.0,
                detail: evidence_detail,
            }],
            signature: e.signature.clone(),
            verb: e.verb.clone(),
            input_formats: e.input_formats.clone(),
            output_formats: e.output_formats.clone(),
            class_hierarchy: vec![],
            protocols: vec![],
            related: vec![],
            async_pattern: e.async_pattern,
            semantic_hints: if e.notes.is_empty() {
                vec![]
            } else {
                vec![e.notes.clone()]
            },
        });
    }
    for proto in seed.protocols.clone() {
        kb.protocols.push(proto);
    }
    if seed.cross_link {
        kb.cross_link(5);
    }
}

/// Convert a FrameworkKB back to SeedFile format (for JSON export).
pub fn kb_to_seed_file(kb: &FrameworkKB) -> SeedFile {
    let entries: Vec<SeedEntry> = kb
        .functions
        .values()
        .map(|e| SeedEntry {
            name: e.name.clone(),
            verb: e.verb.clone(),
            capabilities: e.capabilities.clone(),
            params: e
                .parameters
                .iter()
                .map(|p| (p.name.clone(), p.type_name.clone(), p.is_pointer))
                .collect(),
            return_type: e.return_type.clone(),
            signature: e.signature.clone(),
            input_formats: e.input_formats.clone(),
            output_formats: e.output_formats.clone(),
            notes: e.semantic_hints.first().cloned().unwrap_or_default(),
            async_pattern: e.async_pattern,
        })
        .collect();
    SeedFile {
        framework: kb.framework.clone(),
        type_aliases: kb
            .type_aliases
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        entries,
        protocols: kb.protocols.clone(),
        cross_link: true,
    }
}

impl FrameworkKB {
    /// Cross-link all entries: each entry gets `related` populated with up to `n` other function names.
    pub fn cross_link(&mut self, n: usize) {
        let names: Vec<String> = self.functions.keys().cloned().collect();
        for entry in self.functions.values_mut() {
            entry.related = names
                .iter()
                .filter(|nm| *nm != &entry.name)
                .take(n)
                .cloned()
                .collect();
        }
    }
}

// JSON seed files, loaded at compile time via include_str!
// Each tuple: (framework_name_prefix, json_content)
const SEED_FILES: &[(&str, &str)] = &[
    (
        "Accelerate_vimage",
        include_str!("../seeds/accelerate_vimage.json"),
    ),
    (
        "Accelerate_blas",
        include_str!("../seeds/accelerate_blas.json"),
    ),
    (
        "Accelerate_vdsp",
        include_str!("../seeds/accelerate_vdsp.json"),
    ),
    (
        "VideoToolbox",
        include_str!("../seeds/videotoolbox.json"),
    ),
    ("POSIX", include_str!("../seeds/posix_io.json")),
    (
        "CommonCrypto",
        include_str!("../seeds/commoncrypto.json"),
    ),
    ("Security", include_str!("../seeds/security.json")),
    ("Network", include_str!("../seeds/network.json")),
    ("IOSurface", include_str!("../seeds/iosurface.json")),
    ("kperf", include_str!("../seeds/kperf.json")),
    (
        "Metal_compute",
        include_str!("../seeds/metal_compute.json"),
    ),
    (
        "MetalPerformanceShaders",
        include_str!("../seeds/mps.json"),
    ),
    ("NVIDIA_RM", include_str!("../seeds/nvidia_rm.json")),
    ("CUDA_cublas", include_str!("../seeds/cublas.json")),
    ("CUDA_npp", include_str!("../seeds/npp.json")),
    ("CUDA_cudnn", include_str!("../seeds/cudnn.json")),
    ("CUDA_cufft", include_str!("../seeds/cufft.json")),
    ("ROCm", include_str!("../seeds/rocm_hip.json")),
    ("Hailo_VDMA", include_str!("../seeds/hailo_vdma.json")),
    ("CoreML", include_str!("../seeds/coreml.json")),
];

// Legacy seed function wrappers — delegate to JSON loader.
// These exist for backward compatibility with tests.
pub fn seed_accelerate_vimage(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/accelerate_vimage.json"), kb);
}
pub fn seed_accelerate_blas(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/accelerate_blas.json"), kb);
}
pub fn seed_videotoolbox(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/videotoolbox.json"), kb);
}
pub fn seed_posix_io(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/posix_io.json"), kb);
}
pub fn seed_commoncrypto(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/commoncrypto.json"), kb);
}
pub fn seed_security_framework(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/security.json"), kb);
}
pub fn seed_network_framework(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/network.json"), kb);
}
pub fn seed_iosurface(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/iosurface.json"), kb);
}
pub fn seed_kperf(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/kperf.json"), kb);
}
pub fn seed_mps(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/mps.json"), kb);
}
pub fn seed_nvidia() -> FrameworkKB {
    let mut kb = FrameworkKB::new("NVIDIA_RM");
    load_seed_json(include_str!("../seeds/nvidia_rm.json"), &mut kb);
    kb
}
pub fn seed_cublas() -> FrameworkKB {
    let mut kb = FrameworkKB::new("CUDA");
    load_seed_json(include_str!("../seeds/cublas.json"), &mut kb);
    kb
}
pub fn seed_npp() -> FrameworkKB {
    let mut kb = FrameworkKB::new("CUDA");
    load_seed_json(include_str!("../seeds/npp.json"), &mut kb);
    kb
}
pub fn seed_hailo() -> FrameworkKB {
    let mut kb = FrameworkKB::new("Hailo_VDMA");
    load_seed_json(include_str!("../seeds/hailo_vdma.json"), &mut kb);
    kb
}
pub fn seed_coreml(kb: &mut FrameworkKB) {
    load_seed_json(include_str!("../seeds/coreml.json"), kb);
}
pub fn seed_rocm() -> FrameworkKB {
    let mut kb = FrameworkKB::new("ROCm");
    load_seed_json(include_str!("../seeds/rocm_hip.json"), &mut kb);
    kb
}

// Device-aware seed loading
// =============================================================================

/// Load seed data into a KB by framework name. Returns true if seeds were found.
fn seed_into(name: &str, kb: &mut FrameworkKB) -> bool {
    let mut found = false;
    for (key, json) in SEED_FILES {
        // Match "Accelerate" against "Accelerate_vimage" and "Accelerate_blas"
        if *key == name || key.starts_with(&format!("{name}_")) {
            load_seed_json(json, kb);
            found = true;
        }
    }
    found
}

/// Load seed KB for a single named framework (no device check).
/// Returns None for frameworks without seed data.
pub fn load_seed_kb(name: &str) -> Option<FrameworkKB> {
    let mut kb = FrameworkKB::new(name);
    if seed_into(name, &mut kb) {
        Some(kb)
    } else {
        None
    }
}

/// Load all relevant seed KBs for the current device.
///
/// Uses `DeviceProfile` to determine which frameworks/hardware actually
/// exist on this system. On macOS: loads Apple framework seeds for frameworks
/// found in `device.frameworks`. On Linux: skips Apple seeds entirely.
/// POSIX I/O seeds load on any POSIX platform. NVIDIA/Hailo seeds load
/// when the corresponding hardware is detected.
pub fn load_seeds_for_device(device: &crate::device::DeviceProfile) -> Vec<FrameworkKB> {
    let mut kbs = Vec::new();

    // Load seeds for frameworks that DeviceProfile actually discovered
    for fw in &device.frameworks {
        let mut kb = FrameworkKB::new(&fw.name);
        if seed_into(&fw.name, &mut kb) {
            kbs.push(kb);
        }
    }

    // POSIX I/O: available on macOS and Linux
    if device.platform == "macos" || device.platform == "linux" {
        if !kbs.iter().any(|kb| kb.framework == "POSIX") {
            let mut posix = FrameworkKB::new("POSIX");
            seed_posix_io(&mut posix);
            kbs.push(posix);
        }
    }

    // CommonCrypto: macOS-only, not a .framework dir so DeviceProfile won't list it
    if device.platform == "macos" && !kbs.iter().any(|kb| kb.framework == "CommonCrypto") {
        let mut cc = FrameworkKB::new("CommonCrypto");
        seed_commoncrypto(&mut cc);
        kbs.push(cc);
    }

    // CoreML: macOS-only, loaded via coremltools Python API (not a .framework dir)
    if device.platform == "macos" && !kbs.iter().any(|kb| kb.framework == "CoreML") {
        let mut coreml_kb = FrameworkKB::new("CoreML");
        seed_coreml(&mut coreml_kb);
        kbs.push(coreml_kb);
    }

    // NVIDIA: detected via GPU vendor
    if device
        .gpus
        .iter()
        .any(|g| g.vendor.to_lowercase().contains("nvidia"))
    {
        if !kbs.iter().any(|kb| kb.framework == "NVIDIA_RM") {
            kbs.push(seed_nvidia());
        }
        // CUDA high-level libraries (cuBLAS, NPP)
        if !kbs.iter().any(|kb| kb.framework == "CUDA") {
            let mut cuda_kb = seed_cublas();
            // Merge all CUDA libraries into one KB
            load_seed_json(include_str!("../seeds/npp.json"), &mut cuda_kb);
            load_seed_json(include_str!("../seeds/cudnn.json"), &mut cuda_kb);
            load_seed_json(include_str!("../seeds/cufft.json"), &mut cuda_kb);
            kbs.push(cuda_kb);
        }
    }

    // ROCm: detected via AMD GPU vendor
    if device.gpus.iter().any(|g| {
        g.vendor.to_lowercase().contains("amd")
            || g.vendor.to_lowercase().contains("advanced micro")
    }) {
        if !kbs.iter().any(|kb| kb.framework == "ROCm") {
            let mut rocm_kb = FrameworkKB::new("ROCm");
            load_seed_json(include_str!("../seeds/rocm_hip.json"), &mut rocm_kb);
            kbs.push(rocm_kb);
        }
    }

    // Hailo: detected via accelerator kind
    if device
        .accelerators
        .iter()
        .any(|a| a.kind == "hailo" && a.available)
    {
        if !kbs.iter().any(|kb| kb.framework == "Hailo_VDMA") {
            kbs.push(seed_hailo());
        }
    }

    kbs
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::*;

    fn sample_entry(name: &str, framework: &str) -> ApiEntry {
        ApiEntry {
            name: name.into(),
            framework: framework.into(),
            return_type: "int".into(),
            parameters: vec![ParamEntry {
                name: "ctx".into(),
                type_name: "void".into(),
                is_pointer: true,
            }],
            capabilities: vec!["gpu_compute".into()],
            evidence: vec![
                Evidence {
                    kind: EvidenceKind::DyldExport,
                    confidence: 0.8,
                    detail: "offset 0x1000".into(),
                },
                Evidence {
                    kind: EvidenceKind::NamingInference,
                    confidence: 0.7,
                    detail: "Create pattern".into(),
                },
            ],
            signature: "int FooCreate(void *ctx)".into(),
            verb: String::new(),
            input_formats: vec![],
            output_formats: vec![],
            class_hierarchy: vec![],
            protocols: vec![],
            related: vec![],
            async_pattern: false,
            semantic_hints: vec![],
        }
    }

    #[test]
    fn test_compound_confidence() {
        let entry = sample_entry("FooCreate", "Metal");
        // 1 - (1-0.8)*(1-0.7) = 1 - 0.2*0.3 = 1 - 0.06 = 0.94
        let conf = entry.confidence();
        assert!((conf - 0.94).abs() < 0.001, "expected 0.94, got {conf}");
    }

    #[test]
    fn test_empty_evidence_zero_confidence() {
        let mut entry = sample_entry("Foo", "Bar");
        entry.evidence.clear();
        assert_eq!(entry.confidence(), 0.0);
    }

    #[test]
    fn test_kb_insert_and_query() {
        let mut kb = FrameworkKB::new("Metal");
        kb.insert(sample_entry("MTLCreateDevice", "Metal"));
        kb.insert(sample_entry("MTLDestroyDevice", "Metal"));

        assert!(kb.query_name("MTLCreateDevice").is_some());
        assert!(kb.query_name("nonexistent").is_none());

        let gpu = kb.query_capability("gpu_compute");
        assert_eq!(gpu.len(), 2);
    }

    #[test]
    fn test_natural_query() {
        let mut kb = FrameworkKB::new("Metal");
        kb.insert(sample_entry("MTLCreateDevice", "Metal"));
        kb.insert(sample_entry("vImageScale", "Accelerate"));

        let results = kb.query_natural("create device gpu");
        assert!(!results.is_empty());
        // MTLCreateDevice should score higher (matches "create" in name + "gpu" in capability)
        assert!(results[0].0.name.contains("MTL"));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut kb = FrameworkKB::new("TestFW");
        kb.insert(sample_entry("TestFunc", "TestFW"));
        kb.save(dir.path()).unwrap();

        let loaded = FrameworkKB::load(dir.path(), "TestFW").unwrap();
        assert_eq!(loaded.functions.len(), 1);
        assert!(loaded.query_name("TestFunc").is_some());
        assert_eq!(loaded.query_capability("gpu_compute").len(), 1);
    }

    #[test]
    fn test_llm_context_generation() {
        let mut kb = FrameworkKB::new("Metal");
        kb.insert(sample_entry("MTLCreateDevice", "Metal"));

        let ctx = kb.to_llm_context("MTLCreateDevice").unwrap();
        assert!(ctx.contains("MTLCreateDevice"));
        assert!(ctx.contains("Metal"));
        assert!(ctx.contains("Confidence"));
        assert!(ctx.contains("Evidence"));
    }

    #[test]
    fn test_apply_protocol_feedback() {
        use crate::crate::protocol::{ProtocolEntry, ProtocolStep, StepRole};
        let mut kb = FrameworkKB::new("TestFW");
        let entry = ApiEntry::new_basic(
            "SvcCreate",
            "TestFW",
            "void",
            "void/*?*/ SvcCreate(void)",
            vec![Evidence {
                kind: EvidenceKind::NamingInference,
                confidence: 0.7,
                detail: "guess".into(),
            }],
        );
        kb.insert(entry);
        let proto = ProtocolEntry {
            framework: "TestFW".into(),
            group: "Svc".into(),
            prerequisites: vec![],
            constants: vec![],
            steps: vec![ProtocolStep {
                order: 0,
                function: "SvcCreate".into(),
                role: StepRole::Create,
                args_hint: String::new(),
                returns: "c_void_p".into(),
                notes: String::new(),
            }],
            validated: true,
            iterations: 1,
            recipe: None,
        };
        let updated = kb.apply_protocol_feedback(&proto);
        assert_eq!(updated, 1);
        let entry = kb.query_name("SvcCreate").unwrap();
        assert_eq!(entry.return_type, "void *");
        assert!(entry
            .evidence
            .iter()
            .any(|e| e.kind == EvidenceKind::ProtocolProbe));
    }

    #[test]
    fn test_ctypes_to_c() {
        assert_eq!(ctypes_to_c("c_void_p"), "void *");
        assert_eq!(ctypes_to_c("c_int"), "int");
        assert_eq!(ctypes_to_c("void"), "void");
        assert_eq!(ctypes_to_c("SomeCustomType"), "SomeCustomType");
    }

    #[test]
    fn test_seed_accelerate_vimage() {
        let mut kb = FrameworkKB::new("Accelerate");
        seed_accelerate_vimage(&mut kb);
        assert_eq!(kb.functions.len(), 7);
        // Query by capability
        let scale = kb.query_capability("image_scale");
        assert_eq!(scale.len(), 2, "should find vImageScale_ARGB8888 + Planar8");
        assert!(scale.iter().any(|e| e.name == "vImageScale_ARGB8888"));
        let convert = kb.query_capability("image_color_convert");
        assert_eq!(
            convert.len(),
            2,
            "image_color_convert should find vImageConvert + vImageMatrixMultiply"
        );
        assert!(convert
            .iter()
            .any(|e| e.name == "vImageConvert_ARGB8888toRGB888"));
        assert!(convert
            .iter()
            .any(|e| e.name == "vImageMatrixMultiply_ARGB8888ToPlanar8"));
        // Legacy tag still works too
        let convert_legacy = kb.query_capability("image_convert");
        assert_eq!(
            convert_legacy.len(),
            1,
            "legacy image_convert tag should still match"
        );
        // Check entry details
        let entry = kb.query_name("vImageScale_ARGB8888").unwrap();
        assert_eq!(entry.framework, "Accelerate");
        assert_eq!(entry.verb, "scale");
        assert_eq!(entry.confidence(), 1.0);
        assert!(!entry.related.is_empty(), "should have related functions");
        // Natural query
        let results = kb.query_natural("scale image");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_seed_posix_io() {
        let mut kb = FrameworkKB::new("POSIX");
        seed_posix_io(&mut kb);
        assert_eq!(kb.functions.len(), 9, "writev + readv + sendfile + copy_file_range + splice + kevent + epoll_wait + io_uring_enter + mmap");
        // Vectored I/O
        let writev_caps = kb.query_capability("io_writev");
        assert!(
            writev_caps.iter().any(|e| e.name == "writev"),
            "should find writev"
        );
        let readv_caps = kb.query_capability("io_readv");
        assert!(
            readv_caps.iter().any(|e| e.name == "readv"),
            "should find readv"
        );
        // Zero-copy
        let zc = kb.query_capability("io_zerocopy");
        assert_eq!(zc.len(), 3, "sendfile + copy_file_range + splice");
        // Event notification
        let poll = kb.query_capability("io_poll");
        assert_eq!(poll.len(), 3, "kevent + epoll_wait + io_uring_enter");
        // mmap
        let mmap = kb.query_capability("io_mmap");
        assert_eq!(mmap.len(), 1);
        // Natural language query
        let results = kb.query_natural("zero copy file transfer");
        assert!(!results.is_empty(), "should find zero-copy entries");
        // All entries should have confidence 1.0 (manual seed)
        for entry in kb.functions.values() {
            assert_eq!(
                entry.confidence(),
                1.0,
                "{} should have confidence 1.0",
                entry.name
            );
        }
    }

    #[test]
    fn test_seed_commoncrypto() {
        let mut kb = FrameworkKB::new("CommonCrypto");
        seed_commoncrypto(&mut kb);
        assert_eq!(kb.functions.len(), 10, "CCCryptorCreate + CCCryptorUpdate + CCCryptorFinal + CCCrypt + CC_SHA256 + CC_SHA512 + CCHmac + CCHmacInit + CCKeyDerivationPBKDF + CCRandomGenerateBytes");
        // Cipher
        let cipher = kb.query_capability("crypto_cipher");
        assert!(
            cipher.len() >= 4,
            "should find CCCryptorCreate + Update + Final + CCCrypt"
        );
        assert!(cipher.iter().any(|e| e.name == "CCCryptorCreate"));
        // Hash
        let hash = kb.query_capability("crypto_hash");
        assert_eq!(hash.len(), 2, "CC_SHA256 + CC_SHA512");
        // HMAC
        let hmac = kb.query_capability("crypto_hmac");
        assert!(hmac.iter().any(|e| e.name == "CCHmac"));
        // KDF
        let kdf = kb.query_capability("crypto_kdf");
        assert_eq!(kdf.len(), 1);
        assert_eq!(kdf[0].name, "CCKeyDerivationPBKDF");
        // Random
        let random = kb.query_capability("crypto_random");
        assert_eq!(random.len(), 1);
        assert_eq!(random[0].name, "CCRandomGenerateBytes");
        // Confidence
        for entry in kb.functions.values() {
            assert_eq!(
                entry.confidence(),
                1.0,
                "{} should have confidence 1.0",
                entry.name
            );
        }
        // Natural query
        let results = kb.query_natural("encrypt AES cipher");
        assert!(!results.is_empty(), "should find cipher entries");
    }

    #[test]
    fn test_seed_security_framework() {
        let mut kb = FrameworkKB::new("Security");
        seed_security_framework(&mut kb);
        assert_eq!(kb.functions.len(), 4, "SecKeyCreateSignature + SecKeyVerifySignature + SecKeyCreateRandomKey + SecCertificateCreateWithData");
        let sign = kb.query_capability("crypto_sign");
        assert_eq!(sign.len(), 4);
        assert!(sign.iter().any(|e| e.name == "SecKeyCreateSignature"));
        assert!(sign.iter().any(|e| e.name == "SecKeyVerifySignature"));
    }

    #[test]
    fn test_seed_network_framework() {
        let mut kb = FrameworkKB::new("Network");
        seed_network_framework(&mut kb);
        assert_eq!(
            kb.functions.len(),
            5,
            "create + send + receive + listener_create + set_handler"
        );
        // Socket creation
        let socket = kb.query_capability("io_socket");
        assert!(
            socket.len() >= 2,
            "nw_connection_create + nw_listener_create"
        );
        // Write
        let write = kb.query_capability("io_write");
        assert!(write.iter().any(|e| e.name == "nw_connection_send"));
        // Zero-copy
        let zc = kb.query_capability("io_zerocopy");
        assert!(zc.iter().any(|e| e.name == "nw_connection_send"));
        // Read
        let read = kb.query_capability("io_read");
        assert!(read.iter().any(|e| e.name == "nw_connection_receive"));
        // Poll
        let poll = kb.query_capability("io_poll");
        assert!(poll
            .iter()
            .any(|e| e.name == "nw_listener_set_new_connection_handler"));
        // All async
        for entry in kb.functions.values() {
            assert!(entry.async_pattern, "{} should be async", entry.name);
        }
    }

    #[test]
    fn test_seed_videotoolbox() {
        let mut kb = FrameworkKB::new("VideoToolbox");
        seed_videotoolbox(&mut kb);
        assert_eq!(kb.functions.len(), 16,
      "2 session creates + 4 decode + 3 encode + 2 transfer + 1 query + 2 CoreMedia + 2 CoreVideo");
        // Decode capability
        let decode = kb.query_capability("video_decode");
        assert!(
            decode.len() >= 7,
            "should find decode pipeline entries, got {}",
            decode.len()
        );
        assert!(decode
            .iter()
            .any(|e| e.name == "VTDecompressionSessionDecodeFrame"));
        assert!(decode
            .iter()
            .any(|e| e.name == "VTIsHardwareDecodeSupported"));
        assert!(decode.iter().any(|e| e.name == "CVPixelBufferGetIOSurface"));
        // Encode capability
        let encode = kb.query_capability("video_encode");
        assert!(encode.len() >= 3, "should find encode pipeline entries");
        assert!(encode
            .iter()
            .any(|e| e.name == "VTCompressionSessionEncodeFrame"));
        // Transcode capability
        let transcode = kb.query_capability("video_transcode");
        assert!(
            transcode.len() >= 4,
            "should find session creates + pixel transfer"
        );
        // Surface bridge
        let surface = kb.query_capability("surface_zerocopy");
        assert!(surface
            .iter()
            .any(|e| e.name == "CVPixelBufferGetIOSurface"));
        // All async
        for entry in kb.functions.values() {
            assert!(entry.async_pattern, "{} should be async", entry.name);
            assert_eq!(entry.confidence(), 1.0);
        }
    }

    #[test]
    fn test_seed_iosurface() {
        let mut kb = FrameworkKB::new("IOSurface");
        seed_iosurface(&mut kb);
        assert_eq!(kb.functions.len(), 11,
      "Create + Lock + Unlock + GetBaseAddress + GetWidth + GetHeight + GetBytesPerRow + GetPixelFormat + GetAllocSize + CreateMachPort + LookupFromMachPort");
        // All entries have surface_zerocopy capability
        let zc = kb.query_capability("surface_zerocopy");
        assert_eq!(
            zc.len(),
            11,
            "all IOSurface entries should have surface_zerocopy"
        );
        // Lock/Unlock pair exists
        assert!(kb.query_name("IOSurfaceLock").is_some());
        assert!(kb.query_name("IOSurfaceUnlock").is_some());
        // Base address for zero-copy CPU access
        let base = kb.query_name("IOSurfaceGetBaseAddress").unwrap();
        assert_eq!(base.return_type, "void *");
        assert_eq!(base.verb, "query");
        // Mach port IPC pair
        assert!(kb.query_name("IOSurfaceCreateMachPort").is_some());
        assert!(kb.query_name("IOSurfaceLookupFromMachPort").is_some());
        // Not async (synchronous API)
        for entry in kb.functions.values() {
            assert!(!entry.async_pattern, "{} should not be async", entry.name);
            assert_eq!(entry.confidence(), 1.0);
        }
        // Natural query
        let results = kb.query_natural("zero copy surface GPU");
        assert!(!results.is_empty(), "should find IOSurface entries");
    }

    #[test]
    fn test_seed_kperf() {
        let mut kb = FrameworkKB::new("kperf");
        seed_kperf(&mut kb);
        assert_eq!(kb.functions.len(), 10,
      "get_counter_count + get_config_count + get_config + set_config + get_counting + set_counting + get_thread_counters + get_cpu_counters + force_all_ctrs_set + force_all_ctrs_get");
        // All entries have hw_counters capability
        let hw = kb.query_capability("hw_counters");
        assert_eq!(hw.len(), 10, "all kperf entries should have hw_counters");
        // Key functions exist
        assert!(kb.query_name("kpc_get_thread_counters").is_some());
        assert!(kb.query_name("kpc_set_config").is_some());
        assert!(kb.query_name("kpc_force_all_ctrs_set").is_some());
        // Verbs
        let thread_ctrs = kb.query_name("kpc_get_thread_counters").unwrap();
        assert_eq!(thread_ctrs.verb, "read");
        let set_cfg = kb.query_name("kpc_set_config").unwrap();
        assert_eq!(set_cfg.verb, "configure");
        // Not async
        for entry in kb.functions.values() {
            assert!(!entry.async_pattern, "{} should not be async", entry.name);
            assert_eq!(entry.confidence(), 1.0);
        }
    }

    #[test]
    fn test_seed_mps() {
        let mut kb = FrameworkKB::new("MetalPerformanceShaders");
        seed_mps(&mut kb);
        assert_eq!(
            kb.functions.len(),
            15,
            "4 linalg + 4 image + 1 histogram + 4 CNN + 1 DFT + 1 raytrace"
        );
        // GPU matmul
        let matmul = kb.query_capability("gpu_matmul");
        assert_eq!(
            matmul.len(),
            2,
            "MPSMatrixMultiplication + MPSMatrixVectorMultiplication"
        );
        assert!(matmul.iter().any(|e| e.name == "MPSMatrixMultiplication"));
        // GPU solve
        let solve = kb.query_capability("gpu_solve");
        assert_eq!(
            solve.len(),
            2,
            "MPSMatrixSolveTriangular + MPSMatrixDecompositionLU"
        );
        // GPU image scale
        let scale = kb.query_capability("gpu_image_scale");
        assert_eq!(scale.len(), 2, "Bilinear + Lanczos");
        // GPU convolution (image + CNN)
        let img_conv = kb.query_capability("gpu_image_convolve");
        assert_eq!(img_conv.len(), 2, "GaussianBlur + Convolution");
        let cnn_conv = kb.query_capability("gpu_conv");
        assert_eq!(cnn_conv.len(), 1, "MPSCNNConvolution");
        // GPU pooling
        let pool = kb.query_capability("gpu_pool");
        assert_eq!(pool.len(), 2, "Max + Average");
        // GPU FFT
        let fft = kb.query_capability("gpu_fft");
        assert_eq!(fft.len(), 1, "MPSImageDFT");
        // GPU raytrace
        let rt = kb.query_capability("gpu_raytrace");
        assert_eq!(rt.len(), 1, "MPSRayIntersector");
        // All confidence 1.0, not async (encode to command buffer is synchronous)
        for entry in kb.functions.values() {
            assert!(!entry.async_pattern, "{} should not be async", entry.name);
            assert_eq!(entry.confidence(), 1.0);
            assert_eq!(entry.framework, "MetalPerformanceShaders");
        }
    }

    #[test]
    fn test_mps_seed_has_protocols() {
        let mut kb = FrameworkKB::new("MetalPerformanceShaders");
        seed_mps(&mut kb);
        assert!(!kb.protocols.is_empty(), "MPS seed should have protocols");
        let groups: Vec<&str> = kb.protocols.iter().map(|p| p.group.as_str()).collect();
        assert!(
            groups.contains(&"MpsImageResize"),
            "MPS should have MpsImageResize protocol, got: {:?}",
            groups
        );
        assert!(
            groups.contains(&"MpsMatrixMultiply"),
            "MPS should have MpsMatrixMultiply protocol, got: {:?}",
            groups
        );
        // Verify recipes exist
        for proto in &kb.protocols {
            assert!(
                proto.recipe.is_some(),
                "MPS protocol {} should have a recipe",
                proto.group
            );
        }
        // Verify protocols are validated
        for proto in &kb.protocols {
            assert!(
                proto.validated,
                "MPS protocol {} should be validated",
                proto.group
            );
        }
    }

    #[test]
    fn test_videotoolbox_seed_has_protocols() {
        let mut kb = FrameworkKB::new("VideoToolbox");
        seed_videotoolbox(&mut kb);
        assert!(
            !kb.protocols.is_empty(),
            "VideoToolbox seed should have protocols"
        );
        // Verify at least one protocol exists with a recipe
        let with_recipe: Vec<&str> = kb
            .protocols
            .iter()
            .filter(|p| p.recipe.is_some())
            .map(|p| p.group.as_str())
            .collect();
        assert!(
            !with_recipe.is_empty(),
            "VideoToolbox should have at least one protocol with a recipe, got: {:?}",
            kb.protocols.iter().map(|p| &p.group).collect::<Vec<_>>()
        );
        // Verify protocols are validated
        for proto in &kb.protocols {
            assert!(
                proto.validated,
                "VideoToolbox protocol {} should be validated",
                proto.group
            );
        }
    }

    #[test]
    fn test_seed_accelerate_blas() {
        let mut kb = FrameworkKB::new("Accelerate");
        seed_accelerate_blas(&mut kb);
        assert_eq!(kb.functions.len(), 15,
      "2 GEMM + 4 BLAS L1 + 3 LAPACK + 1 vDSP_mmul + 1 vDSP_fft + 2 vDSP_elem + 2 vDSP_reduce = 15");
        // BLAS GEMM
        let gemm = kb.query_capability("blas_gemm");
        assert_eq!(gemm.len(), 2, "cblas_sgemm + cblas_dgemm");
        assert!(gemm.iter().any(|e| e.name == "cblas_sgemm"));
        assert!(gemm.iter().any(|e| e.name == "cblas_dgemm"));
        // BLAS Level 1
        let axpy = kb.query_capability("blas_axpy");
        assert_eq!(axpy.len(), 1, "cblas_saxpy");
        let dot = kb.query_capability("blas_dot");
        assert_eq!(dot.len(), 1, "cblas_sdot");
        let norm = kb.query_capability("blas_norm");
        assert_eq!(norm.len(), 1, "cblas_snrm2");
        let scale = kb.query_capability("blas_scale");
        assert_eq!(scale.len(), 1, "cblas_sscal");
        // LAPACK
        let solve = kb.query_capability("lapack_solve");
        assert_eq!(solve.len(), 1, "sgesv_");
        let decompose = kb.query_capability("lapack_decompose");
        assert_eq!(decompose.len(), 2, "spotrf_ + sgesvd_");
        // vDSP
        let vfft = kb.query_capability("vdsp_fft");
        assert_eq!(vfft.len(), 1, "vDSP_fft_zrip");
        let vmat = kb.query_capability("vdsp_matmul");
        assert_eq!(vmat.len(), 1, "vDSP_mmul");
        let velem = kb.query_capability("vdsp_elementwise");
        assert_eq!(velem.len(), 2, "vDSP_vadd + vDSP_vmul");
        let vreduce = kb.query_capability("vdsp_reduce");
        assert_eq!(vreduce.len(), 2, "vDSP_sve + vDSP_meanv");
        // All Accelerate framework, not async
        for entry in kb.functions.values() {
            assert!(!entry.async_pattern, "{} should not be async", entry.name);
            assert_eq!(entry.confidence(), 1.0);
            assert_eq!(entry.framework, "Accelerate");
        }
    }

    #[test]
    fn test_from_inferred() {
        use crate::crate::infer::{InferredParam, InferredSig};
        let sig = InferredSig {
            name: "IOSurfaceCreate".into(),
            return_type: "IOSurfaceRef".into(),
            parameters: vec![InferredParam {
                name: "props".into(),
                type_name: "CFDictionary".into(),
                is_pointer: true,
            }],
            confidence: 0.85,
            return_typed: false,
            pattern: "Create",
        };
        let evidence = vec![Evidence {
            kind: EvidenceKind::DyldExport,
            confidence: 0.8,
            detail: "found in exports".into(),
        }];
        let entry = ApiEntry::from_inferred(&sig, "IOSurface", evidence);
        assert_eq!(entry.name, "IOSurfaceCreate");
        assert_eq!(entry.framework, "IOSurface");
        assert!(entry.confidence() > 0.7);
    }

    // =========================================================================
    // NVIDIA RM tests
    // =========================================================================

    #[test]
    fn test_seed_nvidia_not_empty() {
        let kb = seed_nvidia();
        assert!(
            !kb.functions.is_empty(),
            "seed_nvidia() should return non-empty KB"
        );
        assert!(
            kb.functions.len() >= 25,
            "should have at least 25 entries, got {}",
            kb.functions.len()
        );
        assert_eq!(kb.framework, "NVIDIA_RM");
    }

    #[test]
    fn test_nvidia_gpu_init_capability() {
        let kb = seed_nvidia();
        let init = kb.query_capability("gpu_init");
        assert!(!init.is_empty(), "should have gpu_init entries");
        assert!(
            init.iter().any(|e| e.name == "NV_ESC_RM_ALLOC"),
            "gpu_init should include NV_ESC_RM_ALLOC"
        );
        assert!(
            init.iter().any(|e| e.name == "UVM_INITIALIZE"),
            "gpu_init should include UVM_INITIALIZE"
        );
    }

    #[test]
    fn test_nvidia_gpu_memory_capability() {
        let kb = seed_nvidia();
        let mem = kb.query_capability("gpu_memory");
        assert!(
            mem.len() >= 4,
            "should have gpu_memory entries, got {}",
            mem.len()
        );
        assert!(mem.iter().any(|e| e.name == "NV_ESC_RM_ALLOC_MEMORY"));
        assert!(mem.iter().any(|e| e.name == "NV_ESC_RM_MAP_MEMORY"));
    }

    #[test]
    fn test_nvidia_protocol_steps() {
        let kb = seed_nvidia();
        assert_eq!(kb.protocols.len(), 1, "should have one init protocol");
        let proto = &kb.protocols[0];
        assert_eq!(proto.group, "GPU_INIT");
        assert!(
            proto.steps.len() >= 15,
            "init protocol should have ~16 steps, got {}",
            proto.steps.len()
        );
        assert!(proto.validated);
    }

    #[test]
    fn test_nvidia_protocol_prerequisites() {
        let kb = seed_nvidia();
        let proto = &kb.protocols[0];
        let device_paths: Vec<_> = proto
            .prerequisites
            .iter()
            .filter(|p| p.kind == super::crate::protocol::PrereqKind::DevicePath)
            .collect();
        assert_eq!(
            device_paths.len(),
            3,
            "should have 3 device path prerequisites"
        );
        let paths: Vec<&str> = device_paths.iter().map(|p| p.path.as_str()).collect();
        assert!(paths.contains(&"/dev/nvidiactl"));
        assert!(paths.contains(&"/dev/nvidia-uvm"));
        assert!(paths.contains(&"/dev/nvidia0"));
    }

    #[test]
    fn test_cudnn_entries_loaded() {
        let mut kb = FrameworkKB::new("CUDA");
        load_seed_json(include_str!("../seeds/cudnn.json"), &mut kb);
        assert!(
            kb.functions.len() >= 6,
            "should have at least 6 cuDNN entries, got {}",
            kb.functions.len()
        );
        assert!(
            kb.query_name("cudnnConvolutionForward").is_some(),
            "should have conv forward"
        );
        assert!(
            kb.query_name("cudnnConvolutionBiasActivationForward")
                .is_some(),
            "should have fused conv"
        );
    }

    #[test]
    fn test_cufft_entries_loaded() {
        let mut kb = FrameworkKB::new("CUDA");
        load_seed_json(include_str!("../seeds/cufft.json"), &mut kb);
        assert!(
            kb.functions.len() >= 4,
            "should have at least 4 cuFFT entries, got {}",
            kb.functions.len()
        );
        assert!(kb.query_name("cufftExecR2C").is_some(), "should have R2C");
        assert!(kb.query_name("cufftExecC2C").is_some(), "should have C2C");
    }

    #[test]
    fn test_cuda_kb_merges_all_libs() {
        let mut kb = FrameworkKB::new("CUDA");
        load_seed_json(include_str!("../seeds/cublas.json"), &mut kb);
        load_seed_json(include_str!("../seeds/npp.json"), &mut kb);
        load_seed_json(include_str!("../seeds/cudnn.json"), &mut kb);
        load_seed_json(include_str!("../seeds/cufft.json"), &mut kb);
        // Should have cuBLAS + NPP + cuDNN + cuFFT entries
        assert!(
            kb.functions.len() >= 20,
            "merged CUDA KB should have 20+ entries, got {}",
            kb.functions.len()
        );
        let caps: Vec<String> = kb
            .functions
            .values()
            .flat_map(|e| e.capabilities.iter().cloned())
            .collect();
        assert!(
            caps.contains(&"blas_gemm".to_string()),
            "should have BLAS caps"
        );
        assert!(
            caps.contains(&"cudnn_conv".to_string()),
            "should have cuDNN caps"
        );
        assert!(
            caps.contains(&"cufft_exec".to_string()),
            "should have cuFFT caps"
        );
    }

    // =========================================================================
    // Hailo VDMA tests
    // =========================================================================

    #[test]
    fn test_seed_hailo_not_empty() {
        let kb = seed_hailo();
        assert!(
            !kb.functions.is_empty(),
            "seed_hailo() should return non-empty KB"
        );
        assert_eq!(
            kb.functions.len(),
            10,
            "should have 10 entries (9 VDMA + 1 FwControl)"
        );
        assert_eq!(kb.framework, "Hailo_VDMA");
    }

    #[test]
    fn test_hailo_dma_capability() {
        let kb = seed_hailo();
        let transfer = kb.query_capability("dma_transfer");
        assert!(
            transfer.len() >= 3,
            "should have dma_transfer entries, got {}",
            transfer.len()
        );
        assert!(transfer.iter().any(|e| e.name == "EnableChannels"));
        assert!(transfer.iter().any(|e| e.name == "LaunchTransfer"));
    }

    #[test]
    fn test_hailo_protocol_steps() {
        let kb = seed_hailo();
        assert_eq!(kb.protocols.len(), 1, "should have one VDMA init protocol");
        let proto = &kb.protocols[0];
        assert_eq!(proto.group, "VDMA_INIT");
        assert_eq!(proto.steps.len(), 6, "VDMA protocol should have 6 steps");
        assert!(proto.validated);
    }

    #[test]
    fn test_hailo_protocol_prerequisites() {
        let kb = seed_hailo();
        let proto = &kb.protocols[0];
        assert!(
            proto.prerequisites.len() >= 2,
            "should have at least 2 prerequisites"
        );
        assert!(
            proto
                .prerequisites
                .iter()
                .any(|p| { p.kind == super::crate::protocol::PrereqKind::FrameworkLoad }),
            "should require kernel module"
        );
        assert!(
            proto
                .prerequisites
                .iter()
                .any(|p| { p.kind == super::crate::protocol::PrereqKind::DevicePath }),
            "should require device path"
        );
    }

    #[test]
    fn test_nvidia_constants_defined() {
        use crate::crate::nvidia_constants::*;
        // Verify key constants match expected values from source repos
        assert_eq!(NV_ESC_RM_ALLOC, 0x2B);
        assert_eq!(NV_ESC_RM_CONTROL, 0x2A);
        assert_eq!(NV01_ROOT_CLIENT, 0x0041);
        assert_eq!(AMPERE_CHANNEL_GPFIFO_A, 0xC46F);
        assert_eq!(NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN, 0xC36F0108);
        assert_eq!(HAILO_MAX_VDMA_ENGINES, 3);
        assert_eq!(HAILO_MAX_CHANNELS_PER_ENGINE, 32);
        assert_eq!(HAILO_FW_CONTROL_BUFFER_LEN, 1500);
    }

    #[test]
    fn test_validate_cache_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("kb");
        // First call: no version file → creates it, returns false
        assert!(!FrameworkKB::validate_cache(&dir));
        assert!(dir.join(".version").exists());
        // Second call: version matches → returns true
        assert!(FrameworkKB::validate_cache(&dir));
    }

    #[test]
    fn test_validate_cache_wipes_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("kb");
        std::fs::create_dir_all(&dir).unwrap();
        // Write an old version
        std::fs::write(dir.join(".version"), "1").unwrap();
        // Write a stale JSON
        std::fs::write(dir.join("OldFramework.json"), "{}").unwrap();
        assert!(dir.join("OldFramework.json").exists());
        // Validate: old version → wipe JSONs, write new version
        assert!(!FrameworkKB::validate_cache(&dir));
        assert!(!dir.join("OldFramework.json").exists());
        let ver = std::fs::read_to_string(dir.join(".version")).unwrap();
        assert_eq!(ver, FrameworkKB::CACHE_VERSION.to_string());
    }

    // =========================================================================
    // Device-aware seed loading
    // =========================================================================

    fn fake_device(
        platform: &str,
        frameworks: &[&str],
        gpu_vendor: Option<&str>,
    ) -> crate::device::DeviceProfile {
        crate::device::DeviceProfile {
            platform: platform.into(),
            cpu: crate::device::CpuInfo {
                name: "test".into(),
                cores_physical: 1,
                cores_logical: 1,
                features: vec![],
            },
            gpus: gpu_vendor
                .map(|v| {
                    vec![crate::device::GpuInfo {
                        name: "test".into(),
                        vendor: v.into(),
                        vram_bytes: 0,
                        compute_units: 0,
                        features: vec![],
                    }]
                })
                .unwrap_or_default(),
            accelerators: vec![],
            frameworks: frameworks
                .iter()
                .map(|name| crate::device::FrameworkInfo {
                    name: name.to_string(),
                    path: String::new(),
                    is_private: false,
                })
                .collect(),
            memory: crate::device::memory::DeviceMemoryModel {
                total_bytes: 0,
                unified: false,
                memory_types: vec![],
                zerocopy_paths: vec![],
                transfer_costs: vec![],
            },
            thermal: crate::device::ThermalInfo::default(),
        }
    }

    #[test]
    fn test_load_seeds_for_device_macos() {
        let device = fake_device("macos", &["Accelerate", "VideoToolbox", "IOSurface"], None);
        let kbs = load_seeds_for_device(&device);
        let names: Vec<&str> = kbs.iter().map(|k| k.framework.as_str()).collect();
        assert!(
            names.contains(&"Accelerate"),
            "should load Accelerate: {names:?}"
        );
        assert!(
            names.contains(&"VideoToolbox"),
            "should load VideoToolbox: {names:?}"
        );
        assert!(
            names.contains(&"IOSurface"),
            "should load IOSurface: {names:?}"
        );
        assert!(
            names.contains(&"POSIX"),
            "should always load POSIX on macOS: {names:?}"
        );
        assert!(
            names.contains(&"CommonCrypto"),
            "should load CommonCrypto on macOS: {names:?}"
        );
        // Should NOT load NVIDIA or Hailo (no GPU/accelerator detected)
        assert!(
            !names.contains(&"NVIDIA_RM"),
            "no NVIDIA without GPU: {names:?}"
        );
        assert!(
            !names.contains(&"Hailo_VDMA"),
            "no Hailo without accelerator: {names:?}"
        );
    }

    #[test]
    fn test_load_seeds_for_device_linux_no_frameworks() {
        let device = fake_device("linux", &[], None);
        let kbs = load_seeds_for_device(&device);
        let names: Vec<&str> = kbs.iter().map(|k| k.framework.as_str()).collect();
        // Linux with no frameworks → only POSIX
        assert!(
            names.contains(&"POSIX"),
            "should load POSIX on Linux: {names:?}"
        );
        assert!(
            !names.contains(&"Accelerate"),
            "no Apple frameworks on Linux: {names:?}"
        );
        assert!(
            !names.contains(&"CommonCrypto"),
            "no CommonCrypto on Linux: {names:?}"
        );
    }

    #[test]
    fn test_load_seeds_for_device_nvidia() {
        let device = fake_device("linux", &[], Some("NVIDIA Corporation"));
        let kbs = load_seeds_for_device(&device);
        let names: Vec<&str> = kbs.iter().map(|k| k.framework.as_str()).collect();
        assert!(
            names.contains(&"NVIDIA_RM"),
            "should load NVIDIA seeds when GPU detected: {names:?}"
        );
        assert!(
            names.contains(&"POSIX"),
            "should still load POSIX: {names:?}"
        );
    }

    #[test]
    fn test_load_seeds_for_device_unknown_platform() {
        let device = fake_device("unknown", &[], None);
        let kbs = load_seeds_for_device(&device);
        assert!(
            kbs.is_empty(),
            "unknown platform → no seeds: {:?}",
            kbs.iter().map(|k| &k.framework).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_load_seed_kb_centralized() {
        // Verify the centralized load_seed_kb matches the old behavior
        let kb = load_seed_kb("Accelerate").expect("should have Accelerate seeds");
        assert!(kb.functions.contains_key("vImageScale_ARGB8888"));
        assert!(kb.functions.contains_key("cblas_sgemm"));
        let metal_kb = load_seed_kb("Metal").expect("should have Metal compute seeds");
        assert!(metal_kb
            .functions
            .contains_key("MTLCreateSystemDefaultDevice"));
    }

    #[test]
    fn test_vdsp_seed_loads_into_accelerate() {
        let kb = load_seed_kb("Accelerate").expect("should have Accelerate seeds");
        assert!(
            kb.functions.contains_key("vDSP_vsmsa"),
            "should have vDSP_vsmsa"
        );
        assert!(
            kb.functions.contains_key("vDSP_vfltu8"),
            "should have vDSP_vfltu8"
        );
        assert!(
            kb.functions.contains_key("vDSP_vsmul"),
            "should have vDSP_vsmul"
        );
        // vDSP functions should have vector_normalize capability
        let caps = kb.capabilities.get("vector_normalize");
        assert!(caps.is_some(), "should have vector_normalize capability");
        let names = caps.unwrap();
        assert!(names.contains(&"vDSP_vsmsa".to_string()));
        assert!(names.contains(&"vDSP_vfltu8".to_string()));
    }

    #[test]
    fn test_recipes_for_capability_image_scale() {
        let kb = load_seed_kb("Accelerate").expect("should have Accelerate seeds");
        let recipes = kb.recipes_for_capability("image_scale");
        assert!(!recipes.is_empty(), "should have recipes for image_scale");
        let groups: Vec<&str> = recipes.iter().map(|(g, _)| *g).collect();
        assert!(
            groups.contains(&"vImageScale_ARGB8888"),
            "should include vImageScale recipe"
        );
        // Check the recipe contains working code
        let (_, recipe) = recipes
            .iter()
            .find(|(g, _)| *g == "vImageScale_ARGB8888")
            .unwrap();
        assert!(recipe.contains("ctypes"), "recipe should use ctypes");
        assert!(
            recipe.contains("vImage_Buffer"),
            "recipe should define vImage_Buffer struct"
        );
    }

    #[test]
    fn test_recipes_for_capability_vector_normalize() {
        let kb = load_seed_kb("Accelerate").expect("should have Accelerate seeds");
        let recipes = kb.recipes_for_capability("vector_normalize");
        assert!(
            !recipes.is_empty(),
            "should have recipes for vector_normalize"
        );
        let groups: Vec<&str> = recipes.iter().map(|(g, _)| *g).collect();
        assert!(
            groups.contains(&"vDSP_vsmsa"),
            "should include vDSP_vsmsa recipe"
        );
        let (_, recipe) = recipes.iter().find(|(g, _)| *g == "vDSP_vsmsa").unwrap();
        assert!(
            recipe.contains("vdsp_normalize_channel"),
            "recipe should define normalize function"
        );
        assert!(
            recipe.contains("vDSP_vfltu8"),
            "recipe should chain vfltu8 for unsigned uint8→float"
        );
    }

    #[test]
    fn test_recipes_for_capability_nonexistent() {
        let kb = load_seed_kb("Accelerate").expect("should have Accelerate seeds");
        let recipes = kb.recipes_for_capability("quantum_teleport");
        assert!(recipes.is_empty());
    }

    #[test]
    fn test_metal_compute_seed_has_dispatch_harness() {
        let kb = load_seed_kb("Metal").expect("should have Metal seeds");
        // Metal protocols should include the dispatch harness
        assert!(!kb.protocols.is_empty(), "should have protocols");
        let harness = kb
            .protocols
            .iter()
            .find(|p| p.group == "MetalDispatchHarness");
        assert!(
            harness.is_some(),
            "should have MetalDispatchHarness protocol"
        );
        let recipe = harness.unwrap().recipe.as_deref().unwrap();
        assert!(
            recipe.contains("class MetalDispatch"),
            "should define reusable dispatch class"
        );
        assert!(
            recipe.contains("newBufferWithBytesNoCopy"),
            "should use zero-copy buffers"
        );
    }

    #[test]
    fn test_metal_compute_seed_has_kernel_snippets() {
        let kb = load_seed_kb("Metal").expect("should have Metal seeds");
        let snippets = kb
            .protocols
            .iter()
            .find(|p| p.group == "MetalKernelSnippets");
        assert!(
            snippets.is_some(),
            "should have MetalKernelSnippets protocol"
        );
        let recipe = snippets.unwrap().recipe.as_deref().unwrap();
        assert!(
            recipe.contains("RESIZE_BILINEAR_MSL"),
            "should have resize snippet"
        );
        assert!(
            recipe.contains("BGRA_TO_RGB_FLOAT_MSL"),
            "should have color convert snippet"
        );
        assert!(
            recipe.contains("NORMALIZE_CHW_MSL"),
            "should have normalize snippet"
        );
        assert!(
            recipe.contains("build_resize_normalize_kernel"),
            "should have composition example"
        );
    }

    #[test]
    #[ignore] // Run manually: cargo test dump_seeds_to_json -- --ignored --nocapture
    fn dump_seeds_to_json() {
        let seeds_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("seeds");
        std::fs::create_dir_all(&seeds_dir).unwrap();
        // (framework_name, seed_fn, filename)
        let dump = |_name: &str, filename: &str, kb: &FrameworkKB| {
            let seed = kb_to_seed_file(kb);
            let json = serde_json::to_string_pretty(&seed).unwrap();
            std::fs::write(seeds_dir.join(filename), json).unwrap();
            eprintln!(
                "Wrote {}: {} entries, {} protocols",
                filename,
                kb.functions.len(),
                kb.protocols.len()
            );
        };
        // Accelerate vImage
        let mut kb = FrameworkKB::new("Accelerate");
        seed_accelerate_vimage(&mut kb);
        dump("Accelerate", "accelerate_vimage.json", &kb);
        // Accelerate BLAS
        let mut kb = FrameworkKB::new("Accelerate");
        seed_accelerate_blas(&mut kb);
        dump("Accelerate", "accelerate_blas.json", &kb);
        // VideoToolbox
        let mut kb = FrameworkKB::new("VideoToolbox");
        seed_videotoolbox(&mut kb);
        dump("VideoToolbox", "videotoolbox.json", &kb);
        // POSIX
        let mut kb = FrameworkKB::new("POSIX");
        seed_posix_io(&mut kb);
        dump("POSIX", "posix_io.json", &kb);
        // CommonCrypto
        let mut kb = FrameworkKB::new("CommonCrypto");
        seed_commoncrypto(&mut kb);
        dump("CommonCrypto", "commoncrypto.json", &kb);
        // Security
        let mut kb = FrameworkKB::new("Security");
        seed_security_framework(&mut kb);
        dump("Security", "security.json", &kb);
        // Network
        let mut kb = FrameworkKB::new("Network");
        seed_network_framework(&mut kb);
        dump("Network", "network.json", &kb);
        // IOSurface
        let mut kb = FrameworkKB::new("IOSurface");
        seed_iosurface(&mut kb);
        dump("IOSurface", "iosurface.json", &kb);
        // kperf
        let mut kb = FrameworkKB::new("kperf");
        seed_kperf(&mut kb);
        dump("kperf", "kperf.json", &kb);
        // MPS
        let mut kb = FrameworkKB::new("MetalPerformanceShaders");
        seed_mps(&mut kb);
        dump("MPS", "mps.json", &kb);
        // NVIDIA
        let kb = seed_nvidia();
        dump("NVIDIA_RM", "nvidia_rm.json", &kb);
        // Hailo
        let kb = seed_hailo();
        dump("Hailo_VDMA", "hailo_vdma.json", &kb);
        eprintln!("All seeds dumped to {:?}", seeds_dir);
    }

    #[test]
    fn test_json_seed_roundtrip() {
        // Verify that loading from JSON produces the same KB as the original seed functions
        let check = |name: &str, json: &str, original: &FrameworkKB| {
            let mut loaded = FrameworkKB::new(name);
            load_seed_json(json, &mut loaded);
            assert_eq!(
                loaded.functions.len(),
                original.functions.len(),
                "{name}: function count mismatch: loaded={} original={}",
                loaded.functions.len(),
                original.functions.len()
            );
            for (fname, orig_entry) in &original.functions {
                let loaded_entry = loaded
                    .functions
                    .get(fname)
                    .unwrap_or_else(|| panic!("{name}: missing function {fname}"));
                assert_eq!(
                    loaded_entry.capabilities, orig_entry.capabilities,
                    "{name}/{fname}: capabilities mismatch"
                );
                assert_eq!(
                    loaded_entry.return_type, orig_entry.return_type,
                    "{name}/{fname}: return_type mismatch"
                );
                assert_eq!(
                    loaded_entry.parameters.len(),
                    orig_entry.parameters.len(),
                    "{name}/{fname}: param count mismatch"
                );
                assert_eq!(
                    loaded_entry.async_pattern, orig_entry.async_pattern,
                    "{name}/{fname}: async mismatch"
                );
                assert_eq!(
                    loaded_entry.confidence(),
                    orig_entry.confidence(),
                    "{name}/{fname}: confidence mismatch"
                );
            }
            assert_eq!(
                loaded.protocols.len(),
                original.protocols.len(),
                "{name}: protocol count mismatch"
            );
        };
        // Test each seed
        let mut kb = FrameworkKB::new("Security");
        seed_security_framework(&mut kb);
        check(
            "Security",
            include_str!("../seeds/security.json"),
            &kb,
        );

        let mut kb = FrameworkKB::new("POSIX");
        seed_posix_io(&mut kb);
        check("POSIX", include_str!("../seeds/posix_io.json"), &kb);

        let mut kb = FrameworkKB::new("CommonCrypto");
        seed_commoncrypto(&mut kb);
        check(
            "CommonCrypto",
            include_str!("../seeds/commoncrypto.json"),
            &kb,
        );

        let mut kb = FrameworkKB::new("Accelerate");
        seed_accelerate_vimage(&mut kb);
        check(
            "vImage",
            include_str!("../seeds/accelerate_vimage.json"),
            &kb,
        );

        let mut kb = FrameworkKB::new("Accelerate");
        seed_accelerate_blas(&mut kb);
        check(
            "BLAS",
            include_str!("../seeds/accelerate_blas.json"),
            &kb,
        );

        let mut kb = FrameworkKB::new("VideoToolbox");
        seed_videotoolbox(&mut kb);
        check(
            "VideoToolbox",
            include_str!("../seeds/videotoolbox.json"),
            &kb,
        );

        let mut kb = FrameworkKB::new("Network");
        seed_network_framework(&mut kb);
        check("Network", include_str!("../seeds/network.json"), &kb);

        let mut kb = FrameworkKB::new("IOSurface");
        seed_iosurface(&mut kb);
        check(
            "IOSurface",
            include_str!("../seeds/iosurface.json"),
            &kb,
        );

        let mut kb = FrameworkKB::new("kperf");
        seed_kperf(&mut kb);
        check("kperf", include_str!("../seeds/kperf.json"), &kb);

        let mut kb = FrameworkKB::new("MetalPerformanceShaders");
        seed_mps(&mut kb);
        check("MPS", include_str!("../seeds/mps.json"), &kb);

        let kb = seed_nvidia();
        check(
            "NVIDIA_RM",
            include_str!("../seeds/nvidia_rm.json"),
            &kb,
        );

        let kb = seed_hailo();
        check(
            "Hailo_VDMA",
            include_str!("../seeds/hailo_vdma.json"),
            &kb,
        );
    }

    // =========================================================================
    // CUDA cuBLAS + NPP tests
    // =========================================================================

    #[test]
    fn test_seed_cublas_entries() {
        let kb = seed_cublas();
        assert_eq!(kb.framework, "CUDA");
        assert!(
            kb.functions.len() >= 5,
            "cublas should have at least 5 entries, got {}",
            kb.functions.len()
        );
        // Key entries exist
        assert!(kb.query_name("cublasSgemm").is_some());
        assert!(kb.query_name("cublasDgemm").is_some());
        assert!(kb.query_name("cublasSnrm2").is_some());
        assert!(kb.query_name("cublasCreate").is_some());
        assert!(kb.query_name("cublasSgemmStridedBatched").is_some());
        // cublasSgemm has blas_gemm capability
        let sgemm = kb.query_name("cublasSgemm").unwrap();
        assert!(
            sgemm.capabilities.contains(&"blas_gemm".to_string()),
            "cublasSgemm should have blas_gemm capability, got {:?}",
            sgemm.capabilities
        );
        // At least one protocol exists (CublasInit)
        assert!(
            !kb.protocols.is_empty(),
            "cublas should have at least one protocol"
        );
        assert!(
            kb.protocols.iter().any(|p| p.group == "CublasInit"),
            "should have CublasInit protocol"
        );
    }

    #[test]
    fn test_seed_npp_entries() {
        let kb = seed_npp();
        assert_eq!(kb.framework, "CUDA");
        assert!(
            kb.functions.len() >= 3,
            "npp should have at least 3 entries, got {}",
            kb.functions.len()
        );
        // Key entries exist
        assert!(kb.query_name("nppiResize_8u_C3R").is_some());
        assert!(kb.query_name("nppiFilterGauss_8u_C3R").is_some());
        assert!(kb.query_name("nppiNormL2_32f_C1R").is_some());
        // nppiResize has gpu_image_scale capability
        let resize = kb.query_name("nppiResize_8u_C3R").unwrap();
        assert!(
            resize.capabilities.contains(&"gpu_image_scale".to_string()),
            "nppiResize should have gpu_image_scale capability, got {:?}",
            resize.capabilities
        );
        // At least one protocol exists (NppImagePipeline)
        assert!(
            !kb.protocols.is_empty(),
            "npp should have at least one protocol"
        );
        assert!(
            kb.protocols.iter().any(|p| p.group == "NppImagePipeline"),
            "should have NppImagePipeline protocol"
        );
    }

    #[test]
    fn test_load_seed_kb_cuda_merged() {
        let kb = load_seed_kb("CUDA").expect("should have CUDA seeds");
        assert_eq!(kb.framework, "CUDA");
        // Should have entries from BOTH cublas and npp
        assert!(
            kb.functions.len() >= 8,
            "CUDA KB should have at least 8 entries (5 cublas + 3 npp), got {}",
            kb.functions.len()
        );
        // cublas entries present
        assert!(
            kb.query_name("cublasSgemm").is_some(),
            "missing cublasSgemm"
        );
        assert!(
            kb.query_name("cublasCreate").is_some(),
            "missing cublasCreate"
        );
        // npp entries present
        assert!(
            kb.query_name("nppiResize_8u_C3R").is_some(),
            "missing nppiResize_8u_C3R"
        );
        assert!(
            kb.query_name("nppiFilterGauss_8u_C3R").is_some(),
            "missing nppiFilterGauss_8u_C3R"
        );
        // Protocols from both
        assert!(
            kb.protocols.len() >= 2,
            "should have protocols from both cublas and npp, got {}",
            kb.protocols.len()
        );
        assert!(
            kb.protocols.iter().any(|p| p.group == "CublasInit"),
            "should have CublasInit protocol"
        );
        assert!(
            kb.protocols.iter().any(|p| p.group == "NppImagePipeline"),
            "should have NppImagePipeline protocol"
        );
    }

    #[test]
    fn test_load_seeds_for_device_nvidia_includes_cuda() {
        let device = fake_device("linux", &[], Some("NVIDIA Corporation"));
        let kbs = load_seeds_for_device(&device);
        let names: Vec<&str> = kbs.iter().map(|k| k.framework.as_str()).collect();
        // Should have CUDA KB
        assert!(
            names.contains(&"CUDA"),
            "should load CUDA KB when NVIDIA GPU detected: {names:?}"
        );
        // Find the CUDA KB and verify it has entries from both cublas and npp
        let cuda_kb = kbs.iter().find(|k| k.framework == "CUDA").unwrap();
        assert!(
            cuda_kb.query_name("cublasSgemm").is_some(),
            "CUDA KB should have cublasSgemm"
        );
        assert!(
            cuda_kb.query_name("nppiResize_8u_C3R").is_some(),
            "CUDA KB should have nppiResize_8u_C3R from NPP merge"
        );
        assert!(
            cuda_kb.functions.len() >= 8,
            "CUDA KB should have entries from both cublas and npp, got {}",
            cuda_kb.functions.len()
        );
    }

    #[test]
    fn test_cublas_protocol_constants_parsed() {
        let kb = seed_cublas();
        let cublas_init = kb
            .protocols
            .iter()
            .find(|p| p.group == "CublasInit")
            .expect("should have CublasInit protocol");
        // Should have 3 constants: CUBLAS_OP_N, CUBLAS_OP_T, CUBLAS_TENSOR_OP_MATH
        assert!(
            !cublas_init.constants.is_empty(),
            "CublasInit should have constants"
        );
        assert_eq!(
            cublas_init.constants.len(),
            3,
            "CublasInit should have 3 constants, got {}",
            cublas_init.constants.len()
        );
        // Verify fields on the first constant
        let names: Vec<&str> = cublas_init
            .constants
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(names.contains(&"CUBLAS_OP_N"), "should have CUBLAS_OP_N");
        assert!(names.contains(&"CUBLAS_OP_T"), "should have CUBLAS_OP_T");
        assert!(
            names.contains(&"CUBLAS_TENSOR_OP_MATH"),
            "should have CUBLAS_TENSOR_OP_MATH"
        );
        // Verify proper name/value/source fields on a specific constant
        let op_n = cublas_init
            .constants
            .iter()
            .find(|c| c.name == "CUBLAS_OP_N")
            .unwrap();
        assert_eq!(op_n.value, "0", "CUBLAS_OP_N value should be 0");
        assert_eq!(
            op_n.source, "cublas_v2.h",
            "CUBLAS_OP_N source should be cublas_v2.h"
        );
    }

    // =========================================================================
    // CoreML tests
    // =========================================================================

    #[test]
    fn test_load_seed_kb_coreml() {
        let kb = load_seed_kb("CoreML").expect("should have CoreML seeds");
        assert!(
            kb.functions.len() >= 5,
            "should have at least 5 CoreML entries"
        );
        assert!(
            kb.query_name("MLModel.predict").is_some(),
            "should have MLModel.predict"
        );
        assert!(
            kb.query_name("ct.converters.convert").is_some(),
            "should have ct.converters.convert"
        );
    }

    #[test]
    fn test_coreml_predict_protocol() {
        let kb = load_seed_kb("CoreML").expect("should have CoreML seeds");
        let predict_protocol = kb.protocols.iter().find(|p| p.group == "coreml_predict");
        assert!(
            predict_protocol.is_some(),
            "should have coreml_predict protocol"
        );
        let proto = predict_protocol.unwrap();
        assert_eq!(
            proto.steps.len(),
            2,
            "predict protocol should have 2 steps (load + predict)"
        );
        assert!(proto.recipe.is_some(), "should have a recipe");
        assert!(
            proto.recipe.as_ref().unwrap().contains("coremltools"),
            "recipe should use coremltools"
        );
    }

    #[test]
    fn test_coreml_recipes_for_capability() {
        let kb = load_seed_kb("CoreML").expect("should have CoreML seeds");
        let recipes = kb.recipes_for_capability("coreml_predict");
        assert!(
            !recipes.is_empty(),
            "should have recipes for coreml_predict capability"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_coreml_loaded_for_macos_device() {
        let device = crate::device::discover();
        if device.platform == "macos" {
            let kbs = load_seeds_for_device(&device);
            let has_coreml = kbs.iter().any(|kb| kb.framework == "CoreML");
            assert!(has_coreml, "macOS device should load CoreML KB");
        }
    }

    // =========================================================================
    // cuDNN/cuFFT protocol tests
    // =========================================================================

    #[test]
    fn test_cudnn_conv_protocol_recipe() {
        let mut kb = FrameworkKB::new("CUDA");
        load_seed_json(include_str!("../seeds/cudnn.json"), &mut kb);
        let conv = kb.protocols.iter().find(|p| p.group == "CudnnConvForward");
        assert!(conv.is_some(), "should have CudnnConvForward protocol");
        let proto = conv.unwrap();
        assert!(proto.recipe.is_some(), "should have recipe");
        assert!(
            proto
                .recipe
                .as_ref()
                .unwrap()
                .contains("cudnnConvolutionForward"),
            "recipe should call cudnnConvolutionForward"
        );
    }

    #[test]
    fn test_cufft_r2c_protocol_recipe() {
        let mut kb = FrameworkKB::new("CUDA");
        load_seed_json(include_str!("../seeds/cufft.json"), &mut kb);
        let r2c = kb.protocols.iter().find(|p| p.group == "CufftR2C");
        assert!(r2c.is_some(), "should have CufftR2C protocol");
        let proto = r2c.unwrap();
        assert!(proto.recipe.is_some(), "should have recipe");
        assert!(
            proto.recipe.as_ref().unwrap().contains("cufftExecR2C"),
            "recipe should call cufftExecR2C"
        );
    }

    // =========================================================================
    // ROCm/HIP tests
    // =========================================================================

    #[test]
    fn test_rocm_seed_loads() {
        let kb = seed_rocm();
        assert_eq!(kb.framework, "ROCm");
        assert_eq!(
            kb.functions.len(),
            11,
            "ROCm should have 11 entries (4 hipBLAS + 4 hipFFT + 3 hip memory), got {}",
            kb.functions.len()
        );
        // hipBLAS entries
        assert!(
            kb.query_name("hipblasCreate").is_some(),
            "missing hipblasCreate"
        );
        assert!(
            kb.query_name("hipblasSgemm").is_some(),
            "missing hipblasSgemm"
        );
        assert!(
            kb.query_name("hipblasDgemm").is_some(),
            "missing hipblasDgemm"
        );
        assert!(
            kb.query_name("hipblasDestroy").is_some(),
            "missing hipblasDestroy"
        );
        // hipFFT entries
        assert!(
            kb.query_name("hipfftPlan1d").is_some(),
            "missing hipfftPlan1d"
        );
        assert!(
            kb.query_name("hipfftExecR2C").is_some(),
            "missing hipfftExecR2C"
        );
        assert!(
            kb.query_name("hipfftExecC2C").is_some(),
            "missing hipfftExecC2C"
        );
        assert!(
            kb.query_name("hipfftDestroy").is_some(),
            "missing hipfftDestroy"
        );
        // HIP memory entries
        assert!(kb.query_name("hipMalloc").is_some(), "missing hipMalloc");
        assert!(kb.query_name("hipMemcpy").is_some(), "missing hipMemcpy");
        assert!(kb.query_name("hipFree").is_some(), "missing hipFree");
        // Protocols
        assert_eq!(
            kb.protocols.len(),
            2,
            "should have 2 protocols (HipBlasGemm + HipFftR2C)"
        );
        assert!(
            kb.protocols.iter().any(|p| p.group == "HipBlasGemm"),
            "should have HipBlasGemm protocol"
        );
        assert!(
            kb.protocols.iter().any(|p| p.group == "HipFftR2C"),
            "should have HipFftR2C protocol"
        );
    }

    #[test]
    fn test_rocm_capabilities() {
        let kb = seed_rocm();
        // rocm_blas_gemm capability
        let blas = kb.query_capability("rocm_blas_gemm");
        assert!(
            blas.len() >= 4,
            "should have rocm_blas_gemm entries (create + sgemm + dgemm + destroy + memory), got {}",
            blas.len()
        );
        assert!(blas.iter().any(|e| e.name == "hipblasSgemm"));
        assert!(blas.iter().any(|e| e.name == "hipblasDgemm"));
        // rocm_fft capability
        let fft = kb.query_capability("rocm_fft");
        assert!(
            fft.len() >= 3,
            "should have rocm_fft entries, got {}",
            fft.len()
        );
        assert!(fft.iter().any(|e| e.name == "hipfftExecR2C"));
        // gpu_memory capability
        let mem = kb.query_capability("gpu_memory");
        assert!(
            mem.len() >= 2,
            "should have gpu_memory entries, got {}",
            mem.len()
        );
        assert!(mem.iter().any(|e| e.name == "hipMalloc"));
    }

    #[test]
    fn test_rocm_protocol_recipes() {
        let kb = seed_rocm();
        let blas_proto = kb
            .protocols
            .iter()
            .find(|p| p.group == "HipBlasGemm")
            .unwrap();
        assert!(
            blas_proto.recipe.is_some(),
            "HipBlasGemm should have recipe"
        );
        assert!(
            blas_proto.recipe.as_ref().unwrap().contains("hipblasSgemm"),
            "recipe should call hipblasSgemm"
        );
        let fft_proto = kb
            .protocols
            .iter()
            .find(|p| p.group == "HipFftR2C")
            .unwrap();
        assert!(fft_proto.recipe.is_some(), "HipFftR2C should have recipe");
        assert!(
            fft_proto.recipe.as_ref().unwrap().contains("hipfftExecR2C"),
            "recipe should call hipfftExecR2C"
        );
    }

    #[test]
    fn test_rocm_confidence() {
        let kb = seed_rocm();
        for entry in kb.functions.values() {
            assert_eq!(
                entry.confidence(),
                1.0,
                "{} should have confidence 1.0",
                entry.name
            );
        }
    }

    #[test]
    fn test_load_seeds_for_device_rocm() {
        let device = fake_device("linux", &[], Some("Advanced Micro Devices"));
        let kbs = load_seeds_for_device(&device);
        let names: Vec<&str> = kbs.iter().map(|k| k.framework.as_str()).collect();
        assert!(
            names.contains(&"ROCm"),
            "should load ROCm seeds when AMD GPU detected: {names:?}"
        );
        assert!(
            names.contains(&"POSIX"),
            "should still load POSIX: {names:?}"
        );
        // Verify the ROCm KB has entries
        let rocm_kb = kbs.iter().find(|k| k.framework == "ROCm").unwrap();
        assert!(
            rocm_kb.query_name("hipblasSgemm").is_some(),
            "ROCm KB should have hipblasSgemm"
        );
    }

    #[test]
    fn test_load_seed_kb_rocm() {
        let kb = load_seed_kb("ROCm").expect("should have ROCm seeds");
        assert_eq!(kb.framework, "ROCm");
        assert!(
            kb.functions.len() >= 11,
            "ROCm KB should have at least 11 entries, got {}",
            kb.functions.len()
        );
        assert!(kb.query_name("hipblasSgemm").is_some());
        assert!(kb.query_name("hipfftExecR2C").is_some());
        assert!(kb.query_name("hipMalloc").is_some());
    }
}
