//! Explicit host-owned native-client registration. The file contains paths and labels, never tokens.
use originlm_adapters::{
    billing_declarations::BillingDeclarations, native_clients::config::NativeRegistry,
};
use originlm_contracts::*;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
pub fn load(path: Option<&Path>) -> Result<NativeRegistry> {
    let Some(path) = path else {
        return Ok(NativeRegistry::default());
    };
    let text = read_bounded(path)?;
    let scope = stable_id("native-profile-source", &[&path.to_string_lossy()]);
    NativeRegistry::parse(&text, &scope)
}
/// Host billing declarations (`--billing-declarations`); user-supplied evidence only.
pub fn load_declarations(path: Option<&Path>) -> Result<BillingDeclarations> {
    let Some(path) = path else {
        return Ok(BillingDeclarations::default());
    };
    BillingDeclarations::parse(&read_bounded(path)?)
}
fn read_bounded(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).map_err(|_| invalid())?;
    if !file.metadata().map_err(|_| invalid())?.is_file() {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    String::from_utf8(bytes).map_err(|_| invalid())
}
pub fn packaged_browser_blocker() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    let name = if cfg!(windows) {
        "originlm-auth-blocker.exe"
    } else {
        "originlm-auth-blocker"
    };
    let candidate = parent.join(name);
    candidate
        .metadata()
        .ok()
        .filter(|m| m.is_file())
        .map(|_| candidate)
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Native profile configuration could not be loaded",
    )
}
