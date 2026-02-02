#!/bin/bash
#
# Build Keva for macOS
#
# Usage:
#   ./build.sh              # Release build
#   ./build.sh --debug      # Debug build
#   ./build.sh --skip-frontend  # Skip frontend build
#
# Requirements:
#   - Xcode (with command line tools)
#   - Rust toolchain with aarch64-apple-darwin target
#   - Node.js and pnpm
#

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
FRONTEND_DIR="$REPO_ROOT/frontend"
XCODE_PROJECT="$REPO_ROOT/keva_macos/Keva/Keva.xcodeproj"
LIB_DIR="$REPO_ROOT/keva_macos/Keva/lib"
INCLUDE_DIR="$REPO_ROOT/keva_macos/Keva/include"

# Parse arguments
DEBUG=false
SKIP_FRONTEND=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --debug|-d)
            DEBUG=true
            shift
            ;;
        --skip-frontend|-s)
            SKIP_FRONTEND=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

if [ "$DEBUG" = true ]; then
    CONFIGURATION="Debug"
    CARGO_PROFILE=""
else
    CONFIGURATION="Release"
    CARGO_PROFILE="--release"
fi

echo "=== Keva macOS Build ==="
echo "Repository: $REPO_ROOT"
echo "Mode: $CONFIGURATION"
echo ""

# Step 1: Build frontend
if [ "$SKIP_FRONTEND" = false ]; then
    echo "=== Building Frontend ==="

    cd "$FRONTEND_DIR"

    echo "Running pnpm install..."
    pnpm install --frozen-lockfile

    echo "Running pnpm build..."
    pnpm build

    echo "Frontend build complete."
    echo ""
fi

# Step 2: Build Rust FFI static library
echo "=== Building Rust FFI Library ==="

cd "$REPO_ROOT"

cargo build -q $CARGO_PROFILE -p keva_ffi --target aarch64-apple-darwin

# Copy static library and header to locations Xcode expects
mkdir -p "$LIB_DIR" "$INCLUDE_DIR"

if [ "$DEBUG" = true ]; then
    RUST_TARGET_DIR="$REPO_ROOT/target/aarch64-apple-darwin/debug"
else
    RUST_TARGET_DIR="$REPO_ROOT/target/aarch64-apple-darwin/release"
fi

cp "$RUST_TARGET_DIR/libkeva_ffi.a" "$LIB_DIR/"
cp "$REPO_ROOT/ffi/include/keva_ffi.h" "$INCLUDE_DIR/"

echo "FFI library built and copied."
echo ""

# Step 3: Build Swift application
echo "=== Building Swift Application ==="

cd "$REPO_ROOT"

xcodebuild -project "$XCODE_PROJECT" \
    -scheme Keva \
    -configuration "$CONFIGURATION" \
    -derivedDataPath "$REPO_ROOT/target/macos" \
    build

APP_PATH="$REPO_ROOT/target/macos/Build/Products/$CONFIGURATION/Keva.app"

if [ -d "$APP_PATH" ]; then
    SIZE=$(du -sh "$APP_PATH" | cut -f1)
    echo "Build complete: $APP_PATH ($SIZE)"
else
    echo "Warning: App bundle not found at expected path"
fi

echo ""
echo "=== Build Successful ==="
