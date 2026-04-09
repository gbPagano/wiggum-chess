#!/usr/bin/env bash
# benchmark-version.sh — Automate benchmarking a chess engine version
#
# Usage:
#   scripts/benchmark-version.sh [FLAGS]
#
# Flags:
#   --version           <string>  Version label, e.g. v0.1 (required)
#   --engine            <path>    Path to the engine binary (required)
#   --prev-engine       <path>    Path to the previous version binary; required for SPRT, --run-ltc, --run-stc
#   --stockfish         <path>    Path to the Stockfish binary (optional)
#   --stockfish-levels  <list>    Comma-separated Elo levels for Stockfish, e.g. "1500,2000,max"
#                                 (default: 1500,2000,2500,max); "max" disables UCI_LimitStrength
#   --games             <N>       Number of games per fixed match (default: 100)
#   --output-dir        <path>    Directory for storing results (default: chess-engine/versions/<version>)
#   --positions-file    <path>    Path to balanced FEN positions file (one FEN per line)
#   --num-positions     <N>       Number of positions to sample from --positions-file (default: 10)
#   --sprt-max-games    <N>       Max games per SPRT run before stopping as inconclusive (default: unlimited)
#   --run-ltc                     Run LTC fixed-game block vs prev-engine (requires --prev-engine)
#   --run-stc                     Run STC fixed-game block vs prev-engine (requires --prev-engine)
#
# Blocks run:
#   SPRT startpos       — always runs if --prev-engine provided (STC time control)
#   SPRT balanced       — runs if --prev-engine AND --positions-file provided (STC time control)
#   LTC block           — optional (--run-ltc); engine vs prev-engine; startpos + balanced if --positions-file
#   STC block           — optional (--run-stc); engine vs prev-engine; startpos + balanced if --positions-file
#   Stockfish block     — optional (--stockfish); engine vs each level; balanced if --positions-file, else startpos
#
# Stockfish setoption limitation:
#   chess-runner match does not support passing setoption commands to engines at startup.
#   This script works around the limitation by creating temporary wrapper scripts per
#   skill level that intercept the UCI handshake and inject setoption commands before
#   forwarding to Stockfish. The wrappers are cleaned up on exit.
#
# Output files (written to --output-dir):
#   sprt_startpos.csv   — SPRT results from startpos
#   sprt_balanced.csv   — SPRT results from balanced positions
#   ltc.csv             — LTC fixed-game results vs prev-engine
#   stc.csv             — STC fixed-game results vs prev-engine
#   stockfish.csv       — Stockfish match results
#
# Example:
#   scripts/benchmark-version.sh \
#     --version v0.2 \
#     --engine ./target/release/chess-engine \
#     --prev-engine chess-engine/versions/v0.1/wiggum-engine \
#     --stockfish ./engines/stockfish \
#     --stockfish-levels "1500,2000,max" \
#     --games 20 \
#     --positions-file data/balanced-positions.fen \
#     --num-positions 5 \
#     --sprt-max-games 100 \
#     --run-ltc --run-stc

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
VERSION=""
ENGINE=""
PREV_ENGINE=""
STOCKFISH=""
STOCKFISH_LEVELS="1500,2000,2500,max"
GAMES=100
OUTPUT_DIR=""
CHESS_RUNNER="${CHESS_RUNNER:-chess-runner}"
LTC_TIME_MS=60000
LTC_INC_MS=1000
STC_TIME_MS=10000
STC_INC_MS=100
POSITIONS_FILE=""
NUM_POSITIONS=10
SPRT_MAX_GAMES=""
RUN_LTC=0
RUN_STC=0

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)          VERSION="$2";          shift 2 ;;
        --engine)           ENGINE="$2";           shift 2 ;;
        --prev-engine)      PREV_ENGINE="$2";      shift 2 ;;
        --stockfish)        STOCKFISH="$2";        shift 2 ;;
        --stockfish-levels) STOCKFISH_LEVELS="$2"; shift 2 ;;
        --games)            GAMES="$2";            shift 2 ;;
        --output-dir)       OUTPUT_DIR="$2";       shift 2 ;;
        --positions-file)   POSITIONS_FILE="$2";   shift 2 ;;
        --num-positions)    NUM_POSITIONS="$2";    shift 2 ;;
        --sprt-max-games)   SPRT_MAX_GAMES="$2";   shift 2 ;;
        --run-ltc)          RUN_LTC=1;             shift ;;
        --run-stc)          RUN_STC=1;             shift ;;
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
if [[ ! -x "$ENGINE" ]]; then
    echo "Error: engine binary not found or not executable: $ENGINE" >&2; exit 1
