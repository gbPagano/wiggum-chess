#!/usr/bin/env bash
# benchmark-version.sh — Automate benchmarking a chess engine version against Stockfish
#
# Usage:
#   scripts/benchmark-version.sh [FLAGS]
#
# Flags:
#   --version      <string>  Version label, e.g. v0.1 (required)
#   --engine       <path>    Path to the engine binary being benchmarked (required)
#   --prev-engine  <path>    Path to the previous version binary (optional); if provided,
#                            runs a SPRT match to verify improvement
#   --stockfish    <path>    Path to the Stockfish binary (required)
#   --games        <N>       Number of games per fixed match (default: 100)
#   --output-dir   <path>    Directory for storing results (default: chess-engine/versions/<version>)
#
# Stockfish setoption limitation:
#   chess-runner match does not support passing setoption commands to engines at startup.
#   This script works around the limitation by creating temporary wrapper scripts per
#   skill level that intercept the UCI handshake and inject setoption commands before
#   forwarding to Stockfish. The wrappers are cleaned up on exit.
#
# Output:
#   All match results are appended to <output-dir>/results.csv via chess-runner --output.
#   A summary is printed to stdout when all matches complete.
#
# Example:
#   scripts/benchmark-version.sh \
#     --version v0.1 \
#     --engine ./target/release/chess-engine \
#     --stockfish ./engines/stockfish \
#     --games 50

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
VERSION=""
ENGINE=""
PREV_ENGINE=""
STOCKFISH=""
GAMES=100
OUTPUT_DIR=""
CHESS_RUNNER="${CHESS_RUNNER:-chess-runner}"
TIME_MS=10000
INC_MS=100

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)    VERSION="$2";    shift 2 ;;
        --engine)     ENGINE="$2";     shift 2 ;;
        --prev-engine) PREV_ENGINE="$2"; shift 2 ;;
        --stockfish)  STOCKFISH="$2";  shift 2 ;;
        --games)      GAMES="$2";      shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,/^set -/p' "$0" | grep '^#' | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "Unknown flag: $1" >&2
            exit 1
            ;;
    esac
done

# ── Validation ────────────────────────────────────────────────────────────────
if [[ -z "$VERSION" ]]; then
    echo "Error: --version is required" >&2; exit 1
fi
if [[ -z "$ENGINE" ]]; then
    echo "Error: --engine is required" >&2; exit 1
fi
if [[ -z "$STOCKFISH" ]]; then
    echo "Error: --stockfish is required" >&2; exit 1
fi

if [[ ! -x "$ENGINE" ]]; then
    echo "Error: engine binary not found or not executable: $ENGINE" >&2; exit 1
fi
if [[ ! -x "$STOCKFISH" ]]; then
    echo "Error: stockfish binary not found or not executable: $STOCKFISH" >&2; exit 1
fi
if [[ -n "$PREV_ENGINE" && ! -x "$PREV_ENGINE" ]]; then
    echo "Error: prev-engine binary not found or not executable: $PREV_ENGINE" >&2; exit 1
fi

if [[ -z "$OUTPUT_DIR" ]]; then
    OUTPUT_DIR="chess-engine/versions/${VERSION}"
fi

mkdir -p "$OUTPUT_DIR"

RESULTS_CSV="${OUTPUT_DIR}/results.csv"
SPRT_CSV="${OUTPUT_DIR}/sprt_results.csv"

# tag_last_match_row <csv_path> <label>
#
# chess-runner records the opponent's UCI name, which is identical for all
# Stockfish skill levels. Tag the newest row so version-report can keep the
# four benchmark levels separate.
tag_last_match_row() {
    local csv_path="$1"
    local label="$2"

    python3 - "$csv_path" "$label" <<'PY'
import csv
import sys

path, label = sys.argv[1], sys.argv[2]

with open(path, newline="") as f:
    rows = list(csv.reader(f))

if len(rows) < 2:
    raise SystemExit(f"expected at least one result row in {path}")

header = rows[0]
engine2_idx = header.index("engine2_name")
last = rows[-1]
suffix = f" ({label})"
if not last[engine2_idx].endswith(suffix):
    last[engine2_idx] = f"{last[engine2_idx]}{suffix}"

with open(path, "w", newline="") as f:
    csv.writer(f).writerows(rows)
PY
}

# ── Temporary wrapper scripts ─────────────────────────────────────────────────
TMPDIR_WRAPPERS="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_WRAPPERS"' EXIT

