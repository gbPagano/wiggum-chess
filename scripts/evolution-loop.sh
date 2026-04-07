#!/usr/bin/env bash
# evolution-loop.sh
#
# Start a Wiggum engine evolution session.
#
# Usage:
#   ./scripts/evolution-loop.sh --baseline-version <version> [--output-dir <path>] [--max-iterations <count>]
#   ./scripts/evolution-loop.sh --help
#
# Options:
#   --baseline-version  Required. Baseline engine version to evolve from.
#   --output-dir        Session artifact root. Defaults to tasks/evolution-runs.
#   --max-iterations    Maximum iterations to allow. Defaults to 10.
#   --help              Print this help text.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BASELINE_VERSION=""
OUTPUT_DIR="$REPO_ROOT/tasks/evolution-runs"
MAX_ITERATIONS=10
SESSION_SUMMARY_FILENAME="summary.md"

usage() {
  sed -n '1,13p' "$0"
}

require_value() {
  local flag="$1"
  local value="${2-}"

  if [[ -z "$value" || "$value" == --* ]]; then
    echo "Error: $flag requires a value." >&2
    exit 1
  fi
}

session_id_now() {
  date -u +"%Y%m%dT%H%M%SZ"
}

write_session_summary_placeholder() {
  local summary_path="$1"

  cat <<EOF > "$summary_path"
# Wiggum Evolution Session Summary

- Session id:
- Baseline version:
- Max iterations:
- Session directory:
- Status: initialized
EOF
}

write_session_metadata() {
  local metadata_path="$1"
  local session_id="$2"
  local session_dir="$3"

  cat <<EOF > "$metadata_path"
baseline_version=$BASELINE_VERSION
max_iterations=$MAX_ITERATIONS
session_id=$session_id
session_dir=$session_dir
summary_file=$SESSION_SUMMARY_FILENAME
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline-version)
      require_value "$1" "${2-}"
      BASELINE_VERSION="$2"
      shift 2
      ;;
    --output-dir)
      require_value "$1" "${2-}"
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --max-iterations)
      require_value "$1" "${2-}"
      MAX_ITERATIONS="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Error: Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$BASELINE_VERSION" ]]; then
  echo "Error: --baseline-version is required." >&2
  usage >&2
  exit 1
fi

if ! [[ "$MAX_ITERATIONS" =~ ^[1-9][0-9]*$ ]]; then
  echo "Error: --max-iterations must be a positive integer." >&2
  exit 1
fi

SESSION_ID="$(session_id_now)"
SESSION_DIR="$OUTPUT_DIR/$SESSION_ID"
SESSION_SUMMARY_PATH="$SESSION_DIR/$SESSION_SUMMARY_FILENAME"
SESSION_METADATA_PATH="$SESSION_DIR/session.env"

mkdir -p "$SESSION_DIR"
write_session_summary_placeholder "$SESSION_SUMMARY_PATH"
write_session_metadata "$SESSION_METADATA_PATH" "$SESSION_ID" "$SESSION_DIR"

echo "Starting Wiggum evolution loop"
echo "Baseline version: $BASELINE_VERSION"
echo "Output directory: $OUTPUT_DIR"
echo "Max iterations: $MAX_ITERATIONS"
echo "Session ID: $SESSION_ID"
echo "Session directory: $SESSION_DIR"
echo "Session summary: $SESSION_SUMMARY_PATH"
echo "Session metadata: $SESSION_METADATA_PATH"
echo
echo "Session artifact root is ready. Future stories will create iteration artifacts and run iterations."
