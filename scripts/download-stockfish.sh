#!/usr/bin/env bash
set -euo pipefail

ENGINES_DIR="$(cd "$(dirname "$0")/.." && pwd)/engines"
STOCKFISH_PATH="$ENGINES_DIR/stockfish"

if [ -f "$STOCKFISH_PATH" ]; then
    echo "Stockfish already exists at $STOCKFISH_PATH — skipping download."
    exit 0
fi

mkdir -p "$ENGINES_DIR"

# Download Stockfish 17 for Linux x86_64
STOCKFISH_URL="https://github.com/official-stockfish/Stockfish/releases/download/sf_17/stockfish-ubuntu-x86-64.tar"
TMP_DIR=$(mktemp -d)
TMP_TAR="$TMP_DIR/stockfish.tar"

echo "Downloading Stockfish from $STOCKFISH_URL ..."
curl -fsSL -o "$TMP_TAR" "$STOCKFISH_URL"

echo "Extracting ..."
tar -xf "$TMP_TAR" -C "$TMP_DIR"

# Find the stockfish binary in extracted contents
SF_BIN=$(find "$TMP_DIR" -type f -name "stockfish*" ! -name "*.tar" | head -1)
if [ -z "$SF_BIN" ]; then
    echo "ERROR: Could not find stockfish binary in archive." >&2
    exit 1
fi

cp "$SF_BIN" "$STOCKFISH_PATH"
chmod +x "$STOCKFISH_PATH"
rm -rf "$TMP_DIR"

echo "Stockfish installed at $STOCKFISH_PATH"
echo "Verifying ..."
echo "uci" | timeout 5 "$STOCKFISH_PATH" | grep -q "uciok" && echo "OK: uciok received."
