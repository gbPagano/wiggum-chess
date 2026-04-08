use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::state::{
    load_iteration_state, save_iteration_state, DecisionState, PromotionState, SessionMetadata,
};
use crate::versioning::parse_version_tag;

/// Returns the directory for a given baseline version.
pub fn baseline_version_path(repo_root: &Path, version: &str) -> PathBuf {
    repo_root
        .join("chess-engine")
        .join("versions")
        .join(version)
}

/// Copy the candidate binary to the versions directory and set executable bit.
pub fn persist_promoted_artifact(
    candidate_binary: &Path,
    promoted_version: &str,
    repo_root: &Path,
) -> Result<PathBuf> {
    if !candidate_binary.exists() {
        bail!(
            "candidate binary does not exist at {}",
            candidate_binary.display()
        );
    }

    let dest_dir = baseline_version_path(repo_root, promoted_version);
    fs::create_dir_all(&dest_dir)
        .with_context(|| format!("creating version directory {}", dest_dir.display()))?;

    let dest = dest_dir.join("wiggum-engine");
    fs::copy(candidate_binary, &dest)
        .with_context(|| format!("copying binary to {}", dest.display()))?;

    // Set executable bit on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        fs::set_permissions(&dest, perms)?;
    }

    Ok(dest)
}

/// Promote the candidate: copy binary to versions dir, update session metadata
/// and iteration.json with promotion fields.
pub fn promote_candidate(
    _candidate_dir: &Path,
    iteration_state_path: &Path,
    session_meta: &mut SessionMetadata,
    repo_root: &Path,
) -> Result<()> {
    let mut iter_state = load_iteration_state(iteration_state_path)?;

    let promoted_version = iter_state
        .candidate
        .version
        .clone()
        .ok_or_else(|| anyhow::anyhow!("iteration.json missing candidate.version"))?;
    let candidate_binary_str = iter_state
        .candidate
        .binary_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("iteration.json missing candidate.binaryPath"))?;
    let candidate_binary = Path::new(&candidate_binary_str);

    let dest = persist_promoted_artifact(candidate_binary, &promoted_version, repo_root)?;
    let dest_str = dest.to_string_lossy().to_string();
    let dest_dir = baseline_version_path(repo_root, &promoted_version);
    let dest_dir_str = dest_dir.to_string_lossy().to_string();
    let (major, minor) = parse_version_tag(&promoted_version)?;

    session_meta.active_baseline_version = promoted_version.clone();
    session_meta.active_baseline_path = dest_dir_str.clone();
    session_meta.active_baseline_binary = dest_str.clone();
    session_meta.accepted_baseline_version = promoted_version.clone();
    session_meta.accepted_baseline_path = dest_dir_str.clone();
    session_meta.accepted_baseline_binary = dest_str.clone();
    session_meta.accepted_baseline_major = major.to_string();
    session_meta.accepted_baseline_minor = minor.to_string();

    let decision = iter_state.decision.get_or_insert_with(|| DecisionState {
        outcome: "accepted".to_string(),
        evidence: None,
        promotion: None,
    });
    decision.promotion = Some(PromotionState {
        promoted_version: Some(promoted_version.clone()),
        artifact_path: Some(dest_dir_str),
        artifact_binary: Some(dest_str),
    });

    save_iteration_state(iteration_state_path, &iter_state)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        ArtifactsState, CandidateState, CorrectnessState, IdeasState, IsolationState,
        IterationPhase, IterationState, SessionMetadata, StateMachineState,
        StockfishComparisonState,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn sample_iteration_state(candidate_binary: &Path, candidate_version: &str) -> IterationState {
        IterationState {
            iteration: 1,
            baseline_version: "v0.1".to_string(),
            baseline_path: "/baseline".to_string(),
            baseline_binary: "/baseline/wiggum-engine".to_string(),
            ideas: IdeasState {
                proposal_source: Some("self_proposed".to_string()),
                selected_idea: Some("Improve move ordering".to_string()),
                selected_idea_marked_used: Some(false),
            },
            candidate: CandidateState {
                version: Some(candidate_version.to_string()),
                binary_path: Some(candidate_binary.to_string_lossy().to_string()),
                workspace: Some("/candidate".to_string()),
                branch: Some("wiggum-evolution/session/iteration-1".to_string()),
                setup_status: Some("ok".to_string()),
                setup_error: None,
            },
            state: IterationPhase::Accepted,
            isolation: IsolationState {
                worktree: Some("/candidate".to_string()),
                branch: Some("wiggum-evolution/session/iteration-1".to_string()),
            },
            correctness: CorrectnessState {
                status: "passed".to_string(),
                passed: true,
                benchmark_eligible: true,
                checks: vec![],
            },
            stockfish_comparison: StockfishComparisonState {
                baseline_report_available: Some(false),
                recommendation_changed: None,
                limitation: None,
                positive_signal: None,
            },
            state_machine: StateMachineState {
                current: IterationPhase::Accepted,
                transitions: vec![],
            },
            artifacts: ArtifactsState {
                hypothesis: None,
                implementation: None,
                correctness_results: None,
                benchmark: None,
                stockfish_comparison: None,
                decision: None,
                phase_logs: Some(HashMap::new()),
            },
            decision: Some(DecisionState {
                outcome: "accepted".to_string(),
                evidence: Some("strong benchmark".to_string()),
                promotion: None,
            }),
        }
    }

    #[test]
    fn baseline_version_path_joins_repo_and_version() {
        let repo_root = Path::new("/workspace");
        assert_eq!(
            baseline_version_path(repo_root, "v1.2"),
            PathBuf::from("/workspace/chess-engine/versions/v1.2")
        );
    }

    #[test]
    fn persist_promoted_artifact_copies_binary_and_sets_executable_bit() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path();
        let candidate_binary = repo_root.join("candidate/wiggum-engine");
        fs::create_dir_all(candidate_binary.parent().unwrap()).unwrap();
        fs::write(&candidate_binary, b"engine-binary").unwrap();

        let dest = persist_promoted_artifact(&candidate_binary, "v1.2", repo_root).unwrap();

        assert_eq!(
            dest,
            repo_root.join("chess-engine/versions/v1.2/wiggum-engine")
        );
        assert_eq!(fs::read(&dest).unwrap(), b"engine-binary");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&dest).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0);
        }
    }

    #[test]
    fn promote_candidate_updates_session_metadata_and_iteration_promotion() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path();
        let candidate_binary = repo_root.join("candidate/target/debug/wiggum-engine");
        fs::create_dir_all(candidate_binary.parent().unwrap()).unwrap();
        fs::write(&candidate_binary, b"candidate-binary").unwrap();

        let iteration_state_path = repo_root.join("iteration.json");
        save_iteration_state(
            &iteration_state_path,
            &sample_iteration_state(&candidate_binary, "v2.3"),
        )
        .unwrap();

        let mut session_meta = SessionMetadata::default();
        promote_candidate(
            repo_root,
            &iteration_state_path,
            &mut session_meta,
            repo_root,
        )
        .unwrap();

        let promoted_dir = repo_root.join("chess-engine/versions/v2.3");
        let promoted_binary = promoted_dir.join("wiggum-engine");
        assert_eq!(session_meta.active_baseline_version, "v2.3");
        assert_eq!(
            session_meta.active_baseline_path,
            promoted_dir.to_string_lossy()
        );
        assert_eq!(
            session_meta.active_baseline_binary,
            promoted_binary.to_string_lossy()
        );
        assert_eq!(session_meta.accepted_baseline_version, "v2.3");
        assert_eq!(
            session_meta.accepted_baseline_path,
            promoted_dir.to_string_lossy()
        );
        assert_eq!(
            session_meta.accepted_baseline_binary,
            promoted_binary.to_string_lossy()
        );
        assert_eq!(session_meta.accepted_baseline_major, "2");
        assert_eq!(session_meta.accepted_baseline_minor, "3");

        let iteration_state = load_iteration_state(&iteration_state_path).unwrap();
        let promotion = iteration_state
            .decision
            .as_ref()
            .and_then(|decision| decision.promotion.as_ref())
            .expect("promotion metadata should be present");
        assert_eq!(promotion.promoted_version.as_deref(), Some("v2.3"));
        assert_eq!(
            promotion.artifact_path.as_deref(),
            Some(promoted_dir.to_string_lossy().as_ref())
        );
        assert_eq!(
            promotion.artifact_binary.as_deref(),
            Some(promoted_binary.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn promote_candidate_errors_when_candidate_binary_is_missing() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path();
        let candidate_binary = repo_root.join("candidate/target/debug/wiggum-engine");
        let iteration_state_path = repo_root.join("iteration.json");
        save_iteration_state(
            &iteration_state_path,
            &sample_iteration_state(&candidate_binary, "v2.3"),
        )
        .unwrap();

        let mut session_meta = SessionMetadata::default();
        let error = promote_candidate(
            repo_root,
            &iteration_state_path,
            &mut session_meta,
            repo_root,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("candidate binary does not exist at"));
    }
}
