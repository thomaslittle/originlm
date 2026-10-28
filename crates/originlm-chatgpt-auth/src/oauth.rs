use crate::store::{Registration, StateFile, Store, TokenRecord};
use crate::{Result, error, has_plan_scope};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{
    Algorithm, DecodingKey, Validation, decode, decode_header,
    jwk::{AlgorithmParameters, EllipticCurve, JwkSet},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    path::PathBuf,
    sync::{
        Arc, Mutex as StdMutex, Weak,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
    time::{Duration, Instant},
};
use url::Url;

const ISSUER: &str = "https://auth.openai.com";
const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
/// ID tokens from the pinned T3 Code ChatGPT flow (`CodexChatGptAuth.ts`).
const IDENTITY_ALGS: &[&str] = &["RS256", "ES256"];
const IDENTITY_LEEWAY_SECONDS: u64 = 5;
/// Refresh when the access token is inside this many seconds of expiry.
const REFRESH_SKEW_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    SignedOut,
    Pending,
    Connected,
    Error,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub state: AuthState,
    pub login_id: Option<String>,
    pub email: Option<String>,
    pub error_code: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStart {
    pub login_id: String,
    pub auth_url: String,
}
impl std::fmt::Debug for AuthStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthStart")
            .field("login_id", &self.login_id)
            .field("auth_url", &"[redacted]")
            .finish()
    }
}
#[derive(Clone)]
pub struct Credentials {
    pub access_token: String,
    pub email: Option<String>,
    pub subject: String,
    pub client_id: String,
    pub host_id: String,
    pub expires_at: u64,
    pub scopes: Vec<String>,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_token", &"[redacted]")
            .field("email", &self.email)
            .field("subject", &self.subject)
            .field("client_id", &self.client_id)
            .field("host_id", &self.host_id)
            .field("expires_at", &self.expires_at)
            .field("scopes", &self.scopes)
            .finish()
    }
}
#[derive(Clone)]
struct Endpoints {
    issuer: String,
    authorization: String,
    token: String,
    jwks: String,
    revocation: String,
}
#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    revocation_endpoint: String,
}
impl Endpoints {
    fn checked(meta: Discovery, allow_local: bool) -> Result<Self> {
        let trusted = |raw: &str| -> bool {
            let Ok(url) = Url::parse(raw) else {
                return false;
            };
            if !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return false;
            }
            (url.scheme() == "https"
                && url.host_str() == Some("auth.openai.com")
                && url.port().is_none())
                || (allow_local && url.scheme() == "http" && url.host_str() == Some("127.0.0.1"))
        };
        if meta.issuer != ISSUER && !(allow_local && trusted(&meta.issuer)) {
            return Err(error("DISCOVERY_INVALID", "ChatGPT issuer is invalid"));
        }
        if ![
            &meta.authorization_endpoint,
            &meta.token_endpoint,
            &meta.jwks_uri,
            &meta.revocation_endpoint,
        ]
        .iter()
        .all(|u| trusted(u))
        {
            return Err(error(
                "DISCOVERY_INVALID",
                "ChatGPT OAuth endpoint is invalid",
            ));
        }
        if !allow_local
            && (meta.authorization_endpoint != "https://auth.openai.com/api/accounts/authorize"
                || meta.token_endpoint != "https://auth.openai.com/api/accounts/oauth/token"
                || meta.revocation_endpoint != "https://auth.openai.com/api/accounts/oauth/revoke")
        {
            return Err(error("DISCOVERY_INVALID", "ChatGPT OAuth endpoint changed"));
        }
        Ok(Self {
            issuer: meta.issuer,
            authorization: meta.authorization_endpoint,
            token: meta.token_endpoint,
            jwks: meta.jwks_uri,
            revocation: meta.revocation_endpoint,
        })
    }
}
struct Pending {
    login_id: String,
    generation: u64,
    epoch: u64,
    state: String,
    nonce: String,
    verifier: String,
    redirect_uri: String,
    client_id: Option<String>,
    prior_subject: Option<String>,
}
struct State {
    loaded: bool,
    file: StateFile,
    tokens: Option<TokenRecord>,
    pending: Option<Pending>,
    error_code: Option<String>,
    generation: u64,
}
struct Inner {
    app_name: String,
    store: Store,
    http: reqwest::Client,
    state: Mutex<State>,
    refresh: Mutex<()>,
    listener: StdMutex<Option<JoinHandle<()>>>,
    owners: AtomicUsize,
    #[cfg(test)]
    endpoints: Option<Endpoints>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(slot) = self.listener.get_mut()
            && let Some(task) = slot.take()
        {
            task.abort();
        }
    }
}
pub struct AuthManager {
    inner: Arc<Inner>,
    owned: bool,
}
impl Clone for AuthManager {
    fn clone(&self) -> Self {
        self.inner.owners.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: self.inner.clone(),
            owned: true,
        }
    }
}
impl Drop for AuthManager {
    fn drop(&mut self) {
        if self.owned
            && self.inner.owners.fetch_sub(1, Ordering::AcqRel) == 1
            && let Ok(mut slot) = self.inner.listener.lock()
            && let Some(task) = slot.take()
        {
            task.abort();
        }
    }
}
impl AuthManager {
    pub fn new(root: PathBuf, app_name: String) -> Result<Self> {
        if app_name.trim().is_empty() || app_name.len() > 100 {
            return Err(error("INVALID_REQUEST", "ChatGPT app name is invalid"));
        }
        let store = Store::new(root);
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| error("NETWORK_UNAVAILABLE", "ChatGPT HTTP client unavailable"))?;
        Ok(Self {
            inner: Arc::new(Inner {
                app_name,
                store,
                http,
                state: Mutex::new(State {
                    loaded: false,
                    file: StateFile::fresh(),
                    tokens: None,
                    pending: None,
                    error_code: None,
                    generation: 0,
                }),
                refresh: Mutex::new(()),
                listener: StdMutex::new(None),
                owners: AtomicUsize::new(1),
                #[cfg(test)]
                endpoints: None,
            }),
            owned: true,
        })
    }
    async fn ensure_loaded(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        if !state.loaded {
            let file = self.inner.store.load()?;
            let tokens = match self.inner.store.load_tokens(&file) {
                Ok(tokens) => tokens,
                Err(problem) => {
                    state.error_code = Some(problem.code.into());
                    None
                }
            };
            state.file = file;
            state.tokens = tokens;
            state.loaded = true;
        }
        Ok(())
    }
    async fn endpoints(&self) -> Result<Endpoints> {
        #[cfg(test)]
        if let Some(endpoints) = &self.inner.endpoints {
            return Ok(endpoints.clone());
        }
        let response = self
            .inner
            .http
            .get("https://auth.openai.com/.well-known/openid-configuration")
            .send()
            .await
            .map_err(|_| network_error())?
            .error_for_status()
            .map_err(|_| network_error())?;
        let meta: Discovery = bounded_json(response, 32 * 1024).await?;
        Endpoints::checked(meta, false)
    }
    pub async fn status(&self) -> Result<AuthStatus> {
        self.ensure_loaded().await?;
        let mut state = self.inner.state.lock().await;
        // These are atomic records; reading them never waits for another
        // process's refresh lock or its remote token request.
        let latest = self.inner.store.load_or(&state.file)?;
        if state.file.host_id != latest.host_id || state.file.registration != latest.registration {
            return Err(error("IDENTITY_CHANGED", "ChatGPT registration changed"));
        }
        let epoch_changed = latest.epoch != state.file.epoch;
        if epoch_changed {
            state.generation += 1;
            state.pending = None;
            state.error_code = None;
            if let Ok(mut slot) = self.inner.listener.lock()
                && let Some(task) = slot.take()
            {
                task.abort();
            }
        }
        let (latest_tokens, unusable) = match self.inner.store.load_tokens(&latest) {
            Ok(tokens) => (tokens, false),
            Err(problem) if problem.code == "CREDENTIAL_UNUSABLE" => {
                state.error_code = Some(problem.code.into());
                (None, true)
            }
            Err(problem) => return Err(problem),
        };
        if !unusable && state.tokens != latest_tokens {
            state.error_code = None;
        }
        state.file = latest;
        state.tokens = latest_tokens;
        let current = if state.pending.is_some() {
            AuthState::Pending
        } else if state.error_code.is_some() {
            AuthState::Error
        } else if state.tokens.is_some() {
            AuthState::Connected
        } else {
            AuthState::SignedOut
        };
        Ok(AuthStatus {
            state: current,
            login_id: state.pending.as_ref().map(|p| p.login_id.clone()),
            email: state
                .file
                .registration
                .as_ref()
                .and_then(|r| r.email.clone()),
            error_code: state.error_code.clone(),
        })
    }
    pub async fn start(&self) -> Result<AuthStart> {
        self.ensure_loaded().await?;
        let endpoints = self.endpoints().await?;
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|_| {
            error(
                "LOOPBACK_UNAVAILABLE",
                "ChatGPT callback listener unavailable",
            )
        })?;
        let port = listener
            .local_addr()
            .map_err(|_| {
                error(
                    "LOOPBACK_UNAVAILABLE",
                    "ChatGPT callback listener unavailable",
                )
            })?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}/auth/callback");
        let _file_lock = self.inner.store.lock_refresh().await?;
        let mut state = self.inner.state.lock().await;
        if state.pending.is_some() {
            return Err(error("LOGIN_PENDING", "ChatGPT sign-in is already pending"));
        }
        let file = self.inner.store.load_or(&state.file)?;
        if state.file.host_id != file.host_id || state.file.registration != file.registration {
            return Err(error("IDENTITY_CHANGED", "ChatGPT registration changed"));
        }
        let tokens = match self.inner.store.load_tokens(&file) {
            Ok(tokens) => tokens,
            Err(problem) if problem.code == "CREDENTIAL_UNUSABLE" => {
                // Only the known credential record is unusable. Keep the verified
                // registration and host identity for a fresh OAuth attempt.
                state.file = file;
                self.inner.store.save_tokens(&mut state.file, None)?;
                None
            }
            Err(problem) => return Err(problem),
        };
        let file = self.inner.store.load_or(&state.file)?;
        state.file = file;
        state.tokens = tokens;
        if state.tokens.is_some() {
            return Err(error(
                "ALREADY_CONNECTED",
                "ChatGPT account is already connected",
            ));
        }
        state.file.epoch = state
            .file
            .epoch
            .checked_add(1)
            .ok_or_else(|| error("STORAGE_UNAVAILABLE", "ChatGPT auth epoch exhausted"))?;
        self.inner.store.save(&state.file)?; // Host ID and attempt epoch survive before browser sign-in.
        let login_id = random_urlsafe(18)?;
        let csrf_state = random_urlsafe(32)?;
        let nonce = random_urlsafe(32)?;
        let verifier = random_urlsafe(32)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let prior = state.file.registration.clone();
        let client_id = prior.as_ref().map(|r| r.client_id.clone());
        let mut url = Url::parse(&endpoints.authorization).map_err(|_| network_error())?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair(
                "client_id",
                client_id.as_deref().unwrap_or("dynamic_agent_client"),
            );
            if client_id.is_none() {
                q.append_pair("agent_name_hint", &self.inner.app_name);
            }
            q.append_pair("ext_agent_host_id", &state.file.host_id);
            if let Some(r) = &prior {
                if let Some(email) = &r.email {
                    q.append_pair("login_hint", email);
                }
                // A previous grant without plan sharing will not show the
                // consent screen again unless we ask. T3 Code does the same
                // when `sharingEnabled` is false.
                if r.sharing_enabled == Some(false) {
                    q.append_pair("prompt", "consent");
                }
            }
            q.append_pair("response_type", "code");
            q.append_pair("redirect_uri", &redirect_uri);
            q.append_pair("scope", SCOPES);
            q.append_pair("resource", RESOURCE);
            q.append_pair("state", &csrf_state);
            q.append_pair("nonce", &nonce);
            q.append_pair("code_challenge_method", "S256");
            q.append_pair("code_challenge", &challenge);
        }
        state.generation += 1;
        let generation = state.generation;
        state.error_code = None;
        state.pending = Some(Pending {
            login_id: login_id.clone(),
            generation,
            epoch: state.file.epoch,
            state: csrf_state,
            nonce,
            verifier,
            redirect_uri,
            client_id,
            prior_subject: prior.map(|r| r.subject),
        });
        drop(state);
        if let Ok(mut slot) = self.inner.listener.lock() {
            if let Some(task) = slot.take() {
                task.abort();
            }
            let weak = Arc::downgrade(&self.inner);
            *slot = Some(tokio::spawn(async move {
                callback_loop(weak, listener, endpoints, generation).await;
            }));
        }
        Ok(AuthStart {
            login_id,
            auth_url: url.into(),
        })
    }
    pub async fn cancel(&self, login_id: &str) -> Result<()> {
        self.ensure_loaded().await?;
        let mut state = self.inner.state.lock().await;
        if state.pending.as_ref().map(|p| p.login_id.as_str()) != Some(login_id) {
            return Err(error("LOGIN_NOT_FOUND", "ChatGPT sign-in is not pending"));
        }
        state.generation += 1;
        state.pending = None;
        state.error_code = None;
        if let Ok(mut slot) = self.inner.listener.lock()
            && let Some(task) = slot.take()
        {
            task.abort();
        }
        Ok(())
    }
    pub async fn disconnect(&self) -> Result<()> {
        self.disconnect_with(|| async {}).await
    }
    pub async fn disconnect_with<F, Fut>(&self, after_local_clear: F) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        self.ensure_loaded().await?;
        let _refresh = self.inner.refresh.lock().await;
        let (tokens, client_id) = {
            let _file_lock = self.inner.store.lock_refresh().await?;
            let mut state = self.inner.state.lock().await;
            let latest = self.inner.store.load_or(&state.file)?;
            if state.file.host_id != latest.host_id
                || state.file.registration != latest.registration
            {
                return Err(error("IDENTITY_CHANGED", "ChatGPT registration changed"));
            }
            state.file = latest;
            state.generation += 1;
            state.pending = None;
            state.error_code = None;
            state.tokens = self.inner.store.load_tokens(&state.file).unwrap_or(None);
            let tokens = state.tokens.clone();
            state.file.epoch = state
                .file
                .epoch
                .checked_add(1)
                .ok_or_else(|| error("STORAGE_UNAVAILABLE", "ChatGPT auth epoch exhausted"))?;
            self.inner.store.save_tokens(&mut state.file, None)?;
            state.tokens = None;
            let client_id = state
                .file
                .registration
                .as_ref()
                .map(|r| r.client_id.clone());
            if let Ok(mut slot) = self.inner.listener.lock()
                && let Some(task) = slot.take()
            {
                task.abort();
            }
            (tokens, client_id)
        };
        after_local_clear().await;
        if let (Some(tokens), Some(client_id)) = (tokens, client_id) {
            // Local sign-out is already durable. Bound the best-effort remote
            // cleanup so the host can finish disconnecting within its RPC budget.
            let revocation = async {
                let endpoints = self.endpoints().await.map_err(|_| {
                    error(
                        "REVOCATION_UNCONFIRMED",
                        "ChatGPT signed out locally; remote revocation was not confirmed",
                    )
                })?;
                for delay in [0, 200, 500] {
                    if delay != 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    let reply = self
                        .inner
                        .http
                        .post(&endpoints.revocation)
                        .form(&[
                            ("token", tokens.refresh_token.as_str()),
                            ("token_type_hint", "refresh_token"),
                            ("client_id", client_id.as_str()),
                        ])
                        .send()
                        .await;
                    match reply {
                        Ok(response) if response.status().is_success() => return Ok(()),
                        Ok(response) if response.status().is_server_error() => continue,
                        Err(_) => continue,
                        Ok(_) => break,
                    }
                }
                Err(error(
                    "REVOCATION_UNCONFIRMED",
                    "ChatGPT signed out locally; remote revocation was not confirmed",
                ))
            };
            return tokio::time::timeout(Duration::from_secs(10), revocation)
                .await
                .unwrap_or_else(|_| {
                    Err(error(
                        "REVOCATION_UNCONFIRMED",
                        "ChatGPT signed out locally; remote revocation was not confirmed",
                    ))
                });
        }
        Ok(())
    }
    pub async fn credentials(&self) -> Result<Credentials> {
        self.ensure_loaded().await?;
        let _refresh = self.inner.refresh.lock().await;
        let _file_lock = self.inner.store.lock_refresh().await?;
        let (tokens, reg, host_id, generation) = {
            let mut state = self.inner.state.lock().await;
            let latest = self.inner.store.load_or(&state.file)?;
            if state.file.host_id != latest.host_id
                || state.file.registration != latest.registration
            {
                state.tokens = None;
                return Err(error("IDENTITY_CHANGED", "ChatGPT registration changed"));
            }
            state.file = latest;
            state.tokens = self.inner.store.load_tokens(&state.file)?;
            let tokens = state
                .tokens
                .clone()
                .ok_or_else(|| error("NOT_CONNECTED", "ChatGPT account is not connected"))?;
            let reg = state
                .file
                .registration
                .clone()
                .ok_or_else(|| error("NOT_CONNECTED", "ChatGPT account is not connected"))?;
            (tokens, reg, state.file.host_id.clone(), state.generation)
        };
        let tokens = if tokens.expires_at > now().saturating_add(REFRESH_SKEW_SECONDS) {
            tokens
        } else if tokens
            .earliest_refresh_at
            .is_some_and(|earliest| earliest > now())
        {
            // The provider asked us not to rotate yet. A still-valid access
            // token is reused; an expired one waits instead of racing refresh.
            if tokens.expires_at > now() {
                tokens
            } else {
                return Err(error(
                    "REFRESH_DEFERRED",
                    "ChatGPT cannot renew this connection yet",
                ));
            }
        } else {
            let endpoints = self.endpoints().await?;
            let reply = self
                .inner
                .http
                .post(&endpoints.token)
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("client_id", reg.client_id.as_str()),
                    ("refresh_token", tokens.refresh_token.as_str()),
                    ("resource", RESOURCE),
                ])
                .send()
                .await
                .map_err(|_| network_error())?
                .error_for_status()
                .map_err(|_| error("REFRESH_FAILED", "ChatGPT session must be renewed"))?;
            let response: TokenResponse = bounded_json(reply, 64 * 1024)
                .await
                .map_err(|_| error("REFRESH_FAILED", "ChatGPT refresh response is invalid"))?;
            let replacement = token_record(response, Some(&tokens))?;
            if let Some(id) = &replacement.1 {
                let identity =
                    validate_id(&self.inner.http, &endpoints, id, &reg.client_id, None).await?;
                if identity.sub != reg.subject {
                    return Err(error(
                        "IDENTITY_CHANGED",
                        "ChatGPT account identity changed",
                    ));
                }
            }
            let mut state = self.inner.state.lock().await;
            if state.generation != generation || state.tokens.is_none() {
                return Err(error("NOT_CONNECTED", "ChatGPT account was disconnected"));
            }
            self.inner
                .store
                .save_tokens(&mut state.file, Some(replacement.0.clone()))?;
            state.tokens = Some(replacement.0.clone());
            replacement.0
        };
        Ok(Credentials {
            access_token: tokens.access_token,
            email: reg.email,
            subject: reg.subject,
            client_id: reg.client_id,
            host_id,
            expires_at: tokens.expires_at,
            scopes: tokens.scopes,
        })
    }
}
fn network_error() -> crate::AuthError {
    error(
        "NETWORK_UNAVAILABLE",
        "ChatGPT authentication service unavailable",
    )
}
async fn bounded_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
    max: usize,
) -> Result<T> {
    if response
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err(network_error());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| network_error())? {
        if body.len().saturating_add(chunk.len()) > max {
            return Err(network_error());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| network_error())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_urlsafe(size: usize) -> Result<String> {
    let mut bytes = vec![0; size];
    getrandom::fill(&mut bytes)
        .map_err(|_| error("RANDOM_UNAVAILABLE", "Secure randomness unavailable"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: u64,
    scope: Option<String>,
    token_type: Option<String>,
    /// Seconds since epoch, or an RFC3339 timestamp. Unparseable values are ignored.
    #[serde(default)]
    earliest_refresh_at: Option<serde_json::Value>,
}
fn parse_earliest(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?;
    let seconds = match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text
            .parse::<u64>()
            .ok()
            .or_else(|| parse_rfc3339_seconds(text)),
        _ => None,
    };
    seconds.filter(|seconds| *seconds > 0)
}
fn parse_rfc3339_seconds(text: &str) -> Option<u64> {
    let (date, rest) = text.split_once('T')?;
    let mut dates = date.split('-');
    let year: i32 = dates.next()?.parse().ok()?;
    let month: u32 = dates.next()?.parse().ok()?;
    let day: u32 = dates.next()?.parse().ok()?;
    if dates.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (time, offset_seconds) = if let Some(time) = rest.strip_suffix('Z') {
        (time, 0i32)
    } else {
        let split_at = rest.rfind(['+', '-'])?;
        let (time, marked) = rest.split_at(split_at);
        let sign = if marked.starts_with('+') { 1 } else { -1 };
        let mut offset_parts = marked[1..].split(':');
        let offset_hour: i32 = offset_parts.next()?.parse().ok()?;
        let offset_minute: i32 = offset_parts.next()?.parse().ok()?;
        if offset_parts.next().is_some() || offset_hour > 23 || offset_minute > 59 {
            return None;
        }
        (time, sign * (offset_hour * 3_600 + offset_minute * 60))
    };
    let mut times = time.split(':');
    let hour: u32 = times.next()?.parse().ok()?;
    let minute: u32 = times.next()?.parse().ok()?;
    let second_text = times.next()?;
    if times.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    let second: u32 = second_text.split('.').next()?.parse().ok()?;
    if second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    let unix = days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second)
        - i64::from(offset_seconds);
    u64::try_from(unix).ok()
}
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = u64::try_from(y - era * 400).ok()?;
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * shifted as u64 + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + i64::try_from(doe).ok()? - 719_468)
}
fn token_record(
    response: TokenResponse,
    prior: Option<&TokenRecord>,
) -> Result<(TokenRecord, Option<String>)> {
    if response.access_token.is_empty()
        || !response
            .token_type
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case("bearer"))
        || response.expires_in == 0
    {
        return Err(error("TOKEN_INVALID", "ChatGPT token response is invalid"));
    }
    let refresh_token = response
        .refresh_token
        .or_else(|| prior.map(|p| p.refresh_token.clone()))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("TOKEN_INVALID", "ChatGPT renewable session was not granted"))?;
    let scope = response
        .scope
        .unwrap_or_else(|| prior.map(|p| p.scopes.join(" ")).unwrap_or_default());
    if !has_plan_scope(&scope) {
        return Err(error(
            "PLAN_SCOPE_MISSING",
            "ChatGPT plan permission was not granted",
        ));
    }
    let id_token = response
        .id_token
        .or_else(|| prior.map(|p| p.id_token.clone()))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("TOKEN_INVALID", "ChatGPT identity token is missing"))?;
    let new_id = if prior.is_none() || prior.is_some_and(|p| p.id_token != id_token) {
        Some(id_token.clone())
    } else {
        None
    };
    Ok((
        TokenRecord {
            access_token: response.access_token,
            refresh_token,
            id_token,
            expires_at: now().saturating_add(response.expires_in),
            earliest_refresh_at: parse_earliest(response.earliest_refresh_at.as_ref()),
            scopes: scope.split_ascii_whitespace().map(str::to_owned).collect(),
        },
        new_id,
    ))
}
#[derive(Debug, Deserialize)]
struct Identity {
    sub: String,
    nonce: Option<String>,
    email: Option<String>,
}
async fn validate_id(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    id: &str,
    client_id: &str,
    nonce: Option<&str>,
) -> Result<Identity> {
    let header = decode_header(id)
        .map_err(|_| error("IDENTITY_INVALID", "ChatGPT identity token is invalid"))?;
    let kid = header
        .kid
        .ok_or_else(|| error("IDENTITY_INVALID", "ChatGPT identity key is missing"))?;
    let reply = http
        .get(&endpoints.jwks)
        .send()
        .await
        .map_err(|_| network_error())?
        .error_for_status()
        .map_err(|_| network_error())?;
    let keys: JwkSet = bounded_json(reply, 256 * 1024).await?;
    let key = keys.find(&kid).ok_or_else(|| {
        error(
            "IDENTITY_INVALID",
            "ChatGPT identity signing key is unknown",
        )
    })?;
    let algorithm = match (header.alg, &key.algorithm) {
        (Algorithm::RS256, AlgorithmParameters::RSA(_)) if IDENTITY_ALGS.contains(&"RS256") => {
            Algorithm::RS256
        }
        (Algorithm::ES256, AlgorithmParameters::EllipticCurve(curve))
            if IDENTITY_ALGS.contains(&"ES256") && curve.curve == EllipticCurve::P256 =>
        {
            Algorithm::ES256
        }
        _ => {
            return Err(error(
                "IDENTITY_INVALID",
                "ChatGPT identity signature algorithm is unsupported",
            ));
        }
    };
    let decoding = DecodingKey::from_jwk(key).map_err(|_| {
        error(
            "IDENTITY_INVALID",
            "ChatGPT identity signing key is invalid",
        )
    })?;
    let mut validation = Validation::new(algorithm);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.leeway = IDENTITY_LEEWAY_SECONDS;
    validation.set_issuer(&[&endpoints.issuer]);
    validation.set_audience(&[client_id]);
    let claims = decode::<Identity>(id, &decoding, &validation)
        .map_err(|_| {
            error(
                "IDENTITY_INVALID",
                "ChatGPT identity token did not validate",
            )
        })?
        .claims;
    if claims.sub.is_empty()
        || nonce.is_some_and(|expected| claims.nonce.as_deref() != Some(expected))
    {
        return Err(error(
            "IDENTITY_INVALID",
            "ChatGPT identity does not match sign-in",
        ));
    }
    Ok(claims)
}
async fn callback_loop(
    weak: Weak<Inner>,
    listener: TcpListener,
    endpoints: Endpoints,
    generation: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        let accepted = tokio::time::timeout_at(deadline, listener.accept()).await;
        let Ok(Ok((mut stream, _))) = accepted else {
            break;
        };
        let Some(request) = read_callback_request(&mut stream).await else {
            continue;
        };
        let callback = parse_callback(&request);
        let Some(inner) = weak.upgrade() else { break };
        let Some(callback) = callback else {
            let _ = stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            continue;
        };
        let valid_state = {
            let state = inner.state.lock().await;
            state.generation == generation
                && state
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.state == callback.state)
        };
        if !valid_state {
            let _ = stream
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            continue;
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: 36\r\nConnection: close\r\n\r\nSign-in received. Return to the app.").await;
        let manager = AuthManager {
            inner,
            owned: false,
        };
        let result = manager
            .complete(callback, endpoints.clone(), generation)
            .await;
        if let Err(e) = result {
            let mut state = manager.inner.state.lock().await;
            if state.generation == generation {
                state.pending = None;
                state.error_code = Some(e.code.to_owned());
            }
        }
        return;
    }
    if let Some(inner) = weak.upgrade() {
        let mut state = inner.state.lock().await;
        if state.generation == generation {
            state.pending = None;
            state.error_code = Some("LOGIN_TIMEOUT".into());
        }
    }
}
async fn read_callback_request(stream: &mut tokio::net::TcpStream) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut data = Vec::with_capacity(1024);
    while data.len() < 8192 {
        let mut chunk = [0u8; 1024];
        let remaining = (8192 - data.len()).min(chunk.len());
        let size = tokio::time::timeout_at(deadline, stream.read(&mut chunk[..remaining]))
            .await
            .ok()?
            .ok()?;
        if size == 0 {
            return None;
        }
        data.extend_from_slice(&chunk[..size]);
        if data.windows(4).any(|w| w == b"\r\n\r\n") {
            return String::from_utf8(data).ok();
        }
    }
    None
}
struct Callback {
    state: String,
    code: Option<String>,
    client_id: Option<String>,
    error: Option<String>,
}
fn parse_callback(request: &str) -> Option<Callback> {
    let first = request.lines().next()?;
    let path = first.strip_prefix("GET ")?.split(' ').next()?;
    let url = Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    if url.path() != "/auth/callback" || url.fragment().is_some() {
        return None;
    }
    let mut state = None;
    let mut code = None;
    let mut client_id = None;
    let mut error = None;
    for (key, value) in url.query_pairs() {
        let target = match key.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "client_id" => &mut client_id,
            "error" => &mut error,
            _ => continue,
        };
        if target.replace(value.into_owned()).is_some() {
            return None;
        }
    }
    Some(Callback {
        state: state?,
        code,
        client_id,
        error,
    })
}
struct SharingAttempt<'a> {
    client_id: &'a str,
    nonce: &'a str,
    prior_subject: Option<&'a str>,
    generation: u64,
    epoch: u64,
}
impl AuthManager {
    /// Persist the client registration after a sign-in that omitted plan sharing,
    /// without storing tokens. The next `start` sends `prompt=consent`.
    async fn remember_sharing_disabled(
        &self,
        response: &TokenResponse,
        endpoints: &Endpoints,
        attempt: SharingAttempt<'_>,
    ) -> Result<()> {
        if !response
            .token_type
            .as_deref()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("bearer"))
        {
            return Err(error("TOKEN_INVALID", "ChatGPT token response is invalid"));
        }
        let id = response
            .id_token
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| error("TOKEN_INVALID", "ChatGPT identity token is missing"))?;
        let identity = validate_id(
            &self.inner.http,
            endpoints,
            id,
            attempt.client_id,
            Some(attempt.nonce),
        )
        .await?;
        if attempt
            .prior_subject
            .is_some_and(|prior| prior != identity.sub)
        {
            return Err(error(
                "IDENTITY_CHANGED",
                "ChatGPT account identity changed",
            ));
        }
        let _file_lock = self.inner.store.lock_refresh().await?;
        let mut state = self.inner.state.lock().await;
        if state.generation != attempt.generation
            || state
                .pending
                .as_ref()
                .is_none_or(|pending| pending.generation != attempt.generation)
        {
            return Err(error("LOGIN_CANCELLED", "ChatGPT sign-in was cancelled"));
        }
        let latest = self.inner.store.load()?;
        if latest.epoch != attempt.epoch
            || latest.host_id != state.file.host_id
            || latest.registration != state.file.registration
        {
            return Err(error("LOGIN_CANCELLED", "ChatGPT sign-in was cancelled"));
        }
        state.file = latest;
        state.file.registration = Some(Registration {
            client_id: attempt.client_id.to_owned(),
            subject: identity.sub,
            email: identity.email,
            sharing_enabled: Some(false),
        });
        self.inner.store.save(&state.file)?;
        Ok(())
    }
    async fn complete(
        &self,
        callback: Callback,
        endpoints: Endpoints,
        generation: u64,
    ) -> Result<()> {
        let (nonce, verifier, redirect, client, prior_subject, pending_epoch) = {
            let state = self.inner.state.lock().await;
            let pending = state
                .pending
                .as_ref()
                .filter(|p| p.generation == generation)
                .ok_or_else(|| error("LOGIN_CANCELLED", "ChatGPT sign-in was cancelled"))?;
            (
                pending.nonce.clone(),
                pending.verifier.clone(),
                pending.redirect_uri.clone(),
                pending.client_id.clone(),
                pending.prior_subject.clone(),
                pending.epoch,
            )
        };
        if callback.error.is_some() {
            return Err(error("ACCESS_DENIED", "ChatGPT sign-in was denied"));
        }
        let client_id = match client {
            Some(expected) => {
                if callback
                    .client_id
                    .as_deref()
                    .is_some_and(|id| id != expected)
                {
                    return Err(error("CLIENT_MISMATCH", "ChatGPT registration changed"));
                }
                expected
            }
            None => callback
                .client_id
                .filter(|id| id.starts_with("oaiapp_") && id.len() > 7)
                .ok_or_else(|| {
                    error(
                        "REGISTRATION_INCOMPLETE",
                        "ChatGPT issued client ID is missing",
                    )
                })?,
        };
        let code = callback
            .code
            .filter(|code| !code.is_empty())
            .ok_or_else(|| error("CALLBACK_INVALID", "ChatGPT authorization code is missing"))?;
        let reply = self
            .inner
            .http
            .post(&endpoints.token)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", client_id.as_str()),
                ("code", code.as_str()),
                ("code_verifier", verifier.as_str()),
                ("redirect_uri", redirect.as_str()),
                ("resource", RESOURCE),
            ])
            .send()
            .await
            .map_err(|_| network_error())?
            .error_for_status()
            .map_err(|_| error("EXCHANGE_FAILED", "ChatGPT code exchange failed"))?;
        let response: TokenResponse = bounded_json(reply, 64 * 1024)
            .await
            .map_err(|_| error("EXCHANGE_FAILED", "ChatGPT token response is invalid"))?;
        let scope_text = response.scope.clone().unwrap_or_default();
        if !has_plan_scope(&scope_text) {
            self.remember_sharing_disabled(
                &response,
                &endpoints,
                SharingAttempt {
                    client_id: &client_id,
                    nonce: &nonce,
                    prior_subject: prior_subject.as_deref(),
                    generation,
                    epoch: pending_epoch,
                },
            )
            .await?;
            return Err(error(
                "PLAN_SCOPE_MISSING",
                "ChatGPT plan permission was not granted",
            ));
        }
        let (tokens, id) = token_record(response, None)?;
        let identity = validate_id(
            &self.inner.http,
            &endpoints,
            id.as_deref().unwrap_or_default(),
            &client_id,
            Some(&nonce),
        )
        .await?;
        if prior_subject
            .as_deref()
            .is_some_and(|prior| prior != identity.sub)
        {
            return Err(error(
                "IDENTITY_CHANGED",
                "ChatGPT account identity changed",
            ));
        }
        let _file_lock = self.inner.store.lock_refresh().await?;
        let mut state = self.inner.state.lock().await;
        if state.generation != generation
            || state
                .pending
                .as_ref()
                .is_none_or(|p| p.generation != generation)
        {
            return Err(error("LOGIN_CANCELLED", "ChatGPT sign-in was cancelled"));
        }
        let latest = self.inner.store.load()?;
        if latest.epoch != pending_epoch
            || latest.host_id != state.file.host_id
            || latest.registration != state.file.registration
        {
            return Err(error("LOGIN_CANCELLED", "ChatGPT sign-in was cancelled"));
        }
        state.file = latest;
        state.file.registration = Some(Registration {
            client_id,
            subject: identity.sub,
            email: identity.email,
            sharing_enabled: Some(true),
        });
        self.inner
            .store
            .save_tokens(&mut state.file, Some(tokens.clone()))?;
        state.tokens = Some(tokens);
        state.pending = None;
        state.error_code = None;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    fn fake_endpoints(port: u16) -> Endpoints {
        let base = format!("http://127.0.0.1:{port}");
        Endpoints {
            issuer: base.clone(),
            authorization: format!("{base}/authorize"),
            token: format!("{base}/token"),
            jwks: format!("{base}/jwks"),
            revocation: format!("{base}/revoke"),
        }
    }
    fn with_endpoints(root: PathBuf, endpoints: Endpoints) -> AuthManager {
        let mut manager = AuthManager::new(root, "example-app".into()).unwrap();
        Arc::get_mut(&mut manager.inner).unwrap().endpoints = Some(endpoints);
        manager
    }
    #[cfg(windows)]
    fn seeded_credentials(root: &std::path::Path) -> (Store, StateFile) {
        let store = Store::new(root.to_path_buf());
        let mut file = StateFile::fresh();
        file.registration = Some(Registration {
            client_id: "oaiapp_synthetic".into(),
            subject: "synthetic-sub".into(),
            email: Some("synthetic@example.test".into()),
            sharing_enabled: Some(true),
        });
        store
            .save_tokens(
                &mut file,
                Some(TokenRecord {
                    access_token: "synthetic-access".into(),
                    refresh_token: "synthetic-refresh".into(),
                    id_token: "synthetic-id".into(),
                    expires_at: now() + 3600,
                    earliest_refresh_at: None,
                    scopes: SCOPES.split_ascii_whitespace().map(str::to_owned).collect(),
                }),
            )
            .unwrap();
        (store, file)
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn inactive_marker_survives_crash_before_ciphertext_deletion() {
        use crate::store::{credential_path, keyring_entry};
        let dir = tempfile::tempdir().unwrap();
        let (store, mut file) = seeded_credentials(dir.path());
        let reg = file.registration.clone().unwrap();
        let path = credential_path(dir.path(), &file.host_id, &reg);
        assert!(path.exists());
        {
            let _keyring_guard = crate::store::lock_keyring_for_test();
            assert!(
                keyring_entry(&file.host_id, &reg)
                    .unwrap()
                    .get_password()
                    .is_ok()
            );
        }
        // Exact crash boundary: disconnect persisted the tombstone, but the
        // process stopped before it removed the old file and key.
        file.epoch += 1;
        file.credential_active = false;
        store.save(&file).unwrap();
        let fresh = AuthManager::new(dir.path().to_path_buf(), "example-app".into()).unwrap();
        assert_eq!(fresh.status().await.unwrap().state, AuthState::SignedOut);
        assert_eq!(fresh.credentials().await.unwrap_err().code, "NOT_CONNECTED");
        assert!(path.exists());
        store.save_tokens(&mut file, None).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn missing_key_allows_disconnect_to_invalidate_and_clear_local_material() {
        use crate::store::{credential_path, keyring_entry};
        let dir = tempfile::tempdir().unwrap();
        let (store, file) = seeded_credentials(dir.path());
        let reg = file.registration.clone().unwrap();
        let path = credential_path(dir.path(), &file.host_id, &reg);
        {
            let _keyring_guard = crate::store::lock_keyring_for_test();
            keyring_entry(&file.host_id, &reg)
                .unwrap()
                .delete_credential()
                .unwrap();
        }
        let manager = AuthManager::new(dir.path().to_path_buf(), "example-app".into()).unwrap();
        let status = manager.status().await.unwrap();
        assert_eq!(status.state, AuthState::Error);
        assert_eq!(status.error_code.as_deref(), Some("CREDENTIAL_UNUSABLE"));
        manager.disconnect().await.unwrap();
        assert_eq!(manager.status().await.unwrap().state, AuthState::SignedOut);
        let saved = store.load().unwrap();
        assert!(saved.epoch > file.epoch);
        assert!(!saved.credential_active);
        assert_eq!(saved.host_id, file.host_id);
        assert_eq!(saved.registration, file.registration);
        assert!(!path.exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn corrupt_key_allows_fresh_sign_in_without_changing_registration() {
        use crate::store::{credential_path, keyring_entry};
        let dir = tempfile::tempdir().unwrap();
        let (store, file) = seeded_credentials(dir.path());
        let reg = file.registration.clone().unwrap();
        let path = credential_path(dir.path(), &file.host_id, &reg);
        {
            let _keyring_guard = crate::store::lock_keyring_for_test();
            keyring_entry(&file.host_id, &reg)
                .unwrap()
                .set_password("corrupt-key")
                .unwrap();
        }
        let manager = with_endpoints(dir.path().to_path_buf(), fake_endpoints(1234));
        assert_eq!(manager.status().await.unwrap().state, AuthState::Error);
        let start = manager.start().await.unwrap();
        let params: std::collections::HashMap<_, _> = Url::parse(&start.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(params["client_id"], reg.client_id);
        assert_eq!(params["ext_agent_host_id"], file.host_id);
        let saved = store.load().unwrap();
        assert!(!saved.credential_active);
        assert_eq!(saved.registration, file.registration);
        assert!(!path.exists());
        manager.cancel(&start.login_id).await.unwrap();
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn changed_verified_registration_is_not_reset() {
        use crate::store::credential_path;
        let dir = tempfile::tempdir().unwrap();
        let (store, mut file) = seeded_credentials(dir.path());
        let original = file.registration.clone().unwrap();
        let old_cipher = credential_path(dir.path(), &file.host_id, &original);
        let manager = with_endpoints(dir.path().to_path_buf(), fake_endpoints(1234));
        assert_eq!(manager.status().await.unwrap().state, AuthState::Connected);
        file.registration.as_mut().unwrap().subject = "different-subject".into();
        store.save(&file).unwrap();
        assert_eq!(manager.start().await.unwrap_err().code, "IDENTITY_CHANGED");
        assert_eq!(
            manager.disconnect().await.unwrap_err().code,
            "IDENTITY_CHANGED"
        );
        assert!(old_cipher.exists());
        file.registration = Some(original);
        store.save_tokens(&mut file, None).unwrap();
    }
    #[tokio::test]
    async fn direct_first_run_disconnect_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let manager = AuthManager::new(dir.path().to_path_buf(), "example-app".into()).unwrap();
        manager.disconnect().await.unwrap();
        assert_eq!(manager.status().await.unwrap().state, AuthState::SignedOut);
        let saved = Store::new(dir.path().to_path_buf()).load().unwrap();
        assert!(saved.host_id.starts_with("urn:uuid:"));
        assert!(saved.registration.is_none());
    }
    #[tokio::test]
    async fn malformed_registration_is_not_reset_by_auth_actions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chatgpt-auth.json");
        std::fs::write(&path, b"{\"host_id\":\"bad\",\"registration\":null}").unwrap();
        let manager = with_endpoints(dir.path().to_path_buf(), fake_endpoints(1234));
        assert_eq!(
            manager.start().await.unwrap_err().code,
            "STORAGE_UNAVAILABLE"
        );
        assert_eq!(
            manager.disconnect().await.unwrap_err().code,
            "STORAGE_UNAVAILABLE"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"{\"host_id\":\"bad\",\"registration\":null}"
        );
    }

    #[test]
    fn callback_accepts_only_expected_path_and_requires_state() {
        assert!(parse_callback("GET /callback?state=x&code=y HTTP/1.1\r\n").is_none());
        assert!(parse_callback("POST /auth/callback?state=x HTTP/1.1\r\n").is_none());
        assert!(parse_callback("GET /auth/callback?code=y HTTP/1.1\r\n").is_none());
        assert!(parse_callback("GET /auth/callback?state=s&state=s&code=c HTTP/1.1\r\n").is_none());
        assert!(parse_callback("GET /auth/callback?state=s&code=c&code=d HTTP/1.1\r\n").is_none());
        let callback =
            parse_callback("GET /auth/callback?state=s&code=c&client_id=oaiapp_test HTTP/1.1\r\n")
                .unwrap();
        assert_eq!(callback.state, "s");
        assert_eq!(callback.client_id.as_deref(), Some("oaiapp_test"));
    }
    #[tokio::test]
    async fn callback_waits_for_complete_bounded_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut client = tokio::net::TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (mut server, _) = listener.accept().await.unwrap();
        client
            .write_all(b"GET /auth/callback?state=s&co")
            .await
            .unwrap();
        let task = tokio::spawn(async move { read_callback_request(&mut server).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!task.is_finished());
        client
            .write_all(b"de=c HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let request = task.await.unwrap().unwrap();
        assert_eq!(parse_callback(&request).unwrap().code.as_deref(), Some("c"));
    }
    #[test]
    fn token_response_requires_exact_scope_and_rotates_together() {
        let old = TokenRecord {
            access_token: "old-access".into(),
            refresh_token: "old-refresh".into(),
            id_token: "id".into(),
            expires_at: 0,
            earliest_refresh_at: Some(10),
            scopes: vec!["chatgpt.tokens.use.direct".into()],
        };
        let response = TokenResponse {
            access_token: "new-access".into(),
            refresh_token: Some("new-refresh".into()),
            id_token: None,
            expires_in: 3600,
            scope: Some("openid chatgpt.tokens.use.direct".into()),
            token_type: Some("Bearer".into()),
            earliest_refresh_at: Some(serde_json::json!(1_800_000_000)),
        };
        let (next, new_id) = token_record(response, Some(&old)).unwrap();
        assert_eq!(next.access_token, "new-access");
        assert_eq!(next.refresh_token, "new-refresh");
        assert_eq!(next.id_token, "id");
        assert!(new_id.is_none());
        assert!(next.expires_at > now());
        assert_eq!(next.earliest_refresh_at, Some(1_800_000_000));
        let dated = TokenResponse {
            access_token: "new-access".into(),
            refresh_token: Some("new-refresh".into()),
            id_token: None,
            expires_in: 3600,
            scope: Some("openid chatgpt.tokens.use.direct".into()),
            token_type: Some("Bearer".into()),
            earliest_refresh_at: Some(serde_json::json!("2026-10-15T00:00:00Z")),
        };
        assert_eq!(
            token_record(dated, Some(&old))
                .unwrap()
                .0
                .earliest_refresh_at,
            Some(1_792_022_400)
        );
        let ignored = TokenResponse {
            access_token: "new-access".into(),
            refresh_token: Some("new-refresh".into()),
            id_token: None,
            expires_in: 3600,
            scope: Some("openid chatgpt.tokens.use.direct".into()),
            token_type: Some("Bearer".into()),
            earliest_refresh_at: Some(serde_json::json!("not-a-time")),
        };
        assert_eq!(
            token_record(ignored, Some(&old))
                .unwrap()
                .0
                .earliest_refresh_at,
            None
        );
        let missing = TokenResponse {
            access_token: "x".into(),
            refresh_token: Some("r".into()),
            id_token: Some("id".into()),
            expires_in: 1,
            scope: Some("openid".into()),
            token_type: Some("Bearer".into()),
            earliest_refresh_at: None,
        };
        assert_eq!(
            token_record(missing, None).unwrap_err().code,
            "PLAN_SCOPE_MISSING"
        );
    }
    #[test]
    fn discovery_rejects_untrusted_endpoints() {
        for authorization in [
            "https://evil.test/authorize",
            "https://user:password@auth.openai.com/api/accounts/authorize",
            "https://auth.openai.com/api/accounts/authorize#fragment",
            "https://auth.openai.com/api/accounts/authorize?injected=true",
        ] {
            let meta = Discovery {
                issuer: ISSUER.into(),
                authorization_endpoint: authorization.into(),
                token_endpoint: "https://auth.openai.com/api/accounts/oauth/token".into(),
                jwks_uri: "https://auth.openai.com/.well-known/jwks.json".into(),
                revocation_endpoint: "https://auth.openai.com/api/accounts/oauth/revoke".into(),
            };
            assert_eq!(
                Endpoints::checked(meta, false).err().unwrap().code,
                "DISCOVERY_INVALID"
            );
        }
    }
    #[tokio::test]
    async fn start_uses_stable_host_fresh_pkce_and_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let manager = with_endpoints(dir.path().to_path_buf(), fake_endpoints(1234));
        let first = manager.start().await.unwrap();
        let first_url = Url::parse(&first.auth_url).unwrap();
        let params: std::collections::HashMap<_, _> =
            first_url.query_pairs().into_owned().collect();
        assert_eq!(params["client_id"], "dynamic_agent_client");
        assert_eq!(params["agent_name_hint"], "example-app");
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(params["resource"], RESOURCE);
        assert!(params["redirect_uri"].starts_with("http://127.0.0.1:"));
        assert!(params["redirect_uri"].ends_with("/auth/callback"));
        let host = params["ext_agent_host_id"].clone();
        assert!(host.starts_with("urn:uuid:"));
        assert_eq!(manager.status().await.unwrap().state, AuthState::Pending);
        assert_eq!(
            manager.cancel("wrong").await.unwrap_err().code,
            "LOGIN_NOT_FOUND"
        );
        manager.cancel(&first.login_id).await.unwrap();
        assert_eq!(manager.status().await.unwrap().state, AuthState::SignedOut);
        let second = manager.start().await.unwrap();
        let second_params: std::collections::HashMap<_, _> = Url::parse(&second.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(second_params["ext_agent_host_id"], host);
        assert_ne!(second_params["state"], params["state"]);
        assert_ne!(second_params["nonce"], params["nonce"]);
        assert_ne!(second_params["code_challenge"], params["code_challenge"]);
        manager.cancel(&second.login_id).await.unwrap();
        let restarted = with_endpoints(dir.path().to_path_buf(), fake_endpoints(1234));
        let third = restarted.start().await.unwrap();
        let third_params: std::collections::HashMap<_, _> = Url::parse(&third.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(third_params["ext_agent_host_id"], host);
        restarted.cancel(&third.login_id).await.unwrap();
    }
    #[tokio::test]
    async fn status_invalidates_pending_after_another_manager_disconnects() {
        let dir = tempfile::tempdir().unwrap();
        let endpoints = fake_endpoints(1234);
        let first = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        let started = first.start().await.unwrap();
        assert_eq!(first.status().await.unwrap().state, AuthState::Pending);
        let second = with_endpoints(dir.path().to_path_buf(), endpoints);
        second.disconnect().await.unwrap();
        let status = first.status().await.unwrap();
        assert_eq!(status.state, AuthState::SignedOut);
        assert!(status.login_id.is_none());
        assert_eq!(
            first.cancel(&started.login_id).await.unwrap_err().code,
            "LOGIN_NOT_FOUND"
        );
    }
    #[tokio::test]
    async fn signed_id_requires_issuer_audience_nonce_expiry_and_signature() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let endpoints = fake_endpoints(port);
        let jwks = json!({"keys":[{"kty":"RSA","kid":"test","use":"sig","alg":"RS256","n":"s60-5pDzVrPy1E_8O_3N-WKcrlRs0hRuviNNkfH-oXdPCFjaflJdekxcyZP0u5xwo2kBaMKp2WkDXHmrbJQ1WidRC_R0fzlN8fVvbQFWAlACi_rH-AVcuom6W4mf1pNId1TgXvcF3eAe33Y98zOIpNy8SG9w2XceMXW9qsiUlgkoWcf6ybIN83YknGM70NUMHJ2_baOkl44dOzOoXw1784ahZBj47D6AZFvfYgaGXhOlHr-vYvPed3RcwrnX9nY3qZokk5VNRJoJo6RxR00L3MesEWxRV4mMITuv6j64Ugi6vKjds_5niDaGT3uE6vJ_O_kcWEfSKsMr2MWa4KKfuw","e":"AQAB"}]});
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut request = [0u8; 2048];
                let _ = socket.read(&mut request).await;
                let body = jwks.to_string();
                let message = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(message.as_bytes()).await;
            }
        });
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test".into());
        let key =
            EncodingKey::from_rsa_pem(include_bytes!("../tests/fixtures/test-rsa.pem")).unwrap();
        let signed = |issuer: &str, audience: &str, nonce: &str, exp: u64| {
            encode(
                &header,
                &json!({"iss":issuer,"aud":audience,"sub":"sub-1","nonce":nonce,
                "email":"one@example.test","exp":exp}),
                &key,
            )
            .unwrap()
        };
        let http = reqwest::Client::new();
        let valid = signed(&endpoints.issuer, "oaiapp_test", "nonce-1", now() + 3600);
        assert_eq!(
            validate_id(&http, &endpoints, &valid, "oaiapp_test", Some("nonce-1"))
                .await
                .unwrap()
                .sub,
            "sub-1"
        );
        let skewed = signed(&endpoints.issuer, "oaiapp_test", "nonce-1", now() - 3);
        assert_eq!(
            validate_id(&http, &endpoints, &skewed, "oaiapp_test", Some("nonce-1"))
                .await
                .unwrap()
                .sub,
            "sub-1"
        );
        for bad in [
            signed(&endpoints.issuer, "other", "nonce-1", now() + 3600),
            signed("https://evil.test", "oaiapp_test", "nonce-1", now() + 3600),
            signed(&endpoints.issuer, "oaiapp_test", "other", now() + 3600),
            signed(&endpoints.issuer, "oaiapp_test", "nonce-1", now() - 10),
            signed(&endpoints.issuer, "oaiapp_test", "nonce-1", now() - 30),
        ] {
            assert_eq!(
                validate_id(&http, &endpoints, &bad, "oaiapp_test", Some("nonce-1"))
                    .await
                    .unwrap_err()
                    .code,
                "IDENTITY_INVALID"
            );
        }
        // A signed token still needs every standard claim this flow checks.
        // jsonwebtoken's validation flags alone do not require claim presence.
        for claim in ["iss", "aud", "exp", "sub"] {
            for replacement in [None, Some(json!(null)), Some(json!(true)), Some(json!(42))] {
                let mut claims = json!({"iss":endpoints.issuer,"aud":"oaiapp_test",
                    "sub":"sub-1","nonce":"nonce-1","exp":now()+3600});
                match replacement {
                    Some(value) => claims[claim] = value,
                    None => {
                        claims.as_object_mut().unwrap().remove(claim);
                    }
                }
                let token = encode(&header, &claims, &key).unwrap();
                assert!(
                    validate_id(&http, &endpoints, &token, "oaiapp_test", Some("nonce-1"))
                        .await
                        .is_err(),
                    "missing or invalid {claim} must fail identity validation"
                );
            }
        }
        let mut tampered = valid.into_bytes();
        let last = tampered.len() - 2;
        tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
        assert_eq!(
            validate_id(
                &http,
                &endpoints,
                std::str::from_utf8(&tampered).unwrap(),
                "oaiapp_test",
                Some("nonce-1")
            )
            .await
            .unwrap_err()
            .code,
            "IDENTITY_INVALID"
        );
    }
    #[tokio::test]
    async fn registration_refresh_disconnect_and_account_switch_rejection() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let endpoints = fake_endpoints(port);
        let dir = tempfile::tempdir().unwrap();
        let manager = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        let start = manager.start().await.unwrap();
        let query: std::collections::HashMap<_, _> = Url::parse(&start.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        let nonce = query["nonce"].clone();
        let state = query["state"].clone();
        let callback_url = format!(
            "{}?state={state}&code=synthetic-code&client_id=oaiapp_fixture",
            query["redirect_uri"]
        );
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test".into());
        let key =
            EncodingKey::from_rsa_pem(include_bytes!("../tests/fixtures/test-rsa.pem")).unwrap();
        let issue = |subject: &str, nonce: &str| {
            encode(
                &header,
                &json!({"iss":endpoints.issuer,"aud":"oaiapp_fixture","sub":subject,
                "nonce":nonce,"email":"one@example.test","exp":now()+3600}),
                &key,
            )
            .unwrap()
        };
        let first_id = issue("sub-1", &nonce);
        let id_slot = Arc::new(Mutex::new(first_id));
        let token_calls = Arc::new(AtomicUsize::new(0));
        let revocations = Arc::new(AtomicUsize::new(0));
        let slot = id_slot.clone();
        let calls = token_calls.clone();
        let revokes = revocations.clone();
        let server = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut data = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let size = socket.read(&mut chunk).await.unwrap_or(0);
                    if size == 0 {
                        break;
                    }
                    data.extend_from_slice(&chunk[..size]);
                    if let Some(header_end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&data[..header_end]);
                        let len: usize = header
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse().ok())
                            })
                            .unwrap_or(0);
                        if data.len() >= header_end + 4 + len {
                            break;
                        }
                    }
                }
                let request = String::from_utf8_lossy(&data);
                let (status, body) = if request.starts_with("GET /jwks ") {
                    ("200 OK", json!({"keys":[{"kty":"RSA","kid":"test","use":"sig","alg":"RS256","n":"s60-5pDzVrPy1E_8O_3N-WKcrlRs0hRuviNNkfH-oXdPCFjaflJdekxcyZP0u5xwo2kBaMKp2WkDXHmrbJQ1WidRC_R0fzlN8fVvbQFWAlACi_rH-AVcuom6W4mf1pNId1TgXvcF3eAe33Y98zOIpNy8SG9w2XceMXW9qsiUlgkoWcf6ybIN83YknGM70NUMHJ2_baOkl44dOzOoXw1784ahZBj47D6AZFvfYgaGXhOlHr-vYvPed3RcwrnX9nY3qZokk5VNRJoJo6RxR00L3MesEWxRV4mMITuv6j64Ugi6vKjds_5niDaGT3uE6vJ_O_kcWEfSKsMr2MWa4KKfuw","e":"AQAB"}]}).to_string())
                } else if request.starts_with("POST /token ") {
                    assert!(request.contains("client_id=oaiapp_fixture"));
                    assert!(request.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));
                    let n = calls.fetch_add(1, Ordering::SeqCst);
                    let id = slot.lock().await.clone();
                    if n != 1 {
                        assert!(request.contains("grant_type=authorization_code"));
                        assert!(request.contains("code_verifier="));
                    } else {
                        assert!(request.contains("grant_type=refresh_token"));
                        assert!(request.contains("refresh_token=refresh-1"));
                    }
                    ("200 OK", json!({"access_token":format!("access-{}",n+1),
                        "refresh_token":format!("refresh-{}",n+1),"id_token":id,
                        "expires_in":3600,"token_type":"Bearer",
                        "scope":"openid profile email offline_access resource.invoke chatgpt.tokens.use.direct"}).to_string())
                } else if request.starts_with("POST /revoke ") {
                    revokes.fetch_add(1, Ordering::SeqCst);
                    assert!(request.contains("token=refresh-2"));
                    ("200 OK", String::new())
                } else {
                    ("404 Not Found", String::new())
                };
                let message = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(message.as_bytes()).await.unwrap();
            }
        });
        reqwest::get(callback_url).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if manager.status().await.unwrap().state == AuthState::Connected {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let connected = manager.credentials().await.unwrap();
        assert_eq!(connected.access_token, "access-1");
        assert_eq!(connected.subject, "sub-1");
        assert_eq!(connected.client_id, "oaiapp_fixture");
        let twin = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        assert_eq!(twin.status().await.unwrap().state, AuthState::Connected);
        {
            let mut state = manager.inner.state.lock().await;
            state.tokens.as_mut().unwrap().expires_at = now() - 1;
            let expired = state.tokens.clone();
            manager
                .inner
                .store
                .save_tokens(&mut state.file, expired)
                .unwrap();
        }
        let refreshed = manager.credentials().await.unwrap();
        assert_eq!(refreshed.access_token, "access-2");
        assert_eq!(token_calls.load(Ordering::SeqCst), 2);
        assert_eq!(twin.status().await.unwrap().state, AuthState::Connected);
        assert_eq!(twin.credentials().await.unwrap().access_token, "access-2");
        assert_eq!(token_calls.load(Ordering::SeqCst), 2);
        #[cfg(windows)]
        {
            let credential_file = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .ends_with(".credentials")
                })
                .unwrap();
            let sealed = std::fs::read(credential_file.path()).unwrap();
            assert!(!String::from_utf8_lossy(&sealed).contains("access-2"));
        }
        let seen_local_clear = Arc::new(AtomicUsize::new(0));
        let seen = seen_local_clear.clone();
        let checker = manager.clone();
        let before_revoke = revocations.clone();
        manager
            .disconnect_with(move || async move {
                assert_eq!(checker.status().await.unwrap().state, AuthState::SignedOut);
                assert_eq!(before_revoke.load(Ordering::SeqCst), 0);
                seen.store(1, Ordering::SeqCst);
            })
            .await
            .unwrap();
        assert_eq!(seen_local_clear.load(Ordering::SeqCst), 1);
        assert_eq!(revocations.load(Ordering::SeqCst), 1);
        assert_eq!(manager.status().await.unwrap().state, AuthState::SignedOut);
        assert_eq!(twin.status().await.unwrap().state, AuthState::SignedOut);
        assert_eq!(twin.credentials().await.unwrap_err().code, "NOT_CONNECTED");
        let returning = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        let start = returning.start().await.unwrap();
        let query: std::collections::HashMap<_, _> = Url::parse(&start.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(query["client_id"], "oaiapp_fixture");
        assert!(!query.contains_key("agent_name_hint"));
        let second_id = issue("sub-2", &query["nonce"]);
        *id_slot.lock().await = second_id;
        let callback_url = format!(
            "{}?state={}&code=another-code&client_id=oaiapp_fixture",
            query["redirect_uri"], query["state"]
        );
        reqwest::get(callback_url).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if returning.status().await.unwrap().state == AuthState::Error {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            returning.status().await.unwrap().error_code.as_deref(),
            Some("IDENTITY_CHANGED")
        );
        let stale = returning.start().await.unwrap();
        let query: std::collections::HashMap<_, _> = Url::parse(&stale.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        *id_slot.lock().await = issue("sub-1", &query["nonce"]);
        let disconnector = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        disconnector.disconnect().await.unwrap(); // Changes the durable epoch in another manager.
        reqwest::get(format!(
            "{}?state={}&code=late-code&client_id=oaiapp_fixture",
            query["redirect_uri"], query["state"]
        ))
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if returning.status().await.unwrap().state == AuthState::SignedOut {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let restarted = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        assert_eq!(
            restarted.status().await.unwrap().state,
            AuthState::SignedOut
        );
        server.abort();
    }
    #[tokio::test]
    async fn callback_rejects_missing_or_changed_client_and_cancelled_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let endpoints = fake_endpoints(1234);
        let manager = with_endpoints(dir.path().to_path_buf(), endpoints.clone());
        let start = manager.start().await.unwrap();
        let generation = manager.inner.state.lock().await.generation;
        let missing = Callback {
            state: String::new(),
            code: Some("code".into()),
            client_id: None,
            error: None,
        };
        assert_eq!(
            manager
                .complete(missing, endpoints.clone(), generation)
                .await
                .unwrap_err()
                .code,
            "REGISTRATION_INCOMPLETE"
        );
        manager.cancel(&start.login_id).await.unwrap();
        let late = Callback {
            state: String::new(),
            code: Some("code".into()),
            client_id: Some("oaiapp_test".into()),
            error: None,
        };
        assert_eq!(
            manager
                .complete(late, endpoints.clone(), generation)
                .await
                .unwrap_err()
                .code,
            "LOGIN_CANCELLED"
        );
        {
            let mut state = manager.inner.state.lock().await;
            state.file.registration = Some(Registration {
                client_id: "oaiapp_test".into(),
                subject: "sub-1".into(),
                email: None,
                sharing_enabled: Some(true),
            });
            manager.inner.store.save(&state.file).unwrap();
        }
        let returning = manager.start().await.unwrap();
        let generation = manager.inner.state.lock().await.generation;
        let changed = Callback {
            state: String::new(),
            code: Some("code".into()),
            client_id: Some("oaiapp_other".into()),
            error: None,
        };
        assert_eq!(
            manager
                .complete(changed, endpoints, generation)
                .await
                .unwrap_err()
                .code,
            "CLIENT_MISMATCH"
        );
        manager.cancel(&returning.login_id).await.unwrap();
    }
    fn seed_registration(
        root: &std::path::Path,
        sharing: Option<bool>,
        tokens: Option<TokenRecord>,
    ) {
        let store = Store::new(root.to_path_buf());
        let mut file = StateFile::fresh();
        file.registration = Some(Registration {
            client_id: "oaiapp_synthetic".into(),
            subject: "synthetic-sub".into(),
            email: Some("one@example.test".into()),
            sharing_enabled: sharing,
        });
        store.save_tokens(&mut file, tokens).unwrap();
    }
    #[tokio::test]
    async fn earliest_refresh_at_does_not_race_a_deferred_renewal() {
        let dir = tempfile::tempdir().unwrap();
        seed_registration(
            dir.path(),
            Some(true),
            Some(TokenRecord {
                access_token: "still-good".into(),
                refresh_token: "refresh".into(),
                id_token: "id".into(),
                expires_at: now() + 20,
                earliest_refresh_at: Some(now() + 600),
                scopes: vec!["chatgpt.tokens.use.direct".into()],
            }),
        );
        let manager = with_endpoints(dir.path().to_path_buf(), fake_endpoints(9));
        let current = manager.credentials().await.unwrap();
        assert_eq!(current.access_token, "still-good");
        seed_registration(
            dir.path(),
            Some(true),
            Some(TokenRecord {
                access_token: "expired".into(),
                refresh_token: "refresh".into(),
                id_token: "id".into(),
                expires_at: now().saturating_sub(5),
                earliest_refresh_at: Some(now() + 600),
                scopes: vec!["chatgpt.tokens.use.direct".into()],
            }),
        );
        let waiting = with_endpoints(dir.path().to_path_buf(), fake_endpoints(9));
        assert_eq!(
            waiting.credentials().await.unwrap_err().code,
            "REFRESH_DEFERRED"
        );
    }
    #[tokio::test]
    async fn disabled_plan_sharing_asks_for_consent_on_the_next_attempt() {
        let dir = tempfile::tempdir().unwrap();
        seed_registration(dir.path(), Some(false), None);
        let manager = with_endpoints(dir.path().to_path_buf(), fake_endpoints(9));
        let start = manager.start().await.unwrap();
        let query: std::collections::HashMap<_, _> = Url::parse(&start.auth_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(query.get("prompt").map(String::as_str), Some("consent"));
        assert_eq!(
            query.get("login_hint").map(String::as_str),
            Some("one@example.test")
        );
        manager.cancel(&start.login_id).await.unwrap();
    }
    #[tokio::test]
    async fn es256_identity_tokens_match_the_jwk_curve() {
        use ring::signature::KeyPair;
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::EcdsaKeyPair::generate_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &rng,
        )
        .unwrap();
        let pair = ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            pkcs8.as_ref(),
            &rng,
        )
        .unwrap();
        let public = pair.public_key().as_ref();
        assert_eq!(public.first().copied(), Some(0x04));
        assert_eq!(public.len(), 65);
        let x = URL_SAFE_NO_PAD.encode(&public[1..33]);
        let y = URL_SAFE_NO_PAD.encode(&public[33..65]);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let endpoints = fake_endpoints(port);
        let jwks = json!({"keys":[{"kty":"EC","crv":"P-256","kid":"ec","use":"sig","alg":"ES256","x":x,"y":y}]});
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut request = [0u8; 2048];
                let _ = socket.read(&mut request).await;
                let body = jwks.to_string();
                let message = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(message.as_bytes()).await;
            }
        });
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some("ec".into());
        let key = EncodingKey::from_ec_der(pkcs8.as_ref());
        let token = encode(
            &header,
            &json!({"iss":endpoints.issuer,"aud":"oaiapp_test","sub":"sub-ec","nonce":"nonce-ec","exp":now()+3600}),
            &key,
        )
        .unwrap();
        let http = reqwest::Client::new();
        assert_eq!(
            validate_id(&http, &endpoints, &token, "oaiapp_test", Some("nonce-ec"))
                .await
                .unwrap()
                .sub,
            "sub-ec"
        );
        let mut rsa_header = Header::new(Algorithm::RS256);
        rsa_header.kid = Some("ec".into());
        let rsa =
            EncodingKey::from_rsa_pem(include_bytes!("../tests/fixtures/test-rsa.pem")).unwrap();
        let confused = encode(
            &rsa_header,
            &json!({"iss":endpoints.issuer,"aud":"oaiapp_test","sub":"sub-ec","nonce":"nonce-ec","exp":now()+3600}),
            &rsa,
        )
        .unwrap();
        assert_eq!(
            validate_id(
                &http,
                &endpoints,
                &confused,
                "oaiapp_test",
                Some("nonce-ec")
            )
            .await
            .unwrap_err()
            .code,
            "IDENTITY_INVALID"
        );
    }
    #[test]
    fn pinned_chatgpt_oauth_surface_matches_the_t3code_snapshot() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../upstream/t3code/snapshot.json");
        let snapshot: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let oauth = &snapshot["adopted"]["chatgpt"];
        assert_eq!(oauth["issuer"], ISSUER);
        assert_eq!(oauth["resource"], RESOURCE);
        assert_eq!(oauth["scopes"], SCOPES);
        assert_eq!(oauth["requiredScope"], "chatgpt.tokens.use.direct");
        assert_eq!(oauth["clockToleranceSeconds"], IDENTITY_LEEWAY_SECONDS);
        assert_eq!(oauth["refreshSkewSeconds"], REFRESH_SKEW_SECONDS);
        assert_eq!(
            oauth["algorithms"].as_array().unwrap().len(),
            IDENTITY_ALGS.len()
        );
        for alg in IDENTITY_ALGS {
            assert!(
                oauth["algorithms"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item == alg)
            );
        }
        assert_eq!(
            snapshot["source"]["commit"],
            "e9236274a3a6e0c731a7df4092db1e686f039068"
        );
    }
}
