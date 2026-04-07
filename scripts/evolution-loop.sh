#!/usr/bin/env bash
# evolution-loop.sh
#
# Start a Wiggum engine evolution session.
#
# Usage:
#   ./scripts/evolution-loop.sh --baseline-version <version> [--output-dir <path>] [--max-iterations <count>] [--max-infra-failures <count>]
#   ./scripts/evolution-loop.sh --help
#
# Options:
#   --baseline-version     Required. Baseline engine version to evolve from.
#   --output-dir           Session artifact root. Defaults to tasks/evolution-runs.
#   --max-iterations       Maximum iterations to allow. Defaults to 10.
#   --max-infra-failures   Maximum failed iterations to tolerate before stopping. Defaults to 3.
#   --help                 Print this help text.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKER_GUIDANCE="$REPO_ROOT/.claude/evolution/CLAUDE.md"
STATE_MACHINE_REFERENCE="$REPO_ROOT/tasks/prd-wiggum-evolution-loop.md"
DISCARD_POLICY_REFERENCE="$REPO_ROOT/tasks/prd-wiggum-evolution-loop.md"
CLAUDE_BIN="${CLAUDE_BIN:-openclaude}"

BASELINE_VERSION=""
OUTPUT_DIR="$REPO_ROOT/tasks/evolution-runs"
MAX_ITERATIONS=10
MAX_INFRA_FAILURES=3
ACCEPTED_BASELINE_VERSION=""
ACCEPTED_BASELINE_REF=""
SESSION_SUMMARY_FILENAME="summary.md"
ITERATIONS_DIRNAME="iterations"
CANDIDATE_WORKSPACES_DIRNAME="candidate-workspaces"
PHASE_LOGS_DIRNAME="phase-logs"
INITIAL_ITERATION_NUMBER=1
ITERATION_STATE_FILENAME="iteration.json"
HYPOTHESIS_FILENAME="hypothesis.md"
IMPLEMENTATION_FILENAME="implementation.md"
BENCHMARK_FILENAME="benchmark.md"
DECISION_FILENAME="decision.md"
CORRECTNESS_DIRNAME="correctness"
LAST_PHASE_LOG_PATH=""
LAST_PHASE_EXIT_STATUS=0
LAST_PHASE_SKILL_NAME=""
LAST_PHASE_NAME=""
LAST_PHASE_RESULT=""
LAST_PHASE_LOG_RELATIVE_PATH=""

