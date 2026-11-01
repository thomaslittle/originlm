use crate::{Result, error};
use serde::{Deserialize, Serialize};
#[cfg(all(test, windows))]
use std::sync::{Mutex, MutexGuard};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(all(test, windows))]
static WINDOWS_KEYRING_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(all(test, windows))]
pub(crate) fn lock_keyring_for_test() -> MutexGuard<'static, ()> {
    WINDOWS_KEYRING_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TokenRecord {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: u64,
    /// Unix seconds from the token response `earliest_refresh_at`. Absent means
    /// the provider did not ask us to wait.
    #[serde(default)]
    pub earliest_refresh_at: Option<u64>,
    pub scopes: Vec<String>,
}
impl std::fmt::Debug for TokenRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRecord")
            .field("tokens", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Registration {
    pub client_id: String,
    pub subject: String,
    pub email: Option<String>,
    /// `Some(false)` means the last sign-in omitted `chatgpt.tokens.use.direct`.
    /// The next attempt asks for consent again. Older files leave this unset.
    #[serde(default)]
    pub sharing_enabled: Option<bool>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StateFile {
    pub host_id: String,
    #[serde(default)]
    pub epoch: u64,
    pub registration: Option<Registration>,
    #[cfg(windows)]
    #[serde(default)]
    pub credential_active: bool,
    #[cfg(unix)]
    pub tokens: Option<TokenRecord>,
}
impl StateFile {
    pub fn fresh() -> Self {
        Self {
            host_id: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
            epoch: 0,
            registration: None,
            #[cfg(windows)]
            credential_active: false,
            #[cfg(unix)]
            tokens: None,
        }
    }
}
pub(crate) struct Store {
    root: PathBuf,
}
fn storage_error() -> crate::AuthError {
    error(
        "STORAGE_UNAVAILABLE",
        "ChatGPT credential storage is unavailable",
    )
}
#[cfg(windows)]
fn credential_unusable() -> crate::AuthError {
    error(
        "CREDENTIAL_UNUSABLE",
        "ChatGPT credential record cannot be used",
    )
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub async fn lock_refresh(&self) -> Result<fs::File> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            secure_dir(&root)?;
            let file = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(root.join("chatgpt-refresh.lock"))
                .map_err(|_| storage_error())?;
            fs4::fs_std::FileExt::lock_exclusive(&file).map_err(|_| storage_error())?;
            Ok(file)
        })
        .await
        .map_err(|_| storage_error())?
    }
    pub fn load(&self) -> Result<StateFile> {
        self.load_or(&StateFile::fresh())
    }
    pub fn load_or(&self, fallback: &StateFile) -> Result<StateFile> {
        let path = self.root.join("chatgpt-auth.json");
        if let Ok(meta) = path.symlink_metadata() {
            if !meta.file_type().is_file() {
                return Err(storage_error());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o077 != 0 {
                    return Err(storage_error());
                }
            }
        }
        match fs::read(&path) {
            Ok(bytes) if bytes.len() <= 64 * 1024 => {
                let state: StateFile =
                    serde_json::from_slice(&bytes).map_err(|_| storage_error())?;
                validate_state(&state)?;
                Ok(state)
            }
            Ok(_) => Err(storage_error()),
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && fallback.epoch == 0
                    && fallback.registration.is_none() =>
            {
                Ok(fallback.clone())
            }
            Err(_) => Err(storage_error()),
        }
    }
    pub fn save(&self, state: &StateFile) -> Result<()> {
        secure_dir(&self.root)?;
        let mut tmp = tempfile::Builder::new()
            .prefix(".chatgpt-auth-")
            .tempfile_in(&self.root)
            .map_err(|_| storage_error())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())?;
        }
        serde_json::to_writer(tmp.as_file_mut(), state).map_err(|_| storage_error())?;
        tmp.as_file_mut().flush().map_err(|_| storage_error())?;
        tmp.as_file().sync_all().map_err(|_| storage_error())?;
        tmp.persist(self.root.join("chatgpt-auth.json"))
            .map_err(|_| storage_error())?;
        Ok(())
    }
    #[cfg(unix)]
    pub fn load_tokens(&self, state: &StateFile) -> Result<Option<TokenRecord>> {
        Ok(state.tokens.clone())
    }
    #[cfg(unix)]
    pub fn save_tokens(&self, state: &mut StateFile, tokens: Option<TokenRecord>) -> Result<()> {
        state.tokens = tokens;
        self.save(state)
    }
    #[cfg(windows)]
    pub fn load_tokens(&self, state: &StateFile) -> Result<Option<TokenRecord>> {
        #[cfg(test)]
        let _keyring_guard = lock_keyring_for_test();
        let Some(reg) = &state.registration else {
            return Ok(None);
        };
        if !state.credential_active {
            return Ok(None);
        }
        let path = credential_path(&self.root, &state.host_id, reg);
        let sealed = match fs::read(&path) {
            Ok(data) if data.len() <= 64 * 1024 => data,
            Ok(_) => return Err(credential_unusable()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(credential_unusable()),
            Err(_) => return Err(storage_error()),
        };
        if sealed.len() < 12 + 16 {
            return Err(credential_unusable());
        }
        let entry = keyring_entry(&state.host_id, reg)?;
        let key = match entry.get_password() {
            Ok(key) => key,
            Err(keyring::Error::NoEntry) => return Err(credential_unusable()),
            Err(_) => return Err(storage_error()),
        };
        let key = decode_key(&key).map_err(|_| credential_unusable())?;
        let aead = ring::aead::LessSafeKey::new(
            ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
                .map_err(|_| credential_unusable())?,
        );
        let mut body = sealed[12..].to_vec();
        let plain = aead
            .open_in_place(
                ring::aead::Nonce::try_assume_unique_for_key(&sealed[..12])
                    .map_err(|_| credential_unusable())?,
                ring::aead::Aad::from(reg_key(&state.host_id, reg).as_bytes()),
                &mut body,
            )
            .map_err(|_| credential_unusable())?;
        serde_json::from_slice(plain)
            .map(Some)
            .map_err(|_| credential_unusable())
    }
    #[cfg(windows)]
    pub fn save_tokens(&self, state: &mut StateFile, tokens: Option<TokenRecord>) -> Result<()> {
        #[cfg(test)]
        let _keyring_guard = lock_keyring_for_test();
        let Some(reg) = state.registration.clone() else {
            if tokens.is_some() {
                return Err(storage_error());
            }
            state.credential_active = false;
            return self.save(state);
        };
        let path = credential_path(&self.root, &state.host_id, &reg);
        match tokens {
            Some(tokens) => {
                let entry = keyring_entry(&state.host_id, &reg)?;
                let key = match entry.get_password() {
                    Ok(value) => decode_key(&value)?,
                    Err(keyring::Error::NoEntry) => {
                        let mut bytes = [0u8; 32];
                        getrandom::fill(&mut bytes).map_err(|_| storage_error())?;
                        use base64::Engine as _;
                        entry
                            .set_password(
                                &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
                            )
                            .map_err(|_| storage_error())?;
                        bytes
                    }
                    Err(_) => return Err(storage_error()),
                };
                let aead = ring::aead::LessSafeKey::new(
                    ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
                        .map_err(|_| storage_error())?,
                );
                let mut nonce = [0u8; 12];
                getrandom::fill(&mut nonce).map_err(|_| storage_error())?;
                let mut body = serde_json::to_vec(&tokens).map_err(|_| storage_error())?;
                aead.seal_in_place_append_tag(
                    ring::aead::Nonce::assume_unique_for_key(nonce),
                    ring::aead::Aad::from(reg_key(&state.host_id, &reg).as_bytes()),
                    &mut body,
                )
                .map_err(|_| storage_error())?;
                let mut sealed = nonce.to_vec();
                sealed.extend(body);
                secure_dir(&self.root)?;
                let mut tmp = tempfile::Builder::new()
                    .prefix(".chatgpt-credentials-")
                    .tempfile_in(&self.root)
                    .map_err(|_| storage_error())?;
                tmp.write_all(&sealed).map_err(|_| storage_error())?;
                tmp.as_file().sync_all().map_err(|_| storage_error())?;
                tmp.persist(path).map_err(|_| storage_error())?;
                state.credential_active = true;
                self.save(state)
            }
            None => {
                state.credential_active = false;
                self.save(state)?; // Invalidate pending callbacks before deleting the ciphertext.
                match fs::remove_file(path) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(_) => return Err(storage_error()),
                }
                if let Ok(entry) = keyring_entry(&state.host_id, &reg) {
                    let _ = entry.delete_credential(); // An orphan key cannot decrypt a deleted file.
                }
                Ok(())
            }
        }
    }
}
fn validate_state(state: &StateFile) -> Result<()> {
    let host = state
        .host_id
        .strip_prefix("urn:uuid:")
        .ok_or_else(storage_error)?;
    uuid::Uuid::parse_str(host).map_err(|_| storage_error())?;
    if let Some(reg) = &state.registration {
        if !reg.client_id.starts_with("oaiapp_")
            || reg.client_id.len() > 512
            || reg.subject.is_empty()
            || reg.subject.len() > 1024
            || reg.email.as_ref().is_some_and(|email| email.len() > 1024)
        {
            return Err(storage_error());
        }
    } else {
        #[cfg(windows)]
        if state.credential_active {
            return Err(storage_error());
        }
        #[cfg(unix)]
        if state.tokens.is_some() {
            return Err(storage_error());
        }
    }
    Ok(())
}
#[cfg(windows)]
pub(crate) fn keyring_entry(host_id: &str, reg: &Registration) -> Result<keyring::Entry> {
    keyring::Entry::new("originlm.chatgpt", &reg_key(host_id, reg)).map_err(|_| storage_error())
}
#[cfg(windows)]
fn reg_key(host_id: &str, reg: &Registration) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{}\0{}\0{}", host_id, reg.client_id, reg.subject));
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
#[cfg(windows)]
pub(crate) fn credential_path(root: &Path, host_id: &str, reg: &Registration) -> PathBuf {
    root.join(format!("chatgpt-{}.credentials", reg_key(host_id, reg)))
}
#[cfg(windows)]
fn decode_key(value: &str) -> Result<[u8; 32]> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| storage_error())?;
    bytes.try_into().map_err(|_| storage_error())
}
fn secure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|_| storage_error())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| storage_error())?;
    }
    Ok(())
}
