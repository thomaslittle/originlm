//! Authenticated loopback HTTP/SSE. Pairing is explicit, expiring, single-use, and host-scoped.
mod auth;
mod routes;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_service::{ExecutionGrant, NativeService};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{Semaphore, oneshot};
pub struct HttpOptions {
    pub port: u16,
    pub policy: ClientPolicy,
    pub execution: Option<ExecutionGrant>,
    pub assets: Option<PathBuf>,
}
pub(super) struct Token {
    pub client: String,
    pub created: Instant,
    pub ready: bool,
}
pub(super) struct Pair {
    code: String,
    hash: [u8; 32],
    created: Instant,
    used: bool,
    attempts: u8,
}
pub(super) struct Shared {
    pub service: Arc<NativeService>,
    pub url: String,
    pub authority: String,
    pub pair: Mutex<Pair>,
    pub tokens: Mutex<BTreeMap<[u8; 32], Token>>,
    pub options: HttpOptions,
    pub streams: Arc<Semaphore>,
}
pub struct HttpServer {
    pub url: String,
    pub pairing_code: String,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
    shared: Arc<Shared>,
}
impl HttpServer {
    pub async fn close(mut self) {
        let clients: Vec<_> = self
            .shared
            .tokens
            .lock()
            .map(|mut tokens| {
                let ids = tokens
                    .values()
                    .map(|t| t.client.clone())
                    .collect::<Vec<_>>();
                tokens.clear();
                ids
            })
            .unwrap_or_default();
        for client in clients {
            let _ = self
                .shared
                .service
                .handle(&client, "grants.revoke", serde_json::json!({}))
                .await;
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(mut task) = self.task.take()
            && tokio::time::timeout(Duration::from_secs(3), &mut task)
                .await
                .is_err()
        {
            task.abort();
        }
    }
    /// Host-side only: lets an embedding application authorize another explicit pairing.
    pub fn issue_pairing_code(&self) -> Result<String> {
        let code = random(6)?;
        let mut pair = self.shared.pair.lock().map_err(|_| error())?;
        *pair = Pair {
            code: code.clone(),
            hash: hash(&code),
            created: Instant::now(),
            used: false,
            attempts: 0,
        };
        Ok(code)
    }
}
impl Drop for HttpServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
pub(super) fn hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
pub(super) fn random(count: usize) -> Result<String> {
    let mut bytes = vec![0; count];
    getrandom::fill(&mut bytes).map_err(|_| error())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn error() -> DomainError {
    DomainError::new(
        ErrorCode::ProviderUnavailable,
        "Local HTTP runtime unavailable",
    )
}
pub async fn start(service: Arc<NativeService>, options: HttpOptions) -> Result<HttpServer> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port))
        .await
        .map_err(|_| error())?;
    let authority = listener.local_addr().map_err(|_| error())?.to_string();
    let url = format!("http://{authority}");
    let pairing_code = random(6)?;
    let shared = Arc::new(Shared {
        service,
        url: url.clone(),
        authority,
        pair: Mutex::new(Pair {
            code: pairing_code.clone(),
            hash: hash(&pairing_code),
            created: Instant::now(),
            used: false,
            attempts: 0,
        }),
        tokens: Mutex::new(BTreeMap::new()),
        options,
        streams: Arc::new(Semaphore::new(16)),
    });
    let app = Router::new()
        .route("/runtime/info", get(routes::info))
        .route(
            "/runtime/pairing-code",
            get(auth::pairing_code).post(auth::issue_pairing_code),
        )
        .route("/v1/pair", post(auth::pair))
        .route(
            "/v1/rpc",
            post(routes::rpc).layer(DefaultBodyLimit::max(
                originlm_contracts::MAX_REQUEST_FRAME_BYTES,
            )),
        )
        .route("/v1/events", get(routes::events))
        .route("/v1/snapshot", get(routes::snapshot))
        .route("/v1/connections", get(routes::connections))
        .route("/v1/models", get(routes::models))
        .route("/v1/usage", get(routes::usage))
        .route("/v1/diagnostics", get(routes::diagnostics))
        .fallback(routes::asset)
        .layer(DefaultBodyLimit::max(1_048_576))
        .layer(middleware::from_fn_with_state(
            shared.clone(),
            auth::boundary,
        ))
        .with_state(shared.clone());
    let (stop, receive) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = receive.await;
            })
            .await;
    });
    Ok(HttpServer {
        url,
        pairing_code,
        stop: Some(stop),
        task: Some(task),
        shared,
    })
}
