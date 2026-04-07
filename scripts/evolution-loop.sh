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
WORKER_GUIDANCE="$REPO_ROOT/.claude/evolution/CLAUDE.md"

BASELINE_VERSION=""
OUTPUT_DIR="$REPO_ROOT/tasks/evolution-runs"
MAX_ITERATIONS=10
SESSION_SUMMARY_FILENAME="summary.md"
ITERATIONS_DIRNAME="iterations"
INITIAL_ITERATION_NUMBER=1
ITERATION_STATE_FILENAME="iteration.json"
HYPOTHESIS_FILENAME="hypothesis.md"
IMPLEMENTATION_FILENAME="implementation.md"
BENCHMARK_FILENAME="benchmark.md"
DECISION_FILENAME="decision.md"

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

json_escape() {
  local value="$1"

  value=${value//\\/\\\\}
  value=${value//"/\\"}
  value=${value//$'\n'/\\n}
  value=${value//$'\r'/\\r}
  value=${value//$'\t'/\\t}

  printf '%s' "$value"
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

write_markdown_placeholder() {
  local path="$1"
  local title="$2"
  local description="$3"

  cat <<EOF > "$path"
# $title

Status: pending

$description
EOF
}

write_iteration_state() {
  local iteration_json_path="$1"
  local iteration_number="$2"
  local iteration_dir="$3"
  local hypothesis_path="$iteration_dir/$HYPOTHESIS_FILENAME"
  local implementation_path="$iteration_dir/$IMPLEMENTATION_FILENAME"
  local benchmark_path="$iteration_dir/$BENCHMARK_FILENAME"
  local decision_path="$iteration_dir/$DECISION_FILENAME"
  local escaped_baseline_version

  escaped_baseline_version="$(json_escape "$BASELINE_VERSION")"

  cat <<EOF > "$iteration_json_path"
{
  "iteration": $iteration_number,
  "baselineVersion": "$escaped_baseline_version",
  "state": "initialized",
  "artifacts": {
    "iterationJson": "$iteration_json_path",
    "hypothesis": "$hypothesis_path",
    "implementation": "$implementation_path",
    "benchmark": "$benchmark_path",
    "decision": "$decision_path"
  }
}
EOF
}

create_iteration_artifacts() {
  local session_dir="$1"
  local iteration_number="$2"
  local iterations_dir="$session_dir/$ITERATIONS_DIRNAME"
  local iteration_dir="$iterations_dir/$iteration_number"
  local iteration_json_path="$iteration_dir/$ITERATION_STATE_FILENAME"
  local hypothesis_path="$iteration_dir/$HYPOTHESIS_FILENAME"
  local implementation_path="$iteration_dir/$IMPLEMENTATION_FILENAME"
  local benchmark_path="$iteration_dir/$BENCHMARK_FILENAME"
  local decision_path="$iteration_dir/$DECISION_FILENAME"

  mkdir -p "$iteration_dir"

  write_iteration_state "$iteration_json_path" "$iteration_number" "$iteration_dir"
  write_markdown_placeholder "$hypothesis_path" "Iteration $iteration_number Hypothesis" "Describe the selected improvement idea and why it should help."
  write_markdown_placeholder "$implementation_path" "Iteration $iteration_number Implementation" "Summarize candidate changes and list modified files."
  write_markdown_placeholder "$benchmark_path" "Iteration $iteration_number Benchmark" "Record benchmark settings, completed games, and summary metrics."
  write_markdown_placeholder "$decision_path" "Iteration $iteration_number Decision" "Record the final outcome and the reason for it."

  printf '%s\n' "$iteration_dir"
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

INITIAL_ITERATION_DIR="$(create_iteration_artifacts "$SESSION_DIR" "$INITIAL_ITERATION_NUMBER")"
INITIAL_ITERATION_STATE_PATH="$INITIAL_ITERATION_DIR/$ITERATION_STATE_FILENAME"

echo "Starting Wiggum evolution loop"
echo "Baseline version: $BASELINE_VERSION"
echo "Output directory: $OUTPUT_DIR"
echo "Max iterations: $MAX_ITERATIONS"
echo "Session ID: $SESSION_ID"
echo "Session directory: $SESSION_DIR"
echo "Session summary: $SESSION_SUMMARY_PATH"
echo "Session metadata: $SESSION_METADATA_PATH"
echo "Initial iteration directory: $INITIAL_ITERATION_DIR"
echo "Initial iteration state: $INITIAL_ITERATION_STATE_PATH"
echo
echo "Session artifact root and initial iteration skeleton are ready. Future stories will populate iteration phases and loop control."