# create_stockfish_wrapper <label> <uci_limit_strength> [<elo>]
#
# The wrapper intercepts the first 'uci' command from chess-runner, injects
# setoption commands into Stockfish's stdin, and proxies all remaining I/O.
# This is necessary because chess-runner match does not expose a --setoption
# interface for configuring the engine subprocess.
create_stockfish_wrapper() {
    local label="$1"
    local limit_strength="$2"
    local elo="${3:-}"
    local wrapper="${TMPDIR_WRAPPERS}/stockfish-${label}.sh"

    cat > "$wrapper" <<WRAPPER_EOF
#!/usr/bin/env bash
# Thin Stockfish wrapper: injects setoption for skill level '${label}'
set -euo pipefail

SF="${STOCKFISH}"
FIFO_IN="\$(mktemp -u)"
mkfifo "\$FIFO_IN"
trap 'rm -f "\$FIFO_IN"' EXIT

# Start Stockfish with the FIFO as stdin
"\$SF" < "\$FIFO_IN" &
SF_PID=\$!

# Open the write end of the FIFO
exec 3>"\$FIFO_IN"

# Relay stdin → Stockfish (via FIFO), injecting setoption after 'uci'
UCI_SENT=0
while IFS= read -r line; do
    echo "\$line" >&3
    if [[ "\$line" == "uci" && \$UCI_SENT -eq 0 ]]; then
        UCI_SENT=1
        echo "setoption name UCI_LimitStrength value ${limit_strength}" >&3
WRAPPER_EOF

    if [[ -n "$elo" ]]; then
        cat >> "$wrapper" <<WRAPPER_EOF
        echo "setoption name UCI_Elo value ${elo}" >&3
WRAPPER_EOF
    fi

    cat >> "$wrapper" <<WRAPPER_EOF
    fi
done

wait "\$SF_PID"
WRAPPER_EOF

    chmod +x "$wrapper"
    echo "$wrapper"
}

# ── Create wrappers for each difficulty level ─────────────────────────────────
WRAPPER_1500="$(create_stockfish_wrapper "1500" "true" "1500")"
WRAPPER_2000="$(create_stockfish_wrapper "2000" "true" "2000")"
WRAPPER_2500="$(create_stockfish_wrapper "2500" "true" "2500")"
WRAPPER_MAX="$(create_stockfish_wrapper "max" "false")"

echo "========================================="
echo "Benchmark: ${VERSION}"
echo "Engine:    ${ENGINE}"
echo "Stockfish: ${STOCKFISH}"
echo "Games:     ${GAMES} per match"
echo "Output:    ${OUTPUT_DIR}"
echo "========================================="

# ── Optional SPRT vs previous engine ─────────────────────────────────────────
if [[ -n "$PREV_ENGINE" ]]; then
    echo ""
    echo "--- SPRT vs previous engine ---"
    "$CHESS_RUNNER" sprt \
        --engine1 "$ENGINE" \
        --engine2 "$PREV_ENGINE" \
        --time "$TIME_MS" \
        --inc "$INC_MS" \
        --output "$SPRT_CSV"
    echo "SPRT complete. Results in: ${SPRT_CSV}"
fi

# ── Run fixed matches vs Stockfish at each level ──────────────────────────────
run_match() {
    local label="$1"
    local sf_wrapper="$2"

    echo ""
    echo "--- Match vs Stockfish ${label} (${GAMES} games) ---"
    "$CHESS_RUNNER" match \
        --engine1 "$ENGINE" \
        --engine2 "$sf_wrapper" \
        --time "$TIME_MS" \
        --inc "$INC_MS" \
        --games "$GAMES" \
        --output "$RESULTS_CSV"
    tag_last_match_row "$RESULTS_CSV" "$label"
}

run_match "1500" "$WRAPPER_1500"
run_match "2000" "$WRAPPER_2000"
run_match "2500" "$WRAPPER_2500"
run_match "max"  "$WRAPPER_MAX"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "========================================="
echo "All matches complete for ${VERSION}"
echo "Match results: ${RESULTS_CSV}"
if [[ -n "$PREV_ENGINE" ]]; then
    echo "SPRT results:  ${SPRT_CSV}"
fi
echo ""

if [[ -f "$RESULTS_CSV" ]]; then
    echo "Results summary:"
    # Print header + all rows for this engine (simple grep-based filter)
    head -1 "$RESULTS_CSV"
    grep -i "$ENGINE" "$RESULTS_CSV" || true
fi

echo "========================================="
