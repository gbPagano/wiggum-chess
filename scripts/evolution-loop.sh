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
ACCEPTED_BASELINE_VERSION=""
ACCEPTED_BASELINE_REF=""
SESSION_SUMMARY_FILENAME="summary.md"
ITERATIONS_DIRNAME="iterations"
CANDIDATE_WORKSPACES_DIRNAME="candidate-workspaces"
INITIAL_ITERATION_NUMBER=1
ITERATION_STATE_FILENAME="iteration.json"
HYPOTHESIS_FILENAME="hypothesis.md"
IMPLEMENTATION_FILENAME="implementation.md"
BENCHMARK_FILENAME="benchmark.md"
DECISION_FILENAME="decision.md"
CORRECTNESS_DIRNAME="correctness"

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
accepted_baseline_version=$ACCEPTED_BASELINE_VERSION
accepted_baseline_ref=$ACCEPTED_BASELINE_REF
max_iterations=$MAX_ITERATIONS
session_id=$session_id
session_dir=$session_dir
summary_file=$SESSION_SUMMARY_FILENAME
EOF
}

resolve_accepted_baseline_ref() {
  if git -C "$REPO_ROOT" rev-parse --verify HEAD >/dev/null 2>&1; then
    ACCEPTED_BASELINE_REF="$(git -C "$REPO_ROOT" rev-parse HEAD)"
  else
    ACCEPTED_BASELINE_REF="main"
  fi

  ACCEPTED_BASELINE_VERSION="$BASELINE_VERSION"
}

candidate_workspace_root() {
  local session_dir="$1"

  printf '%s/%s\n' "$session_dir" "$CANDIDATE_WORKSPACES_DIRNAME"
}

candidate_workspace_path() {
  local session_dir="$1"
  local iteration_number="$2"

  printf '%s/%s\n' "$(candidate_workspace_root "$session_dir")" "$iteration_number"
}

candidate_branch_name() {
  local iteration_number="$1"

  printf 'wiggum-evolution/%s/iteration-%s\n' "$SESSION_ID" "$iteration_number"
}

remove_candidate_workspace() {
  local candidate_dir="$1"

  if [[ -d "$candidate_dir" ]]; then
    git -C "$REPO_ROOT" worktree remove --force "$candidate_dir" >/dev/null 2>&1 || true
    rm -rf "$candidate_dir"
  fi
}

create_candidate_workspace() {
  local candidate_dir="$1"
  local baseline_ref="$2"

  git -C "$REPO_ROOT" worktree add --detach "$candidate_dir" "$baseline_ref" >/dev/null 2>&1
}

