#!/usr/bin/env bash
# ==============================================================================
# 2D Minecraft & 3D Voxel Suite - Local Build & Package Script
# ==============================================================================
set -euo pipefail

# Ensure we run from repository root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${REPO_ROOT}"

echo "=================================================="
echo "🛠️  Building 2D Minecraft Suite (Native & WebAssembly)"
echo "=================================================="

# 1. Check Rust toolchain
echo "[1/4] Checking Rust environment..."
rustc --version
cargo --version

# Check if wasm32-unknown-unknown is installed
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "⚙️ Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# 2. Build Native Release Binaries
echo "[2/4] Building Native Binaries (voxel_app, voxel_web, voxel_server)..."
cargo build --release --bin voxel_app --bin voxel_web --bin voxel_server

# 3. Build WebAssembly 3D Client
echo "[3/4] Building WebAssembly 3D Client (voxel_web)..."
cargo build --release --target wasm32-unknown-unknown -p voxel_web

# Copy WASM output to assets/web
cp target/wasm32-unknown-unknown/release/voxel_web.wasm assets/web/voxel_web.wasm

# 4. Package Web Bundle
echo "[4/4] Assembling Assets & Packages..."
mkdir -p dist
cp target/release/voxel_app dist/
cp target/release/voxel_web dist/
cp target/release/voxel_server dist/
cp -r mods dist/
cp -r assets dist/

echo "=================================================="
echo "✅ Build Complete!"
echo "--------------------------------------------------"
echo "📦 Native Binaries in target/release/:"
echo "   - voxel_app     (2.5D Isometric Engine + Scheme Mod VM)"
echo "   - voxel_web     (3D First-Person & Isometric Native Client)"
echo "   - voxel_server  (Single-binary Gateway on 0.0.0.0:8083)"
echo "🌐 WebAssembly Client in assets/web/:"
echo "   - assets/web/voxel_web.wasm ($(du -h assets/web/voxel_web.wasm | cut -f1))"
echo "   - assets/web/index.html"
echo "=================================================="
