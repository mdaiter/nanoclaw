---
name: setup-macos-control
description: Build native binaries for macOS control (swift-section, header-index, app-automation). Run this before using macOS control features.
---

# Setup macOS Control

Build all native binaries required for PrivateFrameworks access.

## Prerequisites

- Xcode Command Line Tools (`xcode-select --install`)
- Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Accessibility permissions for Terminal (System Settings → Privacy → Accessibility)

## Build

```bash
./native/build.sh
```

This builds:

- `native/bin/app-automation` — UI automation via Accessibility APIs
- `native/bin/app-agent` — LLM-powered UI agent
- `native/bin/swift-section` — DYLD cache parsing + Swift interface generation
- `native/bin/swift-section-mcp` — MCP server for Swift metadata
- `native/bin/header-index` — API extraction, signature inference, shim generation

## Verify

```bash
native/bin/swift-section --help
native/bin/header-index seeds
native/bin/app-automation health
```
