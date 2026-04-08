use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

// ---------------------------------------------------------------------------
// Session metadata (written as key=value to session.env)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub baseline_version: String,
    pub active_baseline_version: String,
    pub active_baseline_path: String,
    pub active_baseline_binary: String,
    pub accepted_baseline_version: String,
    pub accepted_baseline_path: String,
    pub accepted_baseline_binary: String,
    pub accepted_baseline_major: String,
    pub accepted_baseline_minor: String,
    pub candidate_version: String,
    pub candidate_binary_path: String,
    pub ideas_file: String,
    pub ideas_file_pending_count: String,
    pub ideas_format: String,
    pub stockfish_binary: String,
    pub max_iterations: String,
    pub max_infra_failures: String,
    pub session_id: String,
    pub session_dir: String,
    pub summary_file: String,
}

pub fn load_session_metadata(path: &Path) -> Result<SessionMetadata> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading session metadata from {}", path.display()))?;
    let mut map: HashMap<String, String> = HashMap::new();
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let get = |k: &str| map.get(k).cloned().unwrap_or_default();
    Ok(SessionMetadata {
        baseline_version: get("baseline_version"),
        active_baseline_version: get("active_baseline_version"),
        active_baseline_path: get("active_baseline_path"),
        active_baseline_binary: get("active_baseline_binary"),
        accepted_baseline_version: get("accepted_baseline_version"),
        accepted_baseline_path: get("accepted_baseline_path"),
        accepted_baseline_binary: get("accepted_baseline_binary"),
        accepted_baseline_major: get("accepted_baseline_major"),
        accepted_baseline_minor: get("accepted_baseline_minor"),
        candidate_version: get("candidate_version"),
        candidate_binary_path: get("candidate_binary_path"),
        ideas_file: get("ideas_file"),
        ideas_file_pending_count: get("ideas_file_pending_count"),
        ideas_format: get("ideas_format"),
        stockfish_binary: get("stockfish_binary"),
        max_iterations: get("max_iterations"),
        max_infra_failures: get("max_infra_failures"),
        session_id: get("session_id"),
        session_dir: get("session_dir"),
        summary_file: get("summary_file"),
    })
}

pub fn save_session_metadata(path: &Path, meta: &SessionMetadata) -> Result<()> {
    let mut f = fs::File::create(path)
        .with_context(|| format!("creating session metadata file {}", path.display()))?;
    macro_rules! write_field {
        ($name:ident) => {
            writeln!(f, "{}={}", stringify!($name), meta.$name)?;
        };
    }
    write_field!(baseline_version);
    write_field!(active_baseline_version);
    write_field!(active_baseline_path);
    write_field!(active_baseline_binary);
    write_field!(accepted_baseline_version);
    write_field!(accepted_baseline_path);
    write_field!(accepted_baseline_binary);
    write_field!(accepted_baseline_major);
    write_field!(accepted_baseline_minor);
    write_field!(candidate_version);
    write_field!(candidate_binary_path);
    write_field!(ideas_file);
    write_field!(ideas_file_pending_count);
    write_field!(ideas_format);
    write_field!(stockfish_binary);
    write_field!(max_iterations);
    write_field!(max_infra_failures);
    write_field!(session_id);
    write_field!(session_dir);
    write_field!(summary_file);
    Ok(())
}

