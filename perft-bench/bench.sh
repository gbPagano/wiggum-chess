#!/usr/bin/env bash
set -euo pipefail

# Presets: "depth|fen"
declare -A PRESETS=(
  [starting]="6|rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
  [kiwipete]="5|r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -"
  [promotions]="6|n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - -"
  [captures]="5|rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
)

DEPTH=6
FEN="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
WITH_SIMPLE=0
WITH_PYTHON=0
POSITION=""

usage() {
  echo "Usage: $0 [--simple] [--python] [--position <preset>] [depth] [fen]"
  echo ""
  echo "Presets: ${!PRESETS[*]}"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --simple) WITH_SIMPLE=1; shift ;;
    --python) WITH_PYTHON=1; shift ;;
    --position) POSITION=$2; shift 2 ;;
    --help|-h) usage ;;
    *)
      if [[ -z "${DEPTH_SET:-}" ]]; then
        DEPTH=$1; DEPTH_SET=1
      elif [[ -z "${FEN_SET:-}" ]]; then
        FEN=$1; FEN_SET=1
      else
        usage
      fi
      shift
      ;;
  esac
done

if [[ -n "$POSITION" ]]; then
  if [[ -z "${PRESETS[$POSITION]:-}" ]]; then
    echo "Unknown position '$POSITION'. Available: ${!PRESETS[*]}" >&2
    exit 1
  fi
  DEPTH="${PRESETS[$POSITION]%%|*}"
  FEN="${PRESETS[$POSITION]#*|}"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

RUSTFLAGS="-C target-cpu=native" cargo build --release -p perft-bench --manifest-path "$ROOT_DIR/Cargo.toml" --no-default-features 2>&1

BIN="$ROOT_DIR/target/release/perft-bench"
PY="$SCRIPT_DIR/perft_python_chess.py"
STOCKFISH_BIN="${STOCKFISH_BIN:-stockfish}"

if ! command -v "$STOCKFISH_BIN" >/dev/null 2>&1; then
  echo "Stockfish binary '$STOCKFISH_BIN' not found. Set STOCKFISH_BIN to override." >&2
  exit 1
fi

echo "Position : ${POSITION:-custom}"
echo "FEN      : $FEN"
echo "Depth    : $DEPTH"
echo ""

CMDS=(
  "$BIN --engine chesslib --depth $DEPTH --fen '$FEN'"
  "$BIN --engine chess    --depth $DEPTH --fen '$FEN'"
  "$BIN --engine shakmaty --depth $DEPTH --fen '$FEN'"
  "bash -lc 'printf \"uci\\nisready\\nposition fen %s\\ngo perft %s\\nquit\\n\" \"$FEN\" \"$DEPTH\" | \"$STOCKFISH_BIN\" > /dev/null'"
)

[[ $WITH_SIMPLE -eq 1 ]] && CMDS+=("$BIN --engine chesslib-simple --depth $DEPTH --fen '$FEN'")
[[ $WITH_PYTHON -eq 1 ]] && CMDS+=("$PY --depth $DEPTH --fen '$FEN'")

hyperfine --warmup 2 "${CMDS[@]}"
