use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalSource {
    SelfProposed,
    UserIdeasFile,
}

pub fn parse_version_tag(tag: &str) -> Result<(u32, u32)> {
    let stripped = tag
        .strip_prefix('v')
        .ok_or_else(|| anyhow!("version tag must start with 'v', got '{}'", tag))?;
    let (major_str, minor_str) = stripped
        .split_once('.')
        .ok_or_else(|| anyhow!("version tag must be vMAJOR.MINOR, got '{}'", tag))?;

    if major_str.is_empty() || minor_str.is_empty() || minor_str.contains('.') {
        bail!("version tag must be vMAJOR.MINOR, got '{}'", tag);
    }

    let major = major_str
        .parse::<u32>()
        .with_context(|| format!("invalid major version '{}' in tag '{}'", major_str, tag))?;
    let minor = minor_str
        .parse::<u32>()
        .with_context(|| format!("invalid minor version '{}' in tag '{}'", minor_str, tag))?;

    Ok((major, minor))
}

pub fn candidate_version_for_source(baseline: &str, source: ProposalSource) -> Result<String> {
    let (major, minor) = parse_version_tag(baseline)?;
    match source {
        ProposalSource::SelfProposed => Ok(format!("v{}.{}", major, minor + 1)),
        ProposalSource::UserIdeasFile => Ok(format!("v{}.0", major + 1)),
    }
}

pub fn cargo_semver_from_tag(tag: &str) -> Result<String> {
    let (major, minor) = parse_version_tag(tag)?;
    Ok(format!("{}.{}.0", major, minor))
}

fn update_manifest_version(manifest_path: &Path, semver: &str) -> Result<()> {
    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("reading manifest {}", manifest_path.display()))?;

    let mut updated_lines = Vec::new();
    let mut in_package = false;
    let mut replaced = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = trimmed == "[package]";
        }

        if in_package && trimmed.starts_with("version = \"") {
            updated_lines.push(format!("version = \"{}\"", semver));
            replaced = true;
            in_package = false;
            continue;
        }

        updated_lines.push(line.to_string());
    }

    if !replaced {
        bail!(
            "no package-level 'version = \"...\"' found in {}",
            manifest_path.display()
        );
    }

    let updated = format!("{}\n", updated_lines.join("\n"));
    fs::write(manifest_path, updated)
        .with_context(|| format!("writing manifest {}", manifest_path.display()))?;
    Ok(())
}

pub fn apply_candidate_manifest_versions(workspace: &Path, semver: &str) -> Result<()> {
    for manifest in [
        workspace.join("chess-engine/Cargo.toml"),
        workspace.join("chess-runner/Cargo.toml"),
        workspace.join("chesslib/Cargo.toml"),
    ] {
        update_manifest_version(&manifest, semver)
            .with_context(|| format!("updating version in {}", manifest.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_manifest(path: &Path, version: &str) {
        fs::write(
            path,
            format!(
                "[package]\nname = \"test\"\nversion = \"{}\"\nedition = \"2024\"\n",
                version
            ),
        )
        .unwrap();
    }

    #[test]
    fn parse_version_tag_examples() {
        assert_eq!(parse_version_tag("v1.2").unwrap(), (1, 2));
        assert!(parse_version_tag("bad").is_err());
    }

    #[test]
    fn candidate_version_for_source_examples() {
        assert_eq!(
            candidate_version_for_source("v1.2", ProposalSource::SelfProposed).unwrap(),
            "v1.3"
        );
        assert_eq!(
            candidate_version_for_source("v1.2", ProposalSource::UserIdeasFile).unwrap(),
            "v2.0"
        );
    }

    #[test]
    fn cargo_semver_from_tag_example() {
        assert_eq!(cargo_semver_from_tag("v1.2").unwrap(), "1.2.0");
    }

    #[test]
    fn apply_candidate_manifest_versions_updates_all_three_manifests() {
        let dir = tempdir().unwrap();
        let chess_engine = dir.path().join("chess-engine");
        let chess_runner = dir.path().join("chess-runner");
        let chesslib = dir.path().join("chesslib");
        fs::create_dir_all(&chess_engine).unwrap();
        fs::create_dir_all(&chess_runner).unwrap();
        fs::create_dir_all(&chesslib).unwrap();

        write_manifest(&chess_engine.join("Cargo.toml"), "0.1.0");
        write_manifest(&chess_runner.join("Cargo.toml"), "0.1.0");
        write_manifest(&chesslib.join("Cargo.toml"), "0.1.0");

        apply_candidate_manifest_versions(dir.path(), "1.2.0").unwrap();

        for manifest in [
            chess_engine.join("Cargo.toml"),
            chess_runner.join("Cargo.toml"),
            chesslib.join("Cargo.toml"),
        ] {
            let content = fs::read_to_string(manifest).unwrap();
            assert!(content.contains("version = \"1.2.0\""));
        }
    }

    #[test]
    fn apply_candidate_manifest_versions_updates_only_package_version_line() {
        let dir = tempdir().unwrap();
        let chess_engine = dir.path().join("chess-engine");
        let chess_runner = dir.path().join("chess-runner");
        let chesslib = dir.path().join("chesslib");
        fs::create_dir_all(&chess_engine).unwrap();
        fs::create_dir_all(&chess_runner).unwrap();
        fs::create_dir_all(&chesslib).unwrap();

        fs::write(
            chess_engine.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = { version = \"0.1.0\" }\n",
        )
        .unwrap();
        write_manifest(&chess_runner.join("Cargo.toml"), "0.1.0");
        write_manifest(&chesslib.join("Cargo.toml"), "0.1.0");

        apply_candidate_manifest_versions(dir.path(), "1.2.0").unwrap();

        let content = fs::read_to_string(chess_engine.join("Cargo.toml")).unwrap();
        assert!(content.contains("version = \"1.2.0\""));
        assert!(content.contains("serde = { version = \"0.1.0\" }"));
    }

    #[test]
    fn apply_candidate_manifest_versions_errors_when_manifest_has_no_version() {
        let dir = tempdir().unwrap();
        let chess_engine = dir.path().join("chess-engine");
        let chess_runner = dir.path().join("chess-runner");
        let chesslib = dir.path().join("chesslib");
        fs::create_dir_all(&chess_engine).unwrap();
        fs::create_dir_all(&chess_runner).unwrap();
        fs::create_dir_all(&chesslib).unwrap();

        fs::write(
            chess_engine.join("Cargo.toml"),
            "[package]\nname = \"test\"\nedition = \"2024\"\n",
        )
        .unwrap();
        write_manifest(&chess_runner.join("Cargo.toml"), "0.1.0");
        write_manifest(&chesslib.join("Cargo.toml"), "0.1.0");

        let error = apply_candidate_manifest_versions(dir.path(), "1.2.0")
            .unwrap_err()
            .to_string();
        assert!(error.contains("chess-engine/Cargo.toml"));
    }
}