fi
if [[ -n "$STOCKFISH" && ! -x "$STOCKFISH" ]]; then
    echo "Error: stockfish binary not found or not executable: $STOCKFISH" >&2; exit 1
fi
if [[ -n "$PREV_ENGINE" && ! -x "$PREV_ENGINE" ]]; then
    echo "Error: prev-engine binary not found or not executable: $PREV_ENGINE" >&2; exit 1
fi
if [[ -n "$POSITIONS_FILE" && ! -f "$POSITIONS_FILE" ]]; then
    echo "Error: positions file not found: $POSITIONS_FILE" >&2; exit 1
fi
if [[ "$RUN_LTC" -eq 1 && -z "$PREV_ENGINE" && -z "$STOCKFISH" ]]; then
    echo "Error: --run-ltc requires --prev-engine or --stockfish" >&2; exit 1
fi
if [[ "$RUN_STC" -eq 1 && -z "$PREV_ENGINE" && -z "$STOCKFISH" ]]; then
    echo "Error: --run-stc requires --prev-engine or --stockfish" >&2; exit 1
fi

if [[ -z "$OUTPUT_DIR" ]]; then
    OUTPUT_DIR="chess-engine/versions/${VERSION}"
fi
mkdir -p "$OUTPUT_DIR"

# ── Output files ──────────────────────────────────────────────────────────────
SPRT_STARTPOS_CSV="${OUTPUT_DIR}/sprt_startpos.csv"
SPRT_BALANCED_CSV="${OUTPUT_DIR}/sprt_balanced.csv"
LTC_CSV="${OUTPUT_DIR}/ltc.csv"
STC_CSV="${OUTPUT_DIR}/stc.csv"
STOCKFISH_CSV="${OUTPUT_DIR}/stockfish.csv"

# ── tag_last_match_row <csv_path> <label> ────────────────────────────────────
#
# chess-runner records the opponent's UCI name, which is identical for all
# Stockfish skill levels. Tag the newest row so results can keep the levels separate.
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

# ── Header ────────────────────────────────────────────────────────────────────
echo "========================================="
echo "Benchmark: ${VERSION}"
echo "Engine:    ${ENGINE}"
if [[ -n "$PREV_ENGINE" ]]; then
    echo "Prev:      ${PREV_ENGINE}"
fi
if [[ -n "$STOCKFISH" ]]; then
    echo "Stockfish: ${STOCKFISH} (levels: ${STOCKFISH_LEVELS})"
fi
echo "Games:     ${GAMES} per fixed match"
echo "Output:    ${OUTPUT_DIR}"
if [[ -n "$POSITIONS_FILE" ]]; then
    echo "Positions: ${POSITIONS_FILE} (${NUM_POSITIONS} sampled)"
fi
echo "========================================="