usage() {
  sed -n '1,15p' "$0"
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

update_iteration_state() {
  local iteration_json_path="$1"
  local expected_state="$2"
  local next_state="$3"
  local error_context="$4"

  python3 - <<'PY' "$iteration_json_path" "$expected_state" "$next_state" "$error_context"
import json
import sys

iteration_json_path, expected_state, next_state, error_context = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

current_state = data.get('state')
if current_state != expected_state:
    raise SystemExit(
        f"Invalid iteration state transition for {error_context}: expected {expected_state!r}, found {current_state!r}"
    )

data['state'] = next_state
if 'stateMachine' in data:
    data['stateMachine']['currentPhase'] = next_state

with open(iteration_json_path, 'w', encoding='utf-8') as handle:
    json.dump(data, handle, indent=2)
    handle.write('\n')
PY
}

set_iteration_final_state() {
  local iteration_json_path="$1"
  local next_state="$2"
  local error_context="$3"

  python3 - <<'PY' "$iteration_json_path" "$next_state" "$error_context"
import json
import sys

iteration_json_path, next_state, error_context = sys.argv[1:]
final_states = {'accepted', 'rejected', 'inconclusive', 'failed'}

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

current_state = data.get('state')
if current_state != 'deciding':
    raise SystemExit(
        f"Invalid iteration state transition for {error_context}: expected 'deciding', found {current_state!r}"
    )
if next_state not in final_states:
    raise SystemExit(
        f"Invalid final iteration state for {error_context}: {next_state!r}"
    )

data['state'] = next_state
if 'stateMachine' in data:
    data['stateMachine']['currentPhase'] = next_state

with open(iteration_json_path, 'w', encoding='utf-8') as handle:
    json.dump(data, handle, indent=2)
    handle.write('\n')
PY
}

session_id_now() {
  date -u +"%Y%m%dT%H%M%SZ"
}

write_session_summary_placeholder() {
  local summary_path="$1"

  cat <<EOF > "$summary_path"
# Wiggum Evolution Session Summary

Status: pending final session summary.
EOF
}

write_session_summary_final() {
  local summary_path="$1"
  local session_dir="$2"
  local completed_iterations="$3"
  local stop_reason="$4"
  local stop_reason_details="$5"
  local accepted_baseline_version="$6"
  local accepted_baseline_ref="$7"

  python3 - <<'PY' "$summary_path" "$session_dir" "$SESSION_ID" "$BASELINE_VERSION" "$MAX_ITERATIONS" "$completed_iterations" "$stop_reason" "$stop_reason_details" "$accepted_baseline_version" "$accepted_baseline_ref"
import json
import os
import sys

(
    summary_path,
    session_dir,
    session_id,
    baseline_version,
    max_iterations,
    completed_iterations,
    stop_reason,
    stop_reason_details,
    accepted_baseline_version,
    accepted_baseline_ref,
) = sys.argv[1:]

iterations_dir = os.path.join(session_dir, 'iterations')
entries = []
accepted_versions = []
rejected_attempts = []

if os.path.isdir(iterations_dir):
    for name in sorted(os.listdir(iterations_dir), key=lambda value: int(value) if value.isdigit() else value):
        iteration_dir = os.path.join(iterations_dir, name)
        iteration_json = os.path.join(iteration_dir, 'iteration.json')
        if not os.path.isfile(iteration_json):
            continue

        with open(iteration_json, 'r', encoding='utf-8') as handle:
            data = json.load(handle)

        outcome = data.get('state', 'unknown')
        decision = data.get('decision', {}) or {}
        promotion = decision.get('promotion', {}) or {}
        promoted_version = decision.get('promotedVersion') or promotion.get('promotedVersion') or ''
        summary = {
            'iteration': data.get('iteration', name),
            'outcome': outcome,
            'promoted_version': promoted_version,
            'hypothesis': os.path.relpath(os.path.join(iteration_dir, 'hypothesis.md'), session_dir),
            'implementation': os.path.relpath(os.path.join(iteration_dir, 'implementation.md'), session_dir),
            'correctness': os.path.relpath(os.path.join(iteration_dir, 'correctness', 'results.md'), session_dir),
            'benchmark': os.path.relpath(os.path.join(iteration_dir, 'benchmark.md'), session_dir),
            'decision': os.path.relpath(os.path.join(iteration_dir, 'decision.md'), session_dir),
            'iteration_json': os.path.relpath(iteration_json, session_dir),
        }
        entries.append(summary)

        if outcome == 'accepted':
            accepted_versions.append(promoted_version or data.get('baselineVersion', 'unknown'))
        if outcome == 'rejected':
            rejected_attempts.append(str(data.get('iteration', name)))

lines = [
    '# Wiggum Evolution Session Summary',
    '',
    '## Session',
    f'- Session id: {session_id}',
    f'- Baseline version: {baseline_version}',
    f'- Final accepted baseline version: {accepted_baseline_version}',
    f'- Final accepted baseline ref: {accepted_baseline_ref}',
    f'- Max iterations: {max_iterations}',
    f'- Completed iterations: {completed_iterations}',
    f'- Session directory: {session_dir}',
    f'- Summary file: {summary_path}',
    f'- Stop reason: {stop_reason}',
]

if stop_reason_details:
    lines.append(f'- Stop details: {stop_reason_details}')

lines.extend([
    '',
    '## Outcomes',
    '- Accepted versions: ' + (', '.join(accepted_versions) if accepted_versions else 'none'),
    '- Rejected attempts: ' + (', '.join(rejected_attempts) if rejected_attempts else 'none'),
    '',
    '## Iteration artifacts',
])

if entries:
    for entry in entries:
        lines.extend([
            f"### Iteration {entry['iteration']} — {entry['outcome']}",
            f"- iteration.json: `{entry['iteration_json']}`",
            f"- hypothesis: `{entry['hypothesis']}`",
            f"- implementation: `{entry['implementation']}`",
            f"- correctness: `{entry['correctness']}`",
            f"- benchmark: `{entry['benchmark']}`",
            f"- decision: `{entry['decision']}`",
        ])
        if entry['promoted_version']:
            lines.append(f"- promoted version: `{entry['promoted_version']}`")
        lines.append('')
else:
    lines.append('- No iteration artifacts were created.')
    lines.append('')

with open(summary_path, 'w', encoding='utf-8') as handle:
    handle.write('\n'.join(lines).rstrip() + '\n')
PY
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
max_infra_failures=$MAX_INFRA_FAILURES
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

phase_logs_dir_path() {
  local iteration_dir="$1"

  printf '%s/%s\n' "$iteration_dir" "$PHASE_LOGS_DIRNAME"
}

phase_log_path() {
  local iteration_dir="$1"
  local phase_name="$2"

  printf '%s/%s.log\n' "$(phase_logs_dir_path "$iteration_dir")" "$phase_name"
}

phase_log_relative_path() {
  local iteration_dir="$1"
  local phase_name="$2"

  python3 - <<'PY' "$SESSION_DIR" "$(phase_log_path "$iteration_dir" "$phase_name")"
import os
import sys

session_dir, phase_log_path = sys.argv[1:]
print(os.path.relpath(phase_log_path, session_dir))
PY
}

current_iteration_result() {
  local iteration_json_path="$1"

  python3 - <<'PY' "$iteration_json_path"
import json
import sys

with open(sys.argv[1], 'r', encoding='utf-8') as handle:
    data = json.load(handle)

print(data.get('state', ''))
PY
}

run_logged_phase() {
  local phase_name="$1"
  local skill_name="$2"
  local candidate_workspace_path="$3"
  local iteration_dir="$4"
  local iteration_json_path="$5"
  local phase_log
  local phase_status=0
  local iteration_result

  phase_log="$(phase_log_path "$iteration_dir" "$phase_name")"
  mkdir -p "$(phase_logs_dir_path "$iteration_dir")"

  if run_claude_skill "$skill_name" "$candidate_workspace_path" "$iteration_dir" "$iteration_json_path" >"$phase_log" 2>&1; then
    phase_status=0
  else
    phase_status=$?
  fi

  iteration_result="$(current_iteration_result "$iteration_json_path")"

  LAST_PHASE_LOG_PATH="$phase_log"
  LAST_PHASE_EXIT_STATUS="$phase_status"
  LAST_PHASE_SKILL_NAME="$skill_name"
  LAST_PHASE_NAME="$phase_name"
  LAST_PHASE_RESULT="$iteration_result"
  LAST_PHASE_LOG_RELATIVE_PATH="$(phase_log_relative_path "$iteration_dir" "$phase_name")"

  echo "Phase $phase_name result: ${iteration_result:-unknown}"
  echo "Phase $phase_name log: $LAST_PHASE_LOG_RELATIVE_PATH"

  return "$phase_status"
}

correctness_dir_path() {
  local iteration_dir="$1"

  printf '%s/%s\n' "$iteration_dir" "$CORRECTNESS_DIRNAME"
}

correctness_results_path() {
  local iteration_dir="$1"

  printf '%s/results.md\n' "$(correctness_dir_path "$iteration_dir")"
}

current_iteration_state() {
  local iteration_json_path="$1"

  python3 - <<'PY' "$iteration_json_path"
import json
import sys

with open(sys.argv[1], 'r', encoding='utf-8') as handle:
    data = json.load(handle)

print(data.get('state', ''))
PY
}

iteration_has_no_hypothesis() {
  local iteration_json_path="$1"

  python3 - <<'PY' "$iteration_json_path"
import json
import sys

with open(sys.argv[1], 'r', encoding='utf-8') as handle:
    data = json.load(handle)

hypothesis = data.get('hypothesis', {}) or {}
signals = {
    data.get('state', ''),
    str(hypothesis.get('state', '')),
    str(hypothesis.get('status', '')),
    str(hypothesis.get('stopSignal', '')),
}
print('yes' if 'no_hypothesis' in signals else 'no')
PY
}

current_promoted_version() {
  local iteration_json_path="$1"

  python3 - <<'PY' "$iteration_json_path"
import json
import sys

with open(sys.argv[1], 'r', encoding='utf-8') as handle:
    data = json.load(handle)

decision = data.get('decision', {}) or {}
promotion = decision.get('promotion', {}) or {}
promoted_version = decision.get('promotedVersion') or promotion.get('promotedVersion') or ''
print(promoted_version)
PY
}

record_phase_failure() {
  local iteration_json_path="$1"
  local decision_path="$2"
  local benchmark_path="$3"
  local phase_name="$4"
  local failure_reason="$5"

  python3 - <<'PY' "$iteration_json_path" "$phase_name" "$failure_reason"
import json
import sys

iteration_json_path, phase_name, failure_reason = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

data['state'] = 'failed'
data.setdefault('stateMachine', {})['currentPhase'] = 'failed'

data.setdefault('decision', {})
data['decision']['outcome'] = 'failed'
data['decision']['reasoning'] = failure_reason
data['decision']['evidence'] = [phase_name]

if phase_name == 'propose':
    data['hypothesis'] = {
        'status': 'failed',
        'summary': 'Hypothesis generation failed.',
        'failureReason': failure_reason,
        'targetMetrics': [],
        'buildsOn': [],
    }
    data.setdefault('benchmark', {})
    data['benchmark']['status'] = 'skipped'
    data['benchmark']['skippedReason'] = 'proposal phase failed'
elif phase_name == 'implement':
    data['implementation'] = {
        'summary': 'Implementation phase failed before a candidate was completed.',
        'failureReason': failure_reason,
        'changedFiles': [],
    }
    data.setdefault('benchmark', {})
    data['benchmark']['status'] = 'skipped'
    data['benchmark']['skippedReason'] = 'implementation phase failed'
elif phase_name == 'benchmark':
    data['benchmark'] = {
        'status': 'failed',
        'failureReason': failure_reason,
        'sufficientForPromotion': False,
    }

with open(iteration_json_path, 'w', encoding='utf-8') as handle:
    json.dump(data, handle, indent=2)
    handle.write('\n')
PY

  if [[ "$phase_name" == "propose" || "$phase_name" == "implement" ]]; then
    cat <<EOF > "$benchmark_path"
# Iteration Benchmark

Status: skipped

Benchmark execution is skipped because the $phase_name phase failed.

Reason: $failure_reason
EOF
  elif [[ "$phase_name" == "benchmark" ]]; then
    cat <<EOF > "$benchmark_path"
# Iteration Benchmark

Status: failed

Benchmark execution failed.

Reason: $failure_reason
EOF
  fi

  cat <<EOF > "$decision_path"
# Iteration Decision

Status: failed

The $phase_name phase failed.

Reason: $failure_reason
EOF
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
  local current_state

  current_state="$(current_iteration_state "$iteration_json_path")"

  if [[ "$current_state" == "implemented" ]]; then
    update_iteration_state "$iteration_json_path" "implemented" "validating" "correctness gate start"
    current_state="validating"
  fi

  if [[ "$current_state" == "validating" ]]; then
    update_iteration_state "$iteration_json_path" "validating" "deciding" "correctness failure decision handoff"
    current_state="deciding"
  fi

  if [[ "$current_state" != "deciding" ]]; then
    echo "Invalid iteration state transition for correctness gate failure: expected 'implemented', 'validating', or 'deciding', found '$current_state'" >&2
    exit 1
  fi

  if [[ "$bash_check_status" == "failed" ]]; then
    failed_checks+=("bash -n scripts/evolution-loop.sh")
  fi

  if [[ "$cargo_build_status" == "failed" ]]; then
    failed_checks+=("cargo build --workspace")
  fi

  if [[ "$cargo_test_status" == "failed" ]]; then
    failed_checks+=("cargo test --workspace -- --skip gen_files::magics::name")
  fi

  set_iteration_final_state "$iteration_json_path" "failed" "correctness gate failure"

  python3 - <<'PY' "$iteration_json_path" "$candidate_workspace_path" "$bash_check_status" "$cargo_build_status" "$cargo_test_status"
import json
import sys

iteration_json_path, candidate_workspace_path, bash_check_status, cargo_build_status, cargo_test_status = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

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
data['stateMachine']['currentPhase'] = data['state']

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

  update_iteration_state "$iteration_json_path" "implemented" "validating" "correctness gate start"

  python3 - <<'PY' "$iteration_json_path" "$candidate_workspace_path" "$bash_check_status" "$cargo_build_status" "$cargo_test_status"
import json
import sys

iteration_json_path, candidate_workspace_path, bash_check_status, cargo_build_status, cargo_test_status = sys.argv[1:]

with open(iteration_json_path, 'r', encoding='utf-8') as handle:
    data = json.load(handle)

data['state'] = 'implemented'
data['stateMachine']['currentPhase'] = data['state']
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
  local phase_logs_dir
  local escaped_phase_logs_dir
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
  phase_logs_dir="$(phase_logs_dir_path "$iteration_dir")"

  mkdir -p "$correctness_dir"
  mkdir -p "$phase_logs_dir"

  escaped_baseline_version="$(json_escape "$ACCEPTED_BASELINE_VERSION")"
  escaped_baseline_ref="$(json_escape "$ACCEPTED_BASELINE_REF")"
  escaped_candidate_workspace_path="$(json_escape "$candidate_workspace_path")"
  escaped_candidate_branch="$(json_escape "$candidate_branch")"
  escaped_candidate_setup_status="$(json_escape "$candidate_setup_status")"
  escaped_candidate_setup_error="$(json_escape "$candidate_setup_error")"
  escaped_phase_logs_dir="$(json_escape "$phase_logs_dir")"

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
  "stateMachine": {
    "reference": "$STATE_MACHINE_REFERENCE",
    "currentPhase": "$initial_state",
    "finalStates": ["accepted", "rejected", "inconclusive", "failed"]
  },
  "artifacts": {
    "iterationJson": "$iteration_json_path",
    "hypothesis": "$hypothesis_path",
    "implementation": "$implementation_path",
    "correctness": "$correctness_results",
    "benchmark": "$benchmark_path",
    "decision": "$decision_path",
    "phaseLogsDir": "$escaped_phase_logs_dir",
    "phaseLogs": {
      "propose": "$(phase_log_path "$iteration_dir" "propose")",
      "implement": "$(phase_log_path "$iteration_dir" "implement")",
      "benchmark": "$(phase_log_path "$iteration_dir" "benchmark")",
      "decide": "$(phase_log_path "$iteration_dir" "decide")"
    }
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

data['stateMachine']['currentPhase'] = data['state']

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

run_claude_skill() {
  local skill_name="$1"
  local candidate_workspace_path="$2"
  local iteration_dir="$3"
  local iteration_json_path="$4"

  (
    cd "$candidate_workspace_path" &&
      "$CLAUDE_BIN" --dangerously-skip-permissions --add-dir "$SESSION_DIR" --add-dir "$REPO_ROOT" --print <<EOF
/$skill_name

Run only this iteration phase.

- Repository root: $candidate_workspace_path
- Session directory: $SESSION_DIR
- Iteration directory: $iteration_dir
- Iteration state file: $iteration_json_path
- Session metadata file: $SESSION_METADATA_PATH
- Worker guidance: $WORKER_GUIDANCE

Read and update the iteration artifacts at the paths recorded in iteration.json.
If you cannot complete the phase, record the failure in the appropriate iteration artifact and iteration.json.
If no valid next hypothesis exists during /evolution-propose, record a stop signal in iteration.json using hypothesis.status = "no_hypothesis" and explain it in hypothesis.md.
EOF
  )
}

candidate_workspace_has_changes() {
  local candidate_workspace_path="$1"

  if ! git -C "$candidate_workspace_path" diff --quiet --ignore-submodules --; then
    return 0
  fi

  if ! git -C "$candidate_workspace_path" diff --cached --quiet --ignore-submodules --; then
    return 0
  fi

  if [[ -n "$(git -C "$candidate_workspace_path" ls-files --others --exclude-standard)" ]]; then
    return 0
  fi

  return 1
}

promote_candidate_workspace() {
  local candidate_workspace_path="$1"
  local iteration_json_path="$2"
  local iteration_number="$3"
  local promoted_version

  if candidate_workspace_has_changes "$candidate_workspace_path"; then
    git -C "$candidate_workspace_path" add -A
    git -C "$candidate_workspace_path" commit -m "chore: accept evolution iteration $iteration_number" >/dev/null 2>&1
  fi

  ACCEPTED_BASELINE_REF="$(git -C "$candidate_workspace_path" rev-parse HEAD)"
  promoted_version="$(current_promoted_version "$iteration_json_path")"
  if [[ -n "$promoted_version" ]]; then
    ACCEPTED_BASELINE_VERSION="$promoted_version"
  fi

  write_session_metadata "$SESSION_METADATA_PATH" "$SESSION_ID" "$SESSION_DIR"
}

run_iteration() {
  local iteration_number="$1"
  local iteration_state
  local no_hypothesis_signal

  create_iteration_artifacts "$SESSION_DIR" "$iteration_number"

  if [[ "$LAST_CANDIDATE_WORKSPACE_STATUS" != "ready" ]]; then
    return 0
  fi

  update_iteration_state "$LAST_ITERATION_STATE_PATH" "initialized" "proposing" "proposal phase start"
  if ! run_logged_phase "propose" "evolution-propose" "$LAST_CANDIDATE_WORKSPACE_PATH" "$LAST_ITERATION_DIR" "$LAST_ITERATION_STATE_PATH"; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "propose" "Claude skill execution failed during the propose phase. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi

  no_hypothesis_signal="$(iteration_has_no_hypothesis "$LAST_ITERATION_STATE_PATH")"
  if [[ "$no_hypothesis_signal" == "yes" ]]; then
    STOP_REASON="no valid next hypothesis could be generated"
    STOP_REASON_DETAILS="iteration $iteration_number returned a no_hypothesis stop signal"
    remove_candidate_workspace "$LAST_CANDIDATE_WORKSPACE_PATH"
    return 0
  fi

  iteration_state="$(current_iteration_state "$LAST_ITERATION_STATE_PATH")"
  if [[ "$iteration_state" != "proposed" ]]; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "propose" "Proposal phase completed without writing state 'proposed'. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi

  update_iteration_state "$LAST_ITERATION_STATE_PATH" "proposed" "implementing" "implementation phase start"
  if ! run_logged_phase "implement" "evolution-implement" "$LAST_CANDIDATE_WORKSPACE_PATH" "$LAST_ITERATION_DIR" "$LAST_ITERATION_STATE_PATH"; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "implement" "Claude skill execution failed during the implementation phase. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi

  iteration_state="$(current_iteration_state "$LAST_ITERATION_STATE_PATH")"
  if [[ "$iteration_state" == "failed" ]]; then
    return 0
  fi

  if [[ "$iteration_state" != "implemented" ]]; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "implement" "Implementation phase completed without writing state 'implemented'. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi

  if ! run_correctness_gate "$LAST_ITERATION_STATE_PATH" "$LAST_CORRECTNESS_RESULTS_PATH" "$LAST_BENCHMARK_PATH" "$LAST_DECISION_PATH" "$LAST_CANDIDATE_WORKSPACE_PATH"; then
    return 0
  fi

  iteration_state="$(current_iteration_state "$LAST_ITERATION_STATE_PATH")"
  if [[ "$iteration_state" != "implemented" ]]; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "benchmark" "Correctness gate returned an unexpected state before benchmarking."
    return 0
  fi

  if ! run_logged_phase "benchmark" "evolution-benchmark" "$LAST_CANDIDATE_WORKSPACE_PATH" "$LAST_ITERATION_DIR" "$LAST_ITERATION_STATE_PATH"; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "benchmark" "Claude skill execution failed during the benchmark phase. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi

  iteration_state="$(current_iteration_state "$LAST_ITERATION_STATE_PATH")"
  if [[ "$iteration_state" == "failed" ]]; then
    return 0
  fi

  if [[ "$iteration_state" != "benchmarked" ]]; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "benchmark" "Benchmark phase completed without writing state 'benchmarked'. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi

  update_iteration_state "$LAST_ITERATION_STATE_PATH" "benchmarked" "deciding" "decision phase start"
  if ! run_logged_phase "decide" "evolution-decide" "$LAST_CANDIDATE_WORKSPACE_PATH" "$LAST_ITERATION_DIR" "$LAST_ITERATION_STATE_PATH"; then
    record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "decision" "Claude skill execution failed during the decision phase. See $LAST_PHASE_LOG_RELATIVE_PATH for details."
    return 0
  fi
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
    --max-infra-failures)
      require_value "$1" "${2-}"
      MAX_INFRA_FAILURES="$2"
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

if ! [[ "$MAX_INFRA_FAILURES" =~ ^[1-9][0-9]*$ ]]; then
  echo "Error: --max-infra-failures must be a positive integer." >&2
  exit 1
fi

if ! command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
  echo "Error: Claude CLI binary '$CLAUDE_BIN' was not found in PATH." >&2
  exit 1
fi

SESSION_ID="$(session_id_now)"
SESSION_DIR="$OUTPUT_DIR/$SESSION_ID"
SESSION_SUMMARY_PATH="$SESSION_DIR/$SESSION_SUMMARY_FILENAME"
SESSION_METADATA_PATH="$SESSION_DIR/session.env"
STOP_REASON=""
STOP_REASON_DETAILS=""
INFRA_FAILURE_COUNT=0
ITERATION_NUMBER=$INITIAL_ITERATION_NUMBER
LAST_COMPLETED_ITERATION=0

resolve_accepted_baseline_ref
mkdir -p "$SESSION_DIR"
write_session_summary_placeholder "$SESSION_SUMMARY_PATH"
write_session_metadata "$SESSION_METADATA_PATH" "$SESSION_ID" "$SESSION_DIR"

echo "Starting Wiggum evolution loop"
echo "Baseline version: $BASELINE_VERSION"
echo "Output directory: $OUTPUT_DIR"
echo "Max iterations: $MAX_ITERATIONS"
echo "Max infrastructure failures: $MAX_INFRA_FAILURES"
echo "Session ID: $SESSION_ID"
echo "Session directory: $SESSION_DIR"
echo "Session summary: $SESSION_SUMMARY_PATH"
echo "Session metadata: $SESSION_METADATA_PATH"
echo
echo "Each iteration starts from an isolated candidate worktree. Non-winning outcomes keep the accepted baseline unchanged, preserve iteration artifacts for audit, and leave the candidate worktree isolated until the discard step removes it."
echo "Discard and restore policy reference: $DISCARD_POLICY_REFERENCE"
echo "The orchestration flow runs configured correctness checks before benchmarking, and failed checks make the iteration ineligible for promotion."
echo

while (( ITERATION_NUMBER <= MAX_ITERATIONS )); do
  echo "==============================================================="
  echo "  Evolution iteration $ITERATION_NUMBER of $MAX_ITERATIONS"
  echo "==============================================================="

  run_iteration "$ITERATION_NUMBER"

  LAST_COMPLETED_ITERATION=$ITERATION_NUMBER
  CURRENT_ITERATION_STATE="$(current_iteration_state "$LAST_ITERATION_STATE_PATH")"

  if [[ -n "$STOP_REASON" ]]; then
    break
  fi

  case "$CURRENT_ITERATION_STATE" in
    accepted)
      if ! promote_candidate_workspace "$LAST_CANDIDATE_WORKSPACE_PATH" "$LAST_ITERATION_STATE_PATH" "$ITERATION_NUMBER"; then
        record_phase_failure "$LAST_ITERATION_STATE_PATH" "$LAST_DECISION_PATH" "$LAST_BENCHMARK_PATH" "decision" "Accepted candidate could not be persisted as the next baseline."
        CURRENT_ITERATION_STATE="failed"
      else
        INFRA_FAILURE_COUNT=0
      fi
      remove_candidate_workspace "$LAST_CANDIDATE_WORKSPACE_PATH"
      ;;
    rejected|inconclusive)
      INFRA_FAILURE_COUNT=0
      remove_candidate_workspace "$LAST_CANDIDATE_WORKSPACE_PATH"
      ;;
    failed)
      INFRA_FAILURE_COUNT=$((INFRA_FAILURE_COUNT + 1))
      remove_candidate_workspace "$LAST_CANDIDATE_WORKSPACE_PATH"
      if (( INFRA_FAILURE_COUNT >= MAX_INFRA_FAILURES )); then
        STOP_REASON="infrastructure failure limit reached"
        STOP_REASON_DETAILS="$INFRA_FAILURE_COUNT failed iterations reached the configured limit of $MAX_INFRA_FAILURES"
      fi
      ;;
    *)
      INFRA_FAILURE_COUNT=$((INFRA_FAILURE_COUNT + 1))
      remove_candidate_workspace "$LAST_CANDIDATE_WORKSPACE_PATH"
      STOP_REASON="unexpected iteration state"
      STOP_REASON_DETAILS="iteration $ITERATION_NUMBER ended in unsupported state '$CURRENT_ITERATION_STATE'"
      ;;
  esac

  if [[ -n "$STOP_REASON" ]]; then
    break
  fi

  ITERATION_NUMBER=$((ITERATION_NUMBER + 1))
done

if [[ -z "$STOP_REASON" ]]; then
  STOP_REASON="max_iterations reached"
  STOP_REASON_DETAILS="$LAST_COMPLETED_ITERATION iterations completed without another stop condition"
fi

write_session_summary_final \
  "$SESSION_SUMMARY_PATH" \
  "$SESSION_DIR" \
  "$LAST_COMPLETED_ITERATION" \
  "$STOP_REASON" \
  "$STOP_REASON_DETAILS" \
  "$ACCEPTED_BASELINE_VERSION" \
  "$ACCEPTED_BASELINE_REF"

echo
echo "Evolution loop stopped."
echo "Stop reason: $STOP_REASON"
if [[ -n "$STOP_REASON_DETAILS" ]]; then
  echo "Details: $STOP_REASON_DETAILS"
fi
echo "Completed iterations: $LAST_COMPLETED_ITERATION"
echo "Accepted baseline version: $ACCEPTED_BASELINE_VERSION"
echo "Accepted baseline ref: $ACCEPTED_BASELINE_REF"
echo "Session summary written to: $SESSION_SUMMARY_PATH"
