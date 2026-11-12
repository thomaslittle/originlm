//! Version extraction from `--version` output and installer answers, and
//! comparison. An unparsable version is `None`, never a guess.
use std::cmp::Ordering;

fn is_version_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')
}

/// The first `MAJOR.MINOR.PATCH[-pre][+build]` token, with an optional `v`.
pub fn extract(text: &str) -> Option<String> {
    for word in text.split(|c: char| !is_version_char(c)) {
        let word = word.strip_prefix('v').unwrap_or(word);
        let word = word.strip_prefix("rust-v").unwrap_or(word);
        let mut core = word.splitn(2, ['-', '+']);
        let head = core.next().unwrap_or("");
        let parts: Vec<&str> = head.split('.').collect();
        if parts.len() >= 3
            && parts[..3]
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        {
            // Keep only MAJOR.MINOR.PATCH plus any pre-release/build suffix.
            let tail = &word[head.len()..];
            let tail = tail.trim_end_matches(['.', '-', '+']);
            return Some(format!("{}.{}.{}{}", parts[0], parts[1], parts[2], tail));
        }
    }
    None
}

fn semver(version: &str) -> Option<semver::Version> {
    semver::Version::parse(version.trim().trim_start_matches('v')).ok()
}

/// Muse uses `X.Y.Z-R<n>[.<m>]` for releases; a revision is newer, not a pre-release.
fn muse_key(version: &str) -> Option<(semver::Version, u64, u64)> {
    let (base, revision) = match version.split_once("-R") {
        Some((base, revision)) => (base, Some(revision)),
        None => (version, None),
    };
    let base = semver(base)?;
    let (major, minor) = match revision {
        None => (0, 0),
        Some(revision) => {
            let mut parts = revision.splitn(2, '.');
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next().map(str::parse).transpose().ok()?.unwrap_or(0);
            (major, minor)
        }
    };
    Some((base, major, minor))
}

/// `None` when either side is not a comparable version.
pub fn compare(provider_id: &str, current: &str, latest: &str) -> Option<Ordering> {
    if provider_id == "muse" {
        // `--version` can omit the revision the channel reports; then only the base counts.
        if current.contains("-R") != latest.contains("-R") {
            let base = |version: &str| version.split("-R").next().unwrap_or(version).to_owned();
            return Some(semver(&base(current))?.cmp(&semver(&base(latest))?));
        }
        let (current, latest) = (muse_key(current)?, muse_key(latest)?);
        return Some(current.cmp(&latest));
    }
    Some(semver(current)?.cmp(&semver(latest)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_versions_from_real_output_shapes() {
        assert_eq!(extract("2.1.37 (Claude Code)").as_deref(), Some("2.1.37"));
        assert_eq!(extract("codex-cli 0.58.0").as_deref(), Some("0.58.0"));
        assert_eq!(extract("rust-v0.60.1").as_deref(), Some("0.60.1"));
        assert_eq!(extract("v1.4.0-beta.2\n").as_deref(), Some("1.4.0-beta.2"));
        assert_eq!(extract("muse 1.2.3-R4.1").as_deref(), Some("1.2.3-R4.1"));
        assert_eq!(extract("grok version 0.1.150.").as_deref(), Some("0.1.150"));
        assert_eq!(extract("no version here 1.2"), None);
        assert_eq!(extract(""), None);
    }

    #[test]
    fn compares_semver_and_refuses_garbage() {
        assert_eq!(compare("codex", "0.58.0", "0.60.1"), Some(Ordering::Less));
        assert_eq!(compare("codex", "1.0.0", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(
            compare("codex", "1.0.0-beta.1", "1.0.0"),
            Some(Ordering::Less)
        );
        assert_eq!(compare("codex", "latest", "1.0.0"), None);
    }

    #[test]
    fn muse_revisions_are_releases() {
        assert_eq!(
            compare("muse", "1.2.3-R4", "1.2.3-R5"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare("muse", "1.2.3-R4.2", "1.2.3-R4.10"),
            Some(Ordering::Less)
        );
        // A revision on only one side compares the base versions.
        assert_eq!(compare("muse", "1.2.3", "1.2.3-R1"), Some(Ordering::Equal));
        assert_eq!(
            compare("muse", "1.4.14", "1.4.18-R8587.1"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare("muse", "1.2.4-R1", "1.2.3-R9"),
            Some(Ordering::Greater)
        );
    }
}