# ── SPRT blocks (require --prev-engine) ──────────────────────────────────────
if [[ -n "$PREV_ENGINE" ]]; then
    SPRT_MAX_GAMES_ARGS=()
    if [[ -n "$SPRT_MAX_GAMES" ]]; then
        SPRT_MAX_GAMES_ARGS=(--max-games "$SPRT_MAX_GAMES")
    fi

    # SPRT from startpos
    echo ""
    echo "========================================="
    echo "SPRT block: startpos (STC)"
    echo "========================================="
    "$CHESS_RUNNER" sprt \
        --engine1 "$ENGINE" \
        --engine2 "$PREV_ENGINE" \
        --time "$STC_TIME_MS" \
        --inc "$STC_INC_MS" \
        --output "$SPRT_STARTPOS_CSV" \
        "${SPRT_MAX_GAMES_ARGS[@]+"${SPRT_MAX_GAMES_ARGS[@]}"}"
    echo "SPRT startpos complete. Results in: ${SPRT_STARTPOS_CSV}"

    # SPRT from balanced positions
    if [[ -n "$POSITIONS_FILE" ]]; then
        echo ""
        echo "========================================="
        echo "SPRT block: balanced positions (STC)"
        echo "========================================="
        "$CHESS_RUNNER" sprt \
            --engine1 "$ENGINE" \
            --engine2 "$PREV_ENGINE" \
            --time "$STC_TIME_MS" \
            --inc "$STC_INC_MS" \
            --positions-file "$POSITIONS_FILE" \
            --num-positions "$NUM_POSITIONS" \
            --output "$SPRT_BALANCED_CSV" \
            "${SPRT_MAX_GAMES_ARGS[@]+"${SPRT_MAX_GAMES_ARGS[@]}"}"
        echo "SPRT balanced complete. Results in: ${SPRT_BALANCED_CSV}"
    fi
fi

# ── LTC block vs prev-engine (optional) ──────────────────────────────────────
if [[ "$RUN_LTC" -eq 1 ]]; then
    echo ""
    echo "========================================="
    echo "LTC block vs prev-engine"
    echo "========================================="

    echo "--- Match vs prev-engine LTC startpos (${GAMES} games) ---"
    "$CHESS_RUNNER" match \
        --engine1 "$ENGINE" \
        --engine2 "$PREV_ENGINE" \
        --time "$LTC_TIME_MS" \
        --inc "$LTC_INC_MS" \
        --games "$GAMES" \
        --output "$LTC_CSV"

    if [[ -n "$POSITIONS_FILE" ]]; then
        echo "--- Match vs prev-engine LTC balanced (${GAMES} games, ${NUM_POSITIONS} positions) ---"
        "$CHESS_RUNNER" match \
            --engine1 "$ENGINE" \
            --engine2 "$PREV_ENGINE" \
            --time "$LTC_TIME_MS" \
            --inc "$LTC_INC_MS" \
            --games "$GAMES" \
            --positions-file "$POSITIONS_FILE" \
            --num-positions "$NUM_POSITIONS" \
            --output "$LTC_CSV"
    fi

    echo "LTC complete. Results in: ${LTC_CSV}"
fi

# ── STC block vs prev-engine (optional) ──────────────────────────────────────
if [[ "$RUN_STC" -eq 1 ]]; then
    echo ""
    echo "========================================="
    echo "STC block vs prev-engine"
    echo "========================================="

    echo "--- Match vs prev-engine STC startpos (${GAMES} games) ---"
    "$CHESS_RUNNER" match \
        --engine1 "$ENGINE" \
        --engine2 "$PREV_ENGINE" \
        --time "$STC_TIME_MS" \
        --inc "$STC_INC_MS" \
        --games "$GAMES" \
        --output "$STC_CSV"

    if [[ -n "$POSITIONS_FILE" ]]; then
        echo "--- Match vs prev-engine STC balanced (${GAMES} games, ${NUM_POSITIONS} positions) ---"
        "$CHESS_RUNNER" match \
            --engine1 "$ENGINE" \
            --engine2 "$PREV_ENGINE" \
            --time "$STC_TIME_MS" \
            --inc "$STC_INC_MS" \
            --games "$GAMES" \
            --positions-file "$POSITIONS_FILE" \
            --num-positions "$NUM_POSITIONS" \
            --output "$STC_CSV"
    fi

    echo "STC complete. Results in: ${STC_CSV}"
fi

# ── Stockfish block (optional) ────────────────────────────────────────────────
if [[ -n "$STOCKFISH" ]]; then
    TMPDIR_WRAPPERS="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR_WRAPPERS"' EXIT

    # create_stockfish_wrapper <label> <uci_limit_strength> [<elo>]
    #
    # The wrapper intercepts the first 'uci' command from chess-runner, injects
    # setoption commands into Stockfish's stdin, and proxies all remaining I/O.
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

