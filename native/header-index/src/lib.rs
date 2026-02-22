//! Framework API discovery and shim generation.
//!
//! Extracted from McGyverAI's header_index pipeline.
//! 6-layer pipeline: extract → infer → kb → enrich → probe → shim
//!
//! Public API:
//!   HeaderIndex::new()                    — empty index
//!   HeaderIndex::extract_framework(name)  — full pipeline
//!   HeaderIndex::discover_platform()      — extract all frameworks on this machine
//!   HeaderIndex::query/search             — find APIs

pub mod config;
pub mod device;
pub mod enrich;
pub mod extract;
pub mod infer;
pub mod kb;
pub mod nvidia_constants;
pub mod probe;
pub mod protocol;
pub mod shim;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub use extract::{ParsedDecl, RawExport};
pub use infer::{CapabilityTag, InferredSig};
pub use kb::{ApiEntry, Evidence, EvidenceKind, FrameworkKB};

// =============================================================================
// Legacy types (kept for load_json compatibility)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSignature {
    pub name: String,
    pub framework: String,
    pub signature: String,
    pub return_type: String,
    pub parameters: Vec<Parameter>,
    pub import_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub type_name: String,
    pub is_pointer: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub apis: Vec<String>,
}

// =============================================================================
// HeaderIndex: the main entry point
// =============================================================================

#[allow(dead_code)]
pub struct HeaderIndex {
    known: HashMap<String, ApiSignature>,
    pub frameworks: HashMap<String, FrameworkKB>,
    capabilities: Vec<Capability>,
}

impl HeaderIndex {
    pub fn new() -> Self {
        Self {
            known: HashMap::new(),
            frameworks: HashMap::new(),
            capabilities: Vec::new(),
        }
    }

    /// Full extraction pipeline for a single framework.
    pub fn extract_framework(&mut self, name: &str) -> anyhow::Result<&FrameworkKB> {
        if self.frameworks.contains_key(name) {
            return Ok(&self.frameworks[name]);
        }

        // Try cache first
        let cache_dir = FrameworkKB::cache_dir();
        if let Ok(cached) = FrameworkKB::load(&cache_dir, name) {
            self.frameworks.insert(name.into(), cached);
            return Ok(&self.frameworks[name]);
        }

        let binary_path = extract::resolve_framework_binary(name)
            .ok_or_else(|| anyhow::anyhow!("framework not found: {name}"))?;

        // Layer 1: extract exports
        let from_dyld = extract::dyld_info_exports(&binary_path);
        let (exports, evidence_source) = if !from_dyld.is_empty() {
            (from_dyld, EvidenceKind::DyldExport)
        } else {
            (
                extract::macho_exports(Path::new(&binary_path)),
                EvidenceKind::MachoExport,
            )
        };

        // Layer 1b: clang AST for typed declarations
        let decl_map = build_decl_map(name);

        // Layer 2: infer signatures
        let sigs = infer::infer_all(&exports, &decl_map);

        // Layer 3: build KB with evidence
        let mut kb = FrameworkKB::new(name);
        for sig in &sigs {
            let mut evidence = vec![Evidence {
                kind: evidence_source.clone(),
                confidence: 0.8,
                detail: format!("exported from {name}"),
            }];
            if decl_map.contains_key(&sig.name) {
                evidence.push(Evidence {
                    kind: EvidenceKind::ClangAst,
                    confidence: 0.95,
                    detail: "clang AST typed".into(),
                });
            } else if sig.pattern != "unknown" {
                evidence.push(Evidence {
                    kind: EvidenceKind::NamingInference,
                    confidence: sig.confidence,
                    detail: format!("matched {} pattern", sig.pattern),
                });
            }
            kb.insert(ApiEntry::from_inferred(sig, name, evidence));
        }

        // Layer 2b: binary enrichment
        if let Some(path) = extract::resolve_framework_binary(name) {
            let enrichment = enrich::enrich_binary(Path::new(&path));
            let enriched = kb.apply_enrichment(&enrichment);
            if enriched > 0 {
                kb.insert_evidence_bulk(
                    EvidenceKind::RuntimeProbe,
                    0.6,
                    &format!(
                        "binary enrichment: {} ObjC classes, {} strings",
                        enrichment.objc_classes.len(),
                        enrichment.string_constants.len()
                    ),
                );
            }
        }

        let _ = kb.save(&cache_dir);
        self.frameworks.insert(name.into(), kb);
        Ok(&self.frameworks[name])
    }

