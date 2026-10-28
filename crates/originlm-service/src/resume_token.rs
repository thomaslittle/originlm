//! Durable, host-storable resume tokens.
//!
//! A token is `lai1_` + base64url(nonce || ChaCha20-Poly1305(payload)). The key
//! is 32 random bytes the kit creates once per install in its state directory
//! (`resume.key`, user-only where the platform allows). Tokens carry no
//! credentials; sealing also keeps the native session id, account id and
//! workspace path out of host storage, and makes a token from another install
//! or a modified token unusable. Nothing is read or created until a durable
//! token is first sealed or opened.
use base64::Engine;
use originlm_contracts::*;
use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(crate) const PREFIX: &str = "lai1_";
const KEY_FILE: &str = "resume.key";
const MAX_TOKEN: usize = 8192;

/// Everything needed to revalidate and reattach a suspended conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DurableResume {
    pub v: u8,
    pub owner: String,
    pub connection_id: String,
    pub account_id: Option<String>,
    pub route_id: String,
    pub model_id: String,
    pub options: BTreeMap<String, serde_json::Value>,
    pub workspace_id: String,
    pub workspace_path: PathBuf,
    pub permission_mode: Option<String>,
    pub review_requests: bool,
    pub allow_paid: bool,
    pub billing: Option<BillingMode>,
    pub native_session_id: String,
    pub issued_at: u64,
}

/// Lazily loads (or creates) the per-install resume key.
pub struct ResumeStore {
    dir: PathBuf,
    key: Mutex<Option<[u8; 32]>>,
}
fn unavailable() -> DomainError {
    DomainError::new(
        ErrorCode::ResumeUnavailable,
        "Resume token is not valid for this installation",
    )
}
fn store_error() -> DomainError {
    DomainError::new(
        ErrorCode::ProviderUnavailable,
        "Resume key storage is unavailable",
    )
}
fn read_key(path: &Path) -> std::io::Result<Option<[u8; 32]>> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
        Ok(meta) if !meta.is_file() => {
            return Err(std::io::Error::other("resume key is not a regular file"));
        }
        Ok(_) => {}
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(33)
        .read_to_end(&mut bytes)?;
    <[u8; 32]>::try_from(bytes.as_slice())
        .map(Some)
        .map_err(|_| std::io::Error::other("resume key has the wrong length"))
}
fn create_key(path: &Path) -> std::io::Result<()> {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|_| std::io::Error::other("no randomness"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&key)?;
    file.sync_all()
}
impl ResumeStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            key: Mutex::new(None),
        }
    }
    fn key(&self, create: bool) -> Result<Option<LessSafeKey>> {
        let mut cached = self.key.lock().map_err(|_| store_error())?;
        if cached.is_none() {
            let path = self.dir.join(KEY_FILE);
            let mut loaded = read_key(&path).map_err(|_| store_error())?;
            if loaded.is_none() && create {
                std::fs::create_dir_all(&self.dir).map_err(|_| store_error())?;
                match create_key(&path) {
                    Ok(()) => {}
                    // Another helper created it first; use that key.
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return Err(store_error()),
                }
                loaded = read_key(&path).map_err(|_| store_error())?;
            }
            *cached = loaded;
        }
        cached
            .map(|bytes| {
                UnboundKey::new(&CHACHA20_POLY1305, &bytes)
                    .map(LessSafeKey::new)
                    .map_err(|_| store_error())
            })
            .transpose()
    }
    /// Loads or creates the key; call before an irreversible suspend.
    pub(crate) fn ready(&self) -> Result<()> {
        self.key(true)?.map(|_| ()).ok_or_else(store_error)
    }
    pub(crate) fn seal(&self, payload: &DurableResume) -> Result<String> {
        let key = self.key(true)?.ok_or_else(store_error)?;
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|_| store_error())?;
        let mut data = serde_json::to_vec(payload).map_err(|_| store_error())?;
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(PREFIX.as_bytes()),
            &mut data,
        )
        .map_err(|_| store_error())?;
        let mut sealed = nonce.to_vec();
        sealed.extend_from_slice(&data);
        let token = format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sealed)
        );
        if token.len() > MAX_TOKEN {
            return Err(store_error());
        }
        Ok(token)
    }
    pub(crate) fn open(&self, token: &str) -> Result<DurableResume> {
        let body = token
            .strip_prefix(PREFIX)
            .filter(|_| token.len() <= MAX_TOKEN)
            .ok_or_else(|| DomainError::new(ErrorCode::InvalidRequest, "Invalid resume token"))?;
        let mut sealed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| DomainError::new(ErrorCode::InvalidRequest, "Invalid resume token"))?;
        if sealed.len() <= NONCE_LEN {
            return Err(unavailable());
        }
        // No key yet means no token was ever sealed by this installation.
        let key = self.key(false)?.ok_or_else(unavailable)?;
        let mut data = sealed.split_off(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(&sealed).map_err(|_| unavailable())?;
        let plain = key
            .open_in_place(nonce, Aad::from(PREFIX.as_bytes()), &mut data)
            .map_err(|_| unavailable())?;
        let payload: DurableResume = serde_json::from_slice(plain).map_err(|_| unavailable())?;
        if payload.v != 1 {
            return Err(unavailable());
        }
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload() -> DurableResume {
        DurableResume {
            v: 1,
            owner: "app".into(),
            connection_id: "c".into(),
            account_id: Some("person@example.test".into()),
            route_id: "r".into(),
            model_id: "m".into(),
            options: BTreeMap::new(),
            workspace_id: "w".into(),
            workspace_path: PathBuf::from("/private/workspace"),
            permission_mode: None,
            review_requests: false,
            allow_paid: false,
            billing: Some(BillingMode::Subscription),
            native_session_id: "native-secret-session".into(),
            issued_at: 1,
        }
    }
    #[test]
    fn tokens_round_trip_across_store_instances_and_hide_contents() {
        let dir = tempfile::tempdir().unwrap();
        let token = ResumeStore::new(dir.path()).seal(&payload()).unwrap();
        assert!(token.starts_with(PREFIX));
        for hidden in [
            "native-secret-session",
            "person@example",
            "private",
            "workspace",
        ] {
            assert!(!token.contains(hidden));
        }
        // A new store (a restarted helper) reads the persisted key.
        assert_eq!(
            ResumeStore::new(dir.path()).open(&token).unwrap(),
            payload()
        );
        let key = std::fs::read(dir.path().join(KEY_FILE)).unwrap();
        assert_eq!(key.len(), 32);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(KEY_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
    #[test]
    fn tampered_or_foreign_tokens_are_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let store = ResumeStore::new(dir.path());
        let token = store.seal(&payload()).unwrap();
        let mut tampered = token.clone().into_bytes();
        let last = tampered.len() - 5;
        tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(tampered).unwrap();
        assert_eq!(
            store.open(&tampered).unwrap_err().code,
            ErrorCode::ResumeUnavailable
        );
        let other = tempfile::tempdir().unwrap();
        let foreign = ResumeStore::new(other.path());
        assert_eq!(
            foreign.open(&token).unwrap_err().code,
            ErrorCode::ResumeUnavailable
        );
        // Opening never creates a key.
        assert!(!other.path().join(KEY_FILE).exists());
        foreign.seal(&payload()).unwrap();
        assert_eq!(
            foreign.open(&token).unwrap_err().code,
            ErrorCode::ResumeUnavailable
        );
        assert_eq!(
            store.open("lai1_!!").unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
}
