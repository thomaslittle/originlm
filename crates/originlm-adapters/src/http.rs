//! Credential-bound HTTP transport. Never follows redirects or logs upstream bodies.
use originlm_contracts::*;
use reqwest::{
    Client, Response, Url,
    header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue, RETRY_AFTER},
};
use serde_json::Value;
use std::{net::IpAddr, time::Duration};
use zeroize::Zeroize;

const BODY_LIMIT: usize = 2 * 1024 * 1024;
const MAX_METADATA_BODY_LIMIT: usize = 16 * 1024 * 1024;
const LINE_LIMIT: usize = 1024 * 1024;
const STREAM_LIMIT: usize = 32 * 1024 * 1024;
pub enum Authentication {
    None,
    Bearer(String),
    Header(String, String),
    Basic(String, String),
}
#[derive(Clone)]
pub struct Endpoint {
    base: Url,
    client: Client,
    headers: HeaderMap,
}
fn error(code: ErrorCode) -> DomainError {
    DomainError::new(code, "Provider HTTP operation failed")
}
fn invalid() -> DomainError {
    error(ErrorCode::InvalidRequest)
}
fn network(e: reqwest::Error) -> DomainError {
    error(if e.is_timeout() {
        ErrorCode::Timeout
    } else {
        ErrorCode::ProviderUnavailable
    })
}
fn retry_after(response: &Response) -> Option<u64> {
    let value = response.headers().get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.min(86_400) * 1000);
    }
    let at = chrono::DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&chrono::Utc);
    let milliseconds = (at - chrono::Utc::now()).num_milliseconds();
    (milliseconds > 0).then_some((milliseconds as u64).min(86_400_000))
}
async fn response_error(mut response: Response) -> DomainError {
    const ERROR_BODY_LIMIT: usize = 8192;
    let status = response.status();
    let code = match response.status().as_u16() {
        401 | 403 => ErrorCode::AuthRequired,
        404 => ErrorCode::NotFound,
        429 => ErrorCode::RateLimited,
        _ => ErrorCode::ProviderUnavailable,
    };
    let retry = retry_after(&response);
    let mut bytes = Vec::new();
    while bytes.len() < ERROR_BODY_LIMIT {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = ERROR_BODY_LIMIT - bytes.len();
                bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            }
            Ok(None) | Err(_) => break,
        }
    }
    let body = serde_json::from_slice::<Value>(&bytes).ok();
    let detail = body
        .as_ref()
        .and_then(|body| {
            body["error"]["message"]
                .as_str()
                .or_else(|| body["message"].as_str())
                .or_else(|| body["detail"].as_str())
                .or_else(|| body["error_description"].as_str())
                .or_else(|| body["error"].as_str())
        })
        .map(str::trim)
        .filter(|detail| !detail.is_empty())
        .map(redact_diagnostic_detail);
    let message = detail.filter(|detail| !detail.is_empty()).map_or_else(
        || {
            format!(
                "Provider HTTP request failed with status {}",
                status.as_u16()
            )
        },
        |detail| format!("HTTP {}: {detail}", status.as_u16()),
    );
    let mut error = DomainError::new(code, message);
    if matches!(error.code, ErrorCode::RateLimited) {
        error = error.retryable();
    }
    if let Some(milliseconds) = retry {
        error = error.with_retry_after_ms(milliseconds);
    }
    error
}
fn local(url: &Url) -> bool {
    url.host_str()
        .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
        .is_some_and(|ip| ip.is_loopback())
}
impl Endpoint {
    /// Only documented OpenRouter origins define usage.cost in USD credits.
    pub(crate) fn reports_usd_cost(&self) -> bool {
        self.base.scheme() == "https"
            && self.base.port_or_known_default() == Some(443)
            && matches!(
                self.base.host_str(),
                Some("openrouter.ai" | "eu.openrouter.ai" | "us.openrouter.ai")
            )
    }

