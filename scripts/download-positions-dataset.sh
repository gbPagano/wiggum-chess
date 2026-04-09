#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
DATA_DIR="$REPO_ROOT/data"
OUTPUT_FILE="$DATA_DIR/lichess-2013-01.pgn"

if [ -f "$OUTPUT_FILE" ]; then
    echo "already exists — skipping"
    exit 0
fi

mkdir -p "$DATA_DIR"

DATASET_URL="https://database.lichess.org/standard/lichess_db_standard_rated_2013-01.pgn.zst"
TMP_FILE="$DATA_DIR/lichess_db_standard_rated_2013-01.pgn.zst"

echo "Downloading Lichess dataset from $DATASET_URL ..."
curl -fsSL -o "$TMP_FILE" "$DATASET_URL"

echo "Decompressing ..."
if command -v unzstd &>/dev/null; then
    unzstd -o "$OUTPUT_FILE" "$TMP_FILE"
elif command -v zstd &>/dev/null; then
    zstd -d "$TMP_FILE" -o "$OUTPUT_FILE"
else
    echo "ERROR: neither zstd nor unzstd found. Please install zstd." >&2
    rm -f "$TMP_FILE"
    exit 1
fi

rm -f "$TMP_FILE"
echo "Dataset extracted to $OUTPUT_FILE"
