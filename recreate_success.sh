#!/bin/bash
# Version: 3.0.0 · updated 26-11-18-02-45
# Install sensors.gorilla on Linux or macOS without on-the-fly compilation.
# Prefers the compiled native binary (from release download or `cargo build --release --offline --locked`),
# and if installing the Python fallback, precompiles bytecode (.pyc) ahead of time.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$HOME/.local/bin"
DEST="$INSTALL_DIR/sensors.gorilla"
mkdir -p "$INSTALL_DIR"

OS="$(uname -s)"
ARCH="$(uname -m)"

# 1. Prebuilt native compiled binary next to this script
PREBUILT=""
if [ "$OS" = "Linux" ] && [ -f "$HERE/sensors-gorilla-linux-x86_64" ]; then
    PREBUILT="$HERE/sensors-gorilla-linux-x86_64"
elif [ "$OS" = "Darwin" ] && [ -f "$HERE/sensors-gorilla-macos-universal" ]; then
    PREBUILT="$HERE/sensors-gorilla-macos-universal"
elif [ "$OS" = "Darwin" ] && [ -f "$HERE/sensors-gorilla-macos-x86_64" ]; then
    PREBUILT="$HERE/sensors-gorilla-macos-x86_64"
elif [ -x "$HERE/target/release/sensors-gorilla" ]; then
    PREBUILT="$HERE/target/release/sensors-gorilla"
fi

if [ -n "$PREBUILT" ]; then
    cp "$PREBUILT" "$DEST"
    chmod +x "$DEST"
    ln -sf "$DEST" "$INSTALL_DIR/sensors-gorilla"
    echo "Installed compiled native binary -> $DEST (zero on-the-fly compilation)"
elif command -v cargo >/dev/null 2>&1 && [ -f "$HERE/Cargo.toml" ]; then
    (cd "$HERE" && cargo build --release --offline --locked)
    cp "$HERE/target/release/sensors-gorilla" "$DEST"
    chmod +x "$DEST"
    ln -sf "$DEST" "$INSTALL_DIR/sensors-gorilla"
    echo "Built and installed compiled native binary -> $DEST"
elif [ -f "$HERE/sensors.gorilla" ]; then
    # Precompile Python bytecode ahead of time so Python never compiles the module on every run
    LIB_DIR="$HOME/.local/lib/sensors-gorilla"
    mkdir -p "$LIB_DIR"
    cp "$HERE/sensors.gorilla" "$LIB_DIR/sensors_gorilla_app.py"
    python3 -m py_compile "$LIB_DIR/sensors_gorilla_app.py"
    PYC="$(python3 -c "import importlib.util; print(importlib.util.cache_from_source('$LIB_DIR/sensors_gorilla_app.py'))")"
    cat > "$DEST" <<EOF
#!/bin/sh
exec python3 "$PYC" "\$@"
EOF
    chmod +x "$DEST"
    ln -sf "$DEST" "$INSTALL_DIR/sensors-gorilla"
    echo "Installed precompiled bytecode launcher -> $DEST ($PYC)"
else
    echo "error: no compiled binary or source found in $HERE" >&2
    exit 1
fi

"$DEST" --version
echo "Usage: sensors.gorilla [--once] [-n 5] [--json]"