    /// Discover APIs from all frameworks on this platform.
    pub fn discover_platform(&mut self) -> anyhow::Result<usize> {
        let profile = crate::device::discover();
        let mut count = 0usize;
        for fw in &profile.frameworks {
            let name = Path::new(&fw.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&fw.name);
            match self.extract_framework(name) {
                Ok(kb) => count += kb.functions.len(),
                Err(_) => continue,
            }
        }
        Ok(count)
    }

    /// Apply protocol feedback to a framework's KB.
    pub fn apply_protocol_feedback(
        &mut self,
        framework: &str,
        proto: &protocol::ProtocolEntry,
    ) -> usize {
        match self.frameworks.get_mut(framework) {
            Some(kb) => kb.apply_protocol_feedback(proto),
            None => 0,
        }
    }

    pub fn get(&self, name: &str) -> Option<&ApiSignature> {
        self.known.get(name)
    }

    pub fn get_entry(&self, name: &str) -> Option<&ApiEntry> {
        self.frameworks.values().find_map(|kb| kb.query_name(name))
    }

    pub fn search(&self, query: &str) -> Vec<&ApiSignature> {
        let q = query.to_lowercase();
        self.known
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&q) || s.framework.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn search_natural(&self, text: &str) -> Vec<(&ApiEntry, f64)> {
        let mut all: Vec<_> = self
            .frameworks
            .values()
            .flat_map(|kb| kb.query_natural(text))
            .collect();
        all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        all
    }

    pub fn find_by_capability(&self, keyword: &str) -> Vec<&ApiEntry> {
        let k = keyword.to_lowercase();
        self.frameworks
            .values()
            .flat_map(|kb| {
                kb.capabilities
                    .keys()
                    .filter(|c| c.contains(&k))
                    .flat_map(|c| kb.query_capability(c))
            })
            .collect()
    }

    pub fn to_llm_context(&self, name: &str) -> Option<String> {
        self.frameworks
            .values()
            .find_map(|kb| kb.to_llm_context(name))
    }

    pub fn load_json(&mut self, path: &Path) -> anyhow::Result<usize> {
        let sigs: Vec<ApiSignature> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let count = sigs.len();
        for sig in sigs {
            self.known.insert(sig.name.clone(), sig);
        }
        Ok(count)
    }

    pub fn load_json_zst(&mut self, path: &Path) -> anyhow::Result<usize> {
        let sigs: Vec<ApiSignature> =
            serde_json::from_slice(&zstd::decode_all(std::fs::read(path)?.as_slice())?)?;
        let count = sigs.len();
        for sig in sigs {
            self.known.insert(sig.name.clone(), sig);
        }
        Ok(count)
    }

    pub fn discover_all_frameworks(&mut self) -> (usize, usize, usize) {
        let (mut extracted, mut cached, mut errors) = (0, 0, 0);
        let config = crate::config::platform();
        for dir_path in [
            config.paths.public_frameworks.as_str(),
            config.paths.private_frameworks.as_str(),
        ] {
            let entries = match std::fs::read_dir(dir_path) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let name_str = entry.file_name().to_string_lossy().to_string();
                let fw_name = match name_str.strip_suffix(".framework") {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                if self.frameworks.contains_key(&fw_name) {
                    cached += 1;
                    continue;
                }
                match self.extract_framework(&fw_name) {
                    Ok(_) => extracted += 1,
                    Err(_) => errors += 1,
                }
            }
        }
        (extracted, cached, errors)
    }

    pub fn len(&self) -> usize {
        self.known.len()
            + self
                .frameworks
                .values()
                .map(|kb| kb.functions.len())
                .sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.known.is_empty() && self.frameworks.is_empty()
    }
}

fn current_os_build() -> String {
    crate::config::platform().os_build_version()
}

fn build_decl_map(framework: &str) -> HashMap<String, ParsedDecl> {
    let mut map = HashMap::new();
    let header = crate::config::platform().umbrella_header(framework);
    let path = Path::new(&header);
    if path.exists() {
        for decl in extract::clang_ast_declarations(path, framework) {
            map.insert(decl.name.clone(), decl);
        }
    }
    map
}
