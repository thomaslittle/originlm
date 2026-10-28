//! Explicit host-owned endpoint configuration. No credential is serialized or logged.
use originlm_adapters::http_provider::config::Registry;
use originlm_contracts::*;
use std::{io::Read, path::Path};
pub fn load(path: Option<&Path>) -> Result<Registry> {
    let Some(path) = path else {
        return Ok(Registry::default());
    };
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
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    // Keep the endpoint config path private; use an opaque scope for stable IDs.
    let scope = stable_id("endpoint-source", &[&path.to_string_lossy()]);
    Registry::parse(text, &scope)
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Endpoint configuration could not be loaded",
    )
}