// ---------------------------------------------------------------------------
// Iteration state (iteration.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IterationPhase {
    Initialized,
    Proposing,
    Proposed,
    Implementing,
    Implemented,
    Validating,
    Benchmarking,
    Benchmarked,
    Deciding,
    Accepted,
    Rejected,
    Inconclusive,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeasState {
    pub proposal_source: Option<String>,
    pub selected_idea: Option<String>,
    pub selected_idea_marked_used: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateState {
    pub version: Option<String>,
    pub binary_path: Option<String>,
    pub workspace: Option<String>,
    pub branch: Option<String>,
    pub setup_status: Option<String>,
    pub setup_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolationState {
    pub worktree: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub name: String,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectnessState {
    pub status: String,
    pub passed: bool,
    pub benchmark_eligible: bool,
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockfishComparisonState {
    pub baseline_report_available: Option<bool>,
    pub recommendation_changed: Option<bool>,
    pub limitation: Option<String>,
    pub positive_signal: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateMachineState {
    pub current: IterationPhase,
    pub transitions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsState {
    pub hypothesis: Option<String>,
    pub implementation: Option<String>,
    pub correctness_results: Option<String>,
    pub benchmark: Option<String>,
    pub stockfish_comparison: Option<String>,
    pub decision: Option<String>,
    pub phase_logs: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionState {
    pub outcome: String,
    pub evidence: Option<String>,
    pub promotion: Option<PromotionState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionState {
    pub promoted_version: Option<String>,
    pub artifact_path: Option<String>,
    pub artifact_binary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IterationState {
    pub iteration: u32,
    pub baseline_version: String,
    pub baseline_path: String,
    pub baseline_binary: String,
    pub ideas: IdeasState,
    pub candidate: CandidateState,
    pub state: IterationPhase,
    pub isolation: IsolationState,
    pub correctness: CorrectnessState,
    pub stockfish_comparison: StockfishComparisonState,
    pub state_machine: StateMachineState,
    pub artifacts: ArtifactsState,
    pub decision: Option<DecisionState>,
}

pub fn transition_phase(
    state: &mut IterationState,
    from: IterationPhase,
    to: IterationPhase,
) -> Result<()> {
    if state.state != from {
        bail!(
            "expected iteration phase {:?}, but current phase is {:?}",
            from,
            state.state
        );
    }
    state.state = to;
    Ok(())
}

pub fn load_iteration_state(path: &Path) -> Result<IterationState> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading iteration state from {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("parsing iteration state from {}", path.display()))
}

pub fn save_iteration_state(path: &Path, state: &IterationState) -> Result<()> {
    let json = serde_json::to_string_pretty(state)
        .context("serializing iteration state")?;
    let mut f = fs::File::create(path)
        .with_context(|| format!("creating iteration state file {}", path.display()))?;
    writeln!(f, "{}", json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_iteration_state() -> IterationState {
        IterationState {
            iteration: 1,
            baseline_version: "v0.1".to_string(),
            baseline_path: "/some/path".to_string(),
            baseline_binary: "/some/path/wiggum-engine".to_string(),
            ideas: IdeasState {
                proposal_source: Some("self_proposed".to_string()),
                selected_idea: Some("Improve move ordering".to_string()),
                selected_idea_marked_used: Some(false),
            },
            candidate: CandidateState {
                version: Some("v0.2".to_string()),
                binary_path: Some("/tmp/wiggum-engine".to_string()),
                workspace: Some("/tmp/candidate".to_string()),
                branch: Some("wiggum-evolution/sess/iteration-1".to_string()),
                setup_status: Some("ok".to_string()),
                setup_error: None,
            },
            state: IterationPhase::Initialized,
            isolation: IsolationState {
                worktree: Some("/tmp/candidate".to_string()),
                branch: Some("wiggum-evolution/sess/iteration-1".to_string()),
            },
            correctness: CorrectnessState {
                status: "pending".to_string(),
                passed: false,
                benchmark_eligible: false,
                checks: vec![],
            },
            stockfish_comparison: StockfishComparisonState {
                baseline_report_available: None,
                recommendation_changed: None,
                limitation: None,
                positive_signal: None,
            },
            state_machine: StateMachineState {
                current: IterationPhase::Initialized,
                transitions: vec![],
            },
            artifacts: ArtifactsState {
                hypothesis: Some("/iter/hypothesis.md".to_string()),
                implementation: Some("/iter/implementation.md".to_string()),
                correctness_results: None,
                benchmark: None,
                stockfish_comparison: None,
                decision: None,
                phase_logs: Some(HashMap::new()),
            },
            decision: None,
        }
    }

    #[test]
    fn round_trip_iteration_state() {
        let original = sample_iteration_state();
        let json = serde_json::to_string_pretty(&original).unwrap();
        let restored: IterationState = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string_pretty(&restored).unwrap();
        assert_eq!(json, json2, "round-trip JSON must be identical");
    }

    #[test]
    fn transition_phase_wrong_from_returns_err() {
        let mut state = sample_iteration_state();
        // state is Initialized; transition from Proposing should fail
        let result = transition_phase(&mut state, IterationPhase::Proposing, IterationPhase::Proposed);
        assert!(result.is_err(), "expected Err when from != current state");
    }

    #[test]
    fn transition_phase_correct_from_succeeds() {
        let mut state = sample_iteration_state();
        transition_phase(&mut state, IterationPhase::Initialized, IterationPhase::Proposing).unwrap();
        assert_eq!(state.state, IterationPhase::Proposing);
    }
}
