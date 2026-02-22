#!/bin/bash
set -e

NATIVE_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$NATIVE_DIR/bin"
mkdir -p "$BIN_DIR"

echo "=== Building app-automation (Swift) ==="
cd "$NATIVE_DIR/app-automation"
swift build -c release -q
cp .build/release/app-automation "$BIN_DIR/"
cp .build/release/app-agent "$BIN_DIR/"

echo "=== Building swift-section (Swift) ==="
cd "$NATIVE_DIR/swift-section"
swift build -c release -q
cp .build/release/swift-section "$BIN_DIR/"
cp .build/release/swift-section-mcp "$BIN_DIR/"

echo "=== Building header-index (Rust) ==="
cd "$NATIVE_DIR/header-index"
cargo build --release -q
cp target/release/header-index "$BIN_DIR/"

echo "=== All native binaries built ==="
ls -lh "$BIN_DIR/"
