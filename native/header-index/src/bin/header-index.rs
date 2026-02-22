//! CLI for header-index: JSON-in/JSON-out for nanoclaw integration.
//!
//! Usage:
//!   header-index extract <framework>           — full extraction pipeline
//!   header-index discover                      — discover all platform frameworks
//!   header-index query <text>                  — natural language search
//!   header-index capability <keyword>          — find by capability tag
//!   header-index shim <framework> <fn1,fn2>    — generate Rust shim dylib
//!   header-index probe <framework> <group>     — sandboxed protocol probe
//!   header-index seeds                         — load seed data, print stats

use header_index::{FrameworkKB, HeaderIndex};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: header-index <command> [args...]");
        eprintln!("Commands: extract, discover, query, capability, shim, probe, seeds");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "extract" => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: header-index extract <framework>");
                std::process::exit(1)
            });
            let mut idx = HeaderIndex::new();
            match idx.extract_framework(name) {
                Ok(kb) => {
                    let json = serde_json::json!({
                        "framework": name,
                        "functions": kb.functions.len(),
                        "capabilities": kb.capabilities.keys().collect::<Vec<_>>(),
                        "entries": kb.functions.values().take(50).map(|e| serde_json::json!({
                            "name": e.name, "signature": e.signature, "return_type": e.return_type,
                            "confidence": e.confidence(), "capabilities": e.capabilities,
                        })).collect::<Vec<_>>(),
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        "discover" => {
            let mut idx = HeaderIndex::new();
            let (extracted, cached, errors) = idx.discover_all_frameworks();
            let json = serde_json::json!({
                "extracted": extracted, "cached": cached, "errors": errors,
                "frameworks": idx.frameworks.keys().collect::<Vec<_>>(),
                "total_apis": idx.len(),
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        "query" => {
            let text = args[2..].join(" ");
            let mut idx = HeaderIndex::new();
            // Load cached KBs
            let cache_dir = FrameworkKB::cache_dir();
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(fw) = name.strip_suffix(".json") {
                        if let Ok(kb) = FrameworkKB::load(&cache_dir, fw) {
                            idx.frameworks.insert(fw.to_string(), kb);
                        }
                    }
                }
            }
            let results = idx.search_natural(&text);
            let json: Vec<_> = results.iter().take(20).map(|(e, score)| serde_json::json!({
                "name": e.name, "framework": e.framework, "signature": e.signature,
                "score": score, "confidence": e.confidence(), "capabilities": e.capabilities,
            })).collect();
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        "capability" => {
            let keyword = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let mut idx = HeaderIndex::new();
            let cache_dir = FrameworkKB::cache_dir();
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(fw) = name.strip_suffix(".json") {
                        if let Ok(kb) = FrameworkKB::load(&cache_dir, fw) {
                            idx.frameworks.insert(fw.to_string(), kb);
                        }
                    }
                }
            }
            let results = idx.find_by_capability(keyword);
            let json: Vec<_> = results
                .iter()
                .take(20)
                .map(|e| {
                    serde_json::json!({
                        "name": e.name, "framework": e.framework, "signature": e.signature,
                        "confidence": e.confidence(), "capabilities": e.capabilities,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        "seeds" => {
            let kbs = header_index::kb::load_seeds_for_device(&header_index::device::discover());
            let json = serde_json::json!({
                "seed_count": kbs.len(),
                "seeds": kbs.iter().map(|kb| serde_json::json!({
                    "framework": &kb.framework, "functions": kb.functions.len(),
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
}