create_candidate_branch() {
  local candidate_dir="$1"
  local candidate_branch="$2"

  git -C "$candidate_dir" checkout -b "$candidate_branch" >/dev/null 2>&1
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

correctness_dir_path() {
  local iteration_dir="$1"

  printf '%s/%s\n' "$iteration_dir" "$CORRECTNESS_DIRNAME"
}

correctness_results_path() {
  local iteration_dir="$1"

  printf '%s/results.md\n' "$(correctness_dir_path "$iteration_dir")"
}

record_correctness_failure() {
  local iteration_json_path="$1"
  local decision_path="$2"
  local benchmark_path="$3"
  local correctness_results_path="$4"
  local candidate_workspace_path="$5"
  local bash_check_status="$6"
  local cargo_build_status="$7"
  local cargo_test_status="$8"
  local failed_checks=()

  if [[ "$bash_check_status" == "failed" ]]; then
    failed_checks+=("bash -n scripts/evolution-loop.sh")
  fi

  if [[ "$cargo_build_status" == "failed" ]]; then
    failed_checks+=("cargo build --workspace")
  fi

  if [[ "$cargo_test_status" == "failed" ]]; then
    failed_checks+=("cargo test --workspace -- --skip gen_files::magics::name")
  fi

  python3 - <<'PY' "$iteration_json_path" "$candidate_workspace_path" "$bash_check_status" "$cargo_build_status" "$cargo_test_status"
import json
import sys

iteration_json_path, candidate_workspace_path, bash_check_status, cargo_build_status, cargo_test_status = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

data['state'] = 'failed'
data['correctness'] = {
    'status': 'completed',
    'passed': False,
    'benchmarkEligible': False,
    'checks': [
        {
            'name': 'bash -n scripts/evolution-loop.sh',
            'status': bash_check_status,
            'workspace': candidate_workspace_path,
        },
        {
            'name': 'cargo build --workspace',
            'status': cargo_build_status,
            'workspace': candidate_workspace_path,
        },
        {
            'name': 'cargo test --workspace -- --skip gen_files::magics::name',
            'status': cargo_test_status,
            'workspace': candidate_workspace_path,
        },
    ],
}

data.setdefault('benchmark', {})
data['benchmark']['status'] = 'skipped'
data['benchmark']['skippedReason'] = 'correctness gate failed'

data.setdefault('decision', {})
data['decision']['outcome'] = 'failed'
data['decision']['reasoning'] = 'Configured correctness checks failed before benchmarking, so the candidate is ineligible for promotion.'

actionable_failures = [
    check['name'] for check in data['correctness']['checks'] if check['status'] == 'failed'
]
data['decision']['evidence'] = actionable_failures

with open(iteration_json_path, 'w', encoding='utf-8') as handle:
    json.dump(data, handle, indent=2)
    handle.write('\n')
PY

  cat <<EOF > "$correctness_results_path"
# Iteration Correctness Gate

Status: failed

The configured correctness gate failed, so benchmark execution is skipped.

## Checks

- bash -n scripts/evolution-loop.sh: $bash_check_status
- cargo build --workspace: $cargo_build_status
- cargo test --workspace -- --skip gen_files::magics::name: $cargo_test_status
EOF

  cat <<EOF > "$benchmark_path"
# Iteration Benchmark

Status: skipped

Benchmark execution is skipped because the correctness gate failed.
EOF

  cat <<EOF > "$decision_path"
# Iteration Decision

Status: failed

The configured correctness gate failed before benchmarking, so the candidate is ineligible for promotion.

## Failed checks
$(for failed_check in "${failed_checks[@]}"; do printf -- '- %s\n' "$failed_check"; done)

## Benchmark

Skipped because the correctness gate did not pass.
EOF
}

record_correctness_success() {
  local iteration_json_path="$1"
  local correctness_results_path="$2"
  local candidate_workspace_path="$3"
  local bash_check_status="$4"
  local cargo_build_status="$5"
  local cargo_test_status="$6"

  python3 - <<'PY' "$iteration_json_path" "$candidate_workspace_path" "$bash_check_status" "$cargo_build_status" "$cargo_test_status"
import json
import sys

iteration_json_path, candidate_workspace_path, bash_check_status, cargo_build_status, cargo_test_status = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

data['correctness'] = {
    'status': 'completed',
    'passed': True,
    'benchmarkEligible': True,
    'checks': [
        {
            'name': 'bash -n scripts/evolution-loop.sh',
            'status': bash_check_status,
            'workspace': candidate_workspace_path,
        },
        {
            'name': 'cargo build --workspace',
            'status': cargo_build_status,
            'workspace': candidate_workspace_path,
        },
        {
            'name': 'cargo test --workspace -- --skip gen_files::magics::name',
            'status': cargo_test_status,
            'workspace': candidate_workspace_path,
        },
    ],
}

with open(iteration_json_path, 'w', encoding='utf-8') as handle:
    json.dump(data, handle, indent=2)
    handle.write('\n')
PY

  cat <<EOF > "$correctness_results_path"
# Iteration Correctness Gate

Status: passed

All configured correctness checks passed. Benchmarking remains eligible.

## Checks

- bash -n scripts/evolution-loop.sh: $bash_check_status
- cargo build --workspace: $cargo_build_status
- cargo test --workspace -- --skip gen_files::magics::name: $cargo_test_status
EOF
}

run_correctness_gate() {
  local iteration_json_path="$1"
  local correctness_results_path="$2"
  local benchmark_path="$3"
  local decision_path="$4"
  local candidate_workspace_path="$5"
  local bash_check_status="failed"
  local cargo_build_status="failed"
  local cargo_test_status="failed"

  if bash -n "$candidate_workspace_path/scripts/evolution-loop.sh" >/dev/null 2>&1; then
    bash_check_status="passed"
  fi

  if (cd "$candidate_workspace_path" && cargo build --workspace >/dev/null 2>&1); then
    cargo_build_status="passed"
  fi

  if (cd "$candidate_workspace_path" && cargo test --workspace -- --skip gen_files::magics::name >/dev/null 2>&1); then
    cargo_test_status="passed"
  fi

  if [[ "$bash_check_status" == "passed" && "$cargo_build_status" == "passed" && "$cargo_test_status" == "passed" ]]; then
    record_correctness_success "$iteration_json_path" "$correctness_results_path" "$candidate_workspace_path" "$bash_check_status" "$cargo_build_status" "$cargo_test_status"
    return 0
  fi

  record_correctness_failure "$iteration_json_path" "$decision_path" "$benchmark_path" "$correctness_results_path" "$candidate_workspace_path" "$bash_check_status" "$cargo_build_status" "$cargo_test_status"
  return 1
}

setup_candidate_workspace() {
  local session_dir="$1"
  local iteration_number="$2"
  local candidate_dir
  local candidate_branch
  local setup_status="ready"
  local setup_error=""

  candidate_dir="$(candidate_workspace_path "$session_dir" "$iteration_number")"
  candidate_branch="$(candidate_branch_name "$iteration_number")"

  mkdir -p "$(candidate_workspace_root "$session_dir")"
  remove_candidate_workspace "$candidate_dir"

  if create_candidate_workspace "$candidate_dir" "$ACCEPTED_BASELINE_REF"; then
    if ! create_candidate_branch "$candidate_dir" "$candidate_branch"; then
      setup_status="failed"
      setup_error="failed to create candidate branch $candidate_branch"
      remove_candidate_workspace "$candidate_dir"
    fi
  else
    setup_status="failed"
    setup_error="failed to create isolated git worktree at $candidate_dir from baseline $ACCEPTED_BASELINE_REF"
    rm -rf "$candidate_dir"
  fi

  printf '%s|%s|%s|%s\n' "$candidate_dir" "$candidate_branch" "$setup_status" "$setup_error"
}

write_iteration_state() {
  local iteration_json_path="$1"
  local iteration_number="$2"
  local iteration_dir="$3"
  local candidate_workspace_path="$4"
  local candidate_branch="$5"
  local candidate_setup_status="$6"
  local candidate_setup_error="$7"
  local hypothesis_path="$iteration_dir/$HYPOTHESIS_FILENAME"
  local implementation_path="$iteration_dir/$IMPLEMENTATION_FILENAME"
  local benchmark_path="$iteration_dir/$BENCHMARK_FILENAME"
  local decision_path="$iteration_dir/$DECISION_FILENAME"
  local correctness_dir
  local correctness_results
  local escaped_baseline_version
  local escaped_baseline_ref
  local escaped_candidate_workspace_path
  local escaped_candidate_branch
  local escaped_candidate_setup_status
  local escaped_candidate_setup_error
  local initial_state

  correctness_dir="$(correctness_dir_path "$iteration_dir")"
  correctness_results="$(correctness_results_path "$iteration_dir")"

  mkdir -p "$correctness_dir"


  escaped_baseline_version="$(json_escape "$ACCEPTED_BASELINE_VERSION")"
  escaped_baseline_ref="$(json_escape "$ACCEPTED_BASELINE_REF")"
  escaped_candidate_workspace_path="$(json_escape "$candidate_workspace_path")"
  escaped_candidate_branch="$(json_escape "$candidate_branch")"
  escaped_candidate_setup_status="$(json_escape "$candidate_setup_status")"
  escaped_candidate_setup_error="$(json_escape "$candidate_setup_error")"

  initial_state="initialized"
  if [[ "$candidate_setup_status" == "failed" ]]; then
    initial_state="failed"
  fi

  cat <<EOF > "$iteration_json_path"
{
  "iteration": $iteration_number,
  "baselineVersion": "$escaped_baseline_version",
  "baselineRef": "$escaped_baseline_ref",
  "state": "$initial_state",
  "isolation": {
    "type": "git-worktree",
    "path": "$escaped_candidate_workspace_path",
    "branch": "$escaped_candidate_branch",
    "status": "$escaped_candidate_setup_status",
    "setupError": "$escaped_candidate_setup_error"
  },
  "correctness": {
    "status": "pending",
    "passed": false,
    "benchmarkEligible": false,
    "checks": []
  },
  "artifacts": {
    "iterationJson": "$iteration_json_path",
    "hypothesis": "$hypothesis_path",
    "implementation": "$implementation_path",
    "correctness": "$correctness_results",
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
  local correctness_results
  local isolation_fields
  local candidate_workspace_path
  local candidate_branch
  local candidate_setup_status
  local candidate_setup_error

  mkdir -p "$iteration_dir"
  mkdir -p "$(correctness_dir_path "$iteration_dir")"
  correctness_results="$(correctness_results_path "$iteration_dir")"

  isolation_fields="$(setup_candidate_workspace "$session_dir" "$iteration_number")"
  IFS='|' read -r candidate_workspace_path candidate_branch candidate_setup_status candidate_setup_error <<< "$isolation_fields"
  LAST_CANDIDATE_WORKSPACE_PATH="$candidate_workspace_path"
  LAST_CANDIDATE_WORKSPACE_STATUS="$candidate_setup_status"

  write_iteration_state "$iteration_json_path" "$iteration_number" "$iteration_dir" "$candidate_workspace_path" "$candidate_branch" "$candidate_setup_status" "$candidate_setup_error"
  write_markdown_placeholder "$hypothesis_path" "Iteration $iteration_number Hypothesis" "Describe the selected improvement idea and why it should help."
  write_markdown_placeholder "$implementation_path" "Iteration $iteration_number Implementation" "Summarize candidate changes and list modified files."
  write_markdown_placeholder "$correctness_results" "Iteration $iteration_number Correctness Gate" "Record configured correctness checks and whether benchmarking remains eligible."
  write_markdown_placeholder "$benchmark_path" "Iteration $iteration_number Benchmark" "Record benchmark settings, completed games, and summary metrics."
  write_markdown_placeholder "$decision_path" "Iteration $iteration_number Decision" "Record the final outcome and the reason for it."

  if [[ "$candidate_setup_status" == "failed" ]]; then
    python3 - <<'PY' "$iteration_json_path" "$candidate_workspace_path" "$candidate_setup_error"
import json
import sys

iteration_json_path, candidate_workspace_path, candidate_setup_error = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

data['correctness'] = {
    'status': 'skipped',
    'passed': False,
    'benchmarkEligible': False,
    'checks': [],
    'skippedReason': 'candidate workspace setup failed',
}

data['benchmark'] = {
    'status': 'skipped',
    'skippedReason': 'candidate workspace setup failed',
}

data['decision'] = {
    'outcome': 'failed',
    'reasoning': 'Candidate workspace setup failed before correctness checks or benchmarking could run.',
    'evidence': [candidate_setup_error],
}

with open(iteration_json_path, 'w', encoding='utf-8') as handle:
    json.dump(data, handle, indent=2)
    handle.write('\n')
PY

    cat <<EOF > "$correctness_results"
# Iteration $iteration_number Correctness Gate

Status: skipped

Candidate workspace setup failed before correctness checks could run.

Reason: $candidate_setup_error
EOF

    cat <<EOF > "$benchmark_path"
# Iteration $iteration_number Benchmark

Status: skipped

Benchmark execution is skipped because the candidate workspace setup failed.

Reason: $candidate_setup_error
EOF

    cat <<EOF > "$decision_path"
# Iteration $iteration_number Decision

Status: failed

Candidate workspace setup failed before proposal, correctness validation, or benchmarking could begin.

Reason: $candidate_setup_error
EOF
  fi

  LAST_ITERATION_DIR="$iteration_dir"
  LAST_ITERATION_STATE_PATH="$iteration_json_path"
  LAST_CORRECTNESS_RESULTS_PATH="$correctness_results"
  LAST_BENCHMARK_PATH="$benchmark_path"
  LAST_DECISION_PATH="$decision_path"
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

resolve_accepted_baseline_ref
mkdir -p "$SESSION_DIR"
write_session_summary_placeholder "$SESSION_SUMMARY_PATH"
write_session_metadata "$SESSION_METADATA_PATH" "$SESSION_ID" "$SESSION_DIR"

create_iteration_artifacts "$SESSION_DIR" "$INITIAL_ITERATION_NUMBER"
INITIAL_ITERATION_DIR="$LAST_ITERATION_DIR"
INITIAL_ITERATION_STATE_PATH="$LAST_ITERATION_STATE_PATH"
INITIAL_CORRECTNESS_RESULTS_PATH="$LAST_CORRECTNESS_RESULTS_PATH"
INITIAL_BENCHMARK_PATH="$LAST_BENCHMARK_PATH"
INITIAL_DECISION_PATH="$LAST_DECISION_PATH"

if [[ "$LAST_CANDIDATE_WORKSPACE_STATUS" == "ready" ]]; then
  run_correctness_gate "$INITIAL_ITERATION_STATE_PATH" "$INITIAL_CORRECTNESS_RESULTS_PATH" "$INITIAL_BENCHMARK_PATH" "$INITIAL_DECISION_PATH" "$LAST_CANDIDATE_WORKSPACE_PATH" || true
fi

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
echo "Initial candidate workspace: $LAST_CANDIDATE_WORKSPACE_PATH"
echo "Initial candidate workspace status: $LAST_CANDIDATE_WORKSPACE_STATUS"
echo "Initial correctness artifact: $INITIAL_CORRECTNESS_RESULTS_PATH"
echo
echo "Each iteration starts from an isolated candidate worktree. The orchestration flow runs configured correctness checks before benchmarking, and failed checks make the iteration ineligible for promotion."