"\$SF" < "\$FIFO_IN" &
SF_PID=\$!

exec 3>"\$FIFO_IN"

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

    echo ""
    echo "========================================="
    echo "Stockfish block (levels: ${STOCKFISH_LEVELS})"
    echo "========================================="

    IFS=',' read -ra SF_LEVELS <<< "$STOCKFISH_LEVELS"
    for level in "${SF_LEVELS[@]}"; do
        level="${level// /}"  # trim spaces
        if [[ "$level" == "max" ]]; then
            wrapper="$(create_stockfish_wrapper "max" "false")"
        else
            wrapper="$(create_stockfish_wrapper "$level" "true" "$level")"
        fi

        if [[ "$RUN_LTC" -eq 1 ]]; then
            echo "--- Match vs Stockfish ${level} LTC startpos (${GAMES} games) ---"
            "$CHESS_RUNNER" match \
                --engine1 "$ENGINE" \
                --engine2 "$wrapper" \
                --time "$LTC_TIME_MS" \
                --inc "$LTC_INC_MS" \
                --games "$GAMES" \
                --output "$STOCKFISH_CSV"
            tag_last_match_row "$STOCKFISH_CSV" "${level}-LTC"

            if [[ -n "$POSITIONS_FILE" ]]; then
                echo "--- Match vs Stockfish ${level} LTC balanced (${GAMES} games, ${NUM_POSITIONS} positions) ---"
                "$CHESS_RUNNER" match \
                    --engine1 "$ENGINE" \
                    --engine2 "$wrapper" \
                    --time "$LTC_TIME_MS" \
                    --inc "$LTC_INC_MS" \
                    --games "$GAMES" \
                    --positions-file "$POSITIONS_FILE" \
                    --num-positions "$NUM_POSITIONS" \
                    --output "$STOCKFISH_CSV"
                tag_last_match_row "$STOCKFISH_CSV" "${level}-LTC-balanced"
            fi
        fi

        if [[ "$RUN_STC" -eq 1 ]]; then
            echo "--- Match vs Stockfish ${level} STC startpos (${GAMES} games) ---"
            "$CHESS_RUNNER" match \
                --engine1 "$ENGINE" \
                --engine2 "$wrapper" \
                --time "$STC_TIME_MS" \
                --inc "$STC_INC_MS" \
                --games "$GAMES" \
                --output "$STOCKFISH_CSV"
            tag_last_match_row "$STOCKFISH_CSV" "${level}-STC"

            if [[ -n "$POSITIONS_FILE" ]]; then
                echo "--- Match vs Stockfish ${level} STC balanced (${GAMES} games, ${NUM_POSITIONS} positions) ---"
                "$CHESS_RUNNER" match \
                    --engine1 "$ENGINE" \
                    --engine2 "$wrapper" \
                    --time "$STC_TIME_MS" \
                    --inc "$STC_INC_MS" \
                    --games "$GAMES" \
                    --positions-file "$POSITIONS_FILE" \
                    --num-positions "$NUM_POSITIONS" \
                    --output "$STOCKFISH_CSV"
                tag_last_match_row "$STOCKFISH_CSV" "${level}-STC-balanced"
            fi
        fi
    done

    echo "Stockfish complete. Results in: ${STOCKFISH_CSV}"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "========================================="
echo "All benchmarks complete for ${VERSION}"
if [[ -n "$PREV_ENGINE" ]]; then
    echo "SPRT startpos:  ${SPRT_STARTPOS_CSV}"
    if [[ -n "$POSITIONS_FILE" ]]; then
        echo "SPRT balanced:  ${SPRT_BALANCED_CSV}"
    fi
fi
if [[ "$RUN_LTC" -eq 1 ]]; then
    echo "LTC results:    ${LTC_CSV}"
fi
if [[ "$RUN_STC" -eq 1 ]]; then
    echo "STC results:    ${STC_CSV}"
fi
if [[ -n "$STOCKFISH" ]]; then
    echo "Stockfish:      ${STOCKFISH_CSV}"
fi
echo "========================================="