    /// `local_only` accepts literal loopback addresses only; no DNS or proxy detour.
    pub fn new(url: &str, auth: Authentication, local_only: bool) -> Result<Self> {
        if url.len() > 2048
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
            || url.contains('\\')
        {
            return Err(invalid());
        }
        let mut base = Url::parse(url).map_err(|_| invalid())?;
        if !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.host_str().is_none()
            || (base.scheme() != "https" && !(base.scheme() == "http" && local(&base)))
            || (local_only && !local(&base))
        {
            return Err(invalid());
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let mut headers = HeaderMap::new();
        let mut insert = |name: HeaderName, mut value: String| -> Result<()> {
            let parsed = HeaderValue::from_str(&value).map_err(|_| invalid());
            value.zeroize();
            let mut parsed = parsed?;
            parsed.set_sensitive(true);
            headers.insert(name, parsed);
            Ok(())
        };
        match auth {
            Authentication::None => {}
            Authentication::Bearer(mut token) => {
                if token.is_empty()
                    || token.len() > 16384
                    || token.chars().any(|c| c.is_control() || c.is_whitespace())
                {
                    token.zeroize();
                    return Err(invalid());
                }
                let value = format!("Bearer {token}");
                token.zeroize();
                insert(AUTHORIZATION, value)?;
            }
            Authentication::Header(name, mut value) => {
                // Restrict custom auth to dedicated API-key headers, not routing/cookie headers.
                if !["x-api-key", "api-key", "x-goog-api-key"]
                    .contains(&name.to_ascii_lowercase().as_str())
                    || value.is_empty()
                    || value.len() > 16384
                {
                    value.zeroize();
                    return Err(invalid());
                }
                insert(
                    HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid())?,
                    value,
                )?;
            }
            Authentication::Basic(mut user, mut password) => {
                if user.contains(':')
                    || user.len() + password.len() > 16384
                    || user.chars().chain(password.chars()).any(char::is_control)
                {
                    user.zeroize();
                    password.zeroize();
                    return Err(invalid());
                }
                let builder = Client::new()
                    .get(base.clone())
                    .basic_auth(&user, Some(&password));
                user.zeroize();
                password.zeroize();
                let request = builder.build().map_err(network)?;
                let mut value = request
                    .headers()
                    .get(AUTHORIZATION)
                    .cloned()
                    .ok_or_else(invalid)?;
                value.set_sensitive(true);
                headers.insert(AUTHORIZATION, value);
            }
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(30))
            .user_agent(concat!("originlm/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(network)?;
        Ok(Self {
            base,
            client,
            headers,
        })
    }
    fn url(&self, path: &str) -> Result<Url> {
        if path.is_empty()
            || path.len() > 2048
            || path.starts_with('/')
            || path
                .chars()
                .any(|c| !(c.is_ascii_alphanumeric() || "_-/.:".contains(c)))
            || path.contains("://")
            || path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(invalid());
        }
        let url = self.base.join(path).map_err(|_| invalid())?;
        if url.origin() != self.base.origin() || !url.path().starts_with(self.base.path()) {
            return Err(invalid());
        }
        Ok(url)
    }
    pub fn anthropic_version(mut self) -> Self {
        self.headers
            .insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        self
    }
    fn query_url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        if query.len() > 8
            || query.iter().any(|(key, value)| {
                ![
                    "alt",
                    "pageToken",
                    "pageSize",
                    "limit",
                    "after_id",
                    "directory",
                ]
                .contains(key)
                    || value.len() > 4096
                    || value.chars().any(char::is_control)
            })
        {
            return Err(invalid());
        }
        let mut url = self.url(path)?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        Ok(url)
    }
    pub async fn stream_query(
        &self,
        path: &str,
        query: &[(&str, &str)],
        body: Value,
    ) -> Result<LineStream> {
        let response = self
            .send_url(self.query_url(path, query)?, Some(body), true)
            .await?;
        Ok(LineStream {
            response,
            pending: Vec::new(),
            total: 0,
            done: false,
        })
    }
    async fn send(&self, path: &str, body: Option<Value>, streaming: bool) -> Result<Response> {
        let url = self.url(path)?;
        self.send_url(url, body, streaming).await
    }
    async fn send_url(&self, url: Url, body: Option<Value>, streaming: bool) -> Result<Response> {
        let mut request = if body.is_some() {
            self.client.post(url)
        } else {
            self.client.get(url)
        };
        request = request
            .headers(self.headers.clone())
            .timeout(Duration::from_secs(if streaming { 600 } else { 20 }));
        if let Some(body) = body {
            let bytes = serde_json::to_vec(&body).map_err(|_| invalid())?;
            if bytes.len() > BODY_LIMIT {
                return Err(error(ErrorCode::FrameTooLarge));
            }
            request = request
                .header("content-type", "application/json")
                .body(bytes);
        }
        let response = request.send().await.map_err(network)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(error(ErrorCode::IntegrationRestricted));
        }
        if !status.is_success() {
            return Err(response_error(response).await);
        }
        Ok(response)
    }
    pub async fn action(&self, path: &str, query: &[(&str, &str)], body: Value) -> Result<Value> {
        let mut response = self
            .send_url(self.query_url(path, query)?, Some(body), false)
            .await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            if bytes.len() + chunk.len() > BODY_LIMIT {
                return Err(error(ErrorCode::FrameTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_slice(&bytes).map_err(|_| error(ErrorCode::ProtocolIncompatible))
        }
    }
    pub async fn event_stream(&self, path: &str, query: &[(&str, &str)]) -> Result<LineStream> {
        let response = self
            .send_url(self.query_url(path, query)?, None, true)
            .await?;
        Ok(LineStream {
            response,
            pending: Vec::new(),
            total: 0,
            done: false,
        })
    }
    pub async fn delete(&self, path: &str, query: &[(&str, &str)]) -> Result<()> {
        let response = self
            .client
            .delete(self.query_url(path, query)?)
            .headers(self.headers.clone())
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(network)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(error(ErrorCode::ProviderUnavailable))
        }
    }
    /// Opt-in idempotent deletion for an adapter's already-owned resource.
    /// A 404 is successful only with authentication, bounded JSON, and an
    /// adapter-verified absence confirmation for that exact resource.
    pub(crate) async fn delete_with_confirmed_absence(
        &self,
        path: &str,
        query: &[(&str, &str)],
        confirm_missing: impl FnOnce(&Value) -> bool + Send,
    ) -> Result<()> {
        let mut response = self
            .client
            .delete(self.query_url(path, query)?)
            .headers(self.headers.clone())
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(network)?;
        if response.status().is_success() {
            return Ok(());
        }
        let json_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
        if response.status() != reqwest::StatusCode::NOT_FOUND
            || !self.headers.contains_key(AUTHORIZATION)
            || !json_type
        {
            return Err(error(ErrorCode::ProviderUnavailable));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            if bytes.len() + chunk.len() > 8192 {
                return Err(error(ErrorCode::FrameTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        let body = serde_json::from_slice::<Value>(&bytes)
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        if confirm_missing(&body) {
            Ok(())
        } else {
            Err(error(ErrorCode::ProviderUnavailable))
        }
    }

    pub async fn json(&self, path: &str, body: Option<Value>) -> Result<Value> {
        self.json_query(path, &[], body).await
    }
    pub async fn json_query(
        &self,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value> {
        self.json_query_limited(path, query, body, BODY_LIMIT).await
    }
    pub async fn json_query_limited(
        &self,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
        max_response_bytes: usize,
    ) -> Result<Value> {
        if max_response_bytes == 0 || max_response_bytes > MAX_METADATA_BODY_LIMIT {
            return Err(invalid());
        }
        let url = self.query_url(path, query)?;
        let mut response = self.send_url(url, body, false).await?;
        if response
            .content_length()
            .is_some_and(|n| n > max_response_bytes as u64)
        {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            if bytes.len() + chunk.len() > max_response_bytes {
                return Err(error(ErrorCode::FrameTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| error(ErrorCode::ProtocolIncompatible))
    }
    pub async fn stream(&self, path: &str, body: Value) -> Result<LineStream> {
        Ok(LineStream {
            response: self.send(path, Some(body), true).await?,
            pending: Vec::new(),
            total: 0,
            done: false,
        })
    }
}
pub struct LineStream {
    response: Response,
    pending: Vec<u8>,
    total: usize,
    done: bool,
}
impl LineStream {
    pub async fn line(&mut self) -> Result<Option<String>> {
        if self.done {
            return Ok(None);
        }
        let result = self.next_line().await;
        if result.is_err() {
            self.done = true;
            self.pending.clear();
        }
        result
    }
    async fn next_line(&mut self) -> Result<Option<String>> {
        loop {
            if let Some(index) = self.pending.iter().position(|b| *b == b'\n') {
                if index > LINE_LIMIT {
                    return Err(error(ErrorCode::FrameTooLarge));
                }
                let mut line = self.pending.drain(..=index).collect::<Vec<_>>();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return String::from_utf8(line)
                    .map(Some)
                    .map_err(|_| error(ErrorCode::ProtocolIncompatible));
            }
            if self.pending.len() > LINE_LIMIT {
                return Err(error(ErrorCode::FrameTooLarge));
            }
            match self.response.chunk().await.map_err(network)? {
                Some(chunk) => {
                    self.total = self.total.saturating_add(chunk.len());
                    if self.total > STREAM_LIMIT || self.pending.len() + chunk.len() > BODY_LIMIT {
                        return Err(error(ErrorCode::FrameTooLarge));
                    }
                    self.pending.extend_from_slice(&chunk);
                }
                None => {
                    self.done = true;
                    return if self.pending.is_empty() {
                        Ok(None)
                    } else {
                        Err(error(ErrorCode::ProtocolIncompatible))
                    };
                }
            }
        }
    }
}
