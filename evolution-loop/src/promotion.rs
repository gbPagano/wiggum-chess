use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::state::{
    load_iteration_state, save_iteration_state, DecisionState, PromotionState, SessionMetadata,
};

/// Returns the directory for a given baseline version.
pub fn baseline_version_path(repo_root: &Path, version: &str) -> PathBuf {
    repo_root.join("chess-engine").join("versions").join(version)
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
    let dest_dir_str = baseline_version_path(repo_root, &promoted_version)
        .to_string_lossy()
        .to_string();

    // Update session metadata active/accepted baseline fields
    session_meta.active_baseline_version = promoted_version.clone();
    session_meta.active_baseline_path = dest_dir_str.clone();
    session_meta.active_baseline_binary = dest_str.clone();
    session_meta.accepted_baseline_version = promoted_version.clone();
    session_meta.accepted_baseline_path = dest_dir_str.clone();
    session_meta.accepted_baseline_binary = dest_str.clone();

    // Update iteration.json decision with promotion fields
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
