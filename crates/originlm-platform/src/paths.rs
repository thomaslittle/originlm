use originlm_contracts::{DomainError, ErrorCode, Result};
use std::path::{Component, Path, PathBuf};

pub fn child_path(root: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.len() > 255
        || name.contains(['/', char::from(92), ':', char::from(0)])
        || !matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
    {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Expected one safe path component",
        ));
    }
    Ok(root.join(name))
}

/// Resolve platform state storage without creating directories or reading files.
pub fn default_state_dir(home: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    if let Some(home) = home {
        return Some(home.join(".originlm"));
    }
    let var = |name: &str| {
        std::env::var_os(name)
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    if let Some(dir) = var("ORIGINLM_STATE_DIR").or_else(|| var("LOCAL_AI_KIT_STATE_DIR")) {
        return Some(dir);
    }
    if cfg!(windows) {
        var("LOCALAPPDATA").map(|p| p.join("originlm"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|p| p.join("Library/Application Support/originlm"))
    } else {
        var("XDG_STATE_HOME")
            .or_else(|| var("HOME").map(|p| p.join(".local/state")))
            .map(|p| p.join("originlm"))
    }
}
