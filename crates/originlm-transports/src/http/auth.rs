use super::{Shared, Token, hash, random};
use axum::{
    Json,
    body::Bytes,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub(super) fn reject(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}
pub(super) async fn boundary(
    State(state): State<Arc<Shared>>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let host = headers.get("host").and_then(|h| h.to_str().ok());
    let origin = headers.get("origin").and_then(|h| h.to_str().ok());
    if host != Some(state.authority.as_str())
        || origin.is_some_and(|o| o != state.url)
        || headers
            .get("sec-fetch-site")
            .is_some_and(|h| h == "cross-site")
    {
        return reject(StatusCode::FORBIDDEN, "ORIGIN_DENIED");
    }
    if request.uri().query().is_some() && request.uri().path() != "/v1/events" {
        return reject(StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    }
    let mut response = match tokio::time::timeout(Duration::from_secs(30), next.run(request)).await
    {
        Ok(r) => r,
        Err(_) => reject(StatusCode::REQUEST_TIMEOUT, "TIMEOUT"),
    };
    for (key, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'",
        ),
    ] {
        response
            .headers_mut()
            .insert(key, HeaderValue::from_static(value));
    }
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairRequest {
    code: String,
}
pub(super) async fn pair(
    State(state): State<Arc<Shared>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !headers
        .get("content-type")
        .is_some_and(|h| h.as_bytes().starts_with(b"application/json"))
    {
        return reject(StatusCode::UNSUPPORTED_MEDIA_TYPE, "CONTENT_TYPE_REQUIRED");
    }
    let request: PairRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return reject(StatusCode::BAD_REQUEST, "INVALID_REQUEST"),
    };
    reap_expired(&state);
    let mut pairing = match state.pair.lock() {
        Ok(p) => p,
        Err(_) => return reject(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
    };
    if request.code.len() != 12
        || pairing.used
        || pairing.created.elapsed() > Duration::from_secs(300)
        || pairing.attempts >= 10
    {
        return reject(StatusCode::FORBIDDEN, "PAIRING_DENIED");
    }
    pairing.attempts += 1;
    if hash(&request.code) != pairing.hash {
        return reject(StatusCode::FORBIDDEN, "PAIRING_DENIED");
    }
    let token = match random(32) {
        Ok(t) => t,
        Err(_) => return reject(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
    };
    let client = format!(
        "http-{}",
        hash(&token)
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let mut tokens = match state.tokens.lock() {
        Ok(t) => t,
        Err(_) => return reject(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
    };
    if tokens.len() >= 16 {
        return reject(StatusCode::TOO_MANY_REQUESTS, "CLIENT_LIMIT");
    }
    if state
        .service
        .register_client(&client, state.options.policy.clone())
        .is_err()
    {
        return reject(StatusCode::SERVICE_UNAVAILABLE, "CLIENT_UNAVAILABLE");
    }
    // Every client paired with this helper acts for the one host application
    // that launched it; durable resume tokens follow that application.
    let _ = state.service.set_app_identity(&client, "http-host");
    if let Some(grant) = &state.options.execution
        && state
            .service
            .register_execution(&client, grant.clone())
            .is_err()
    {
        state.service.close_client(&client);
        return reject(StatusCode::SERVICE_UNAVAILABLE, "EXECUTION_UNAVAILABLE");
    }
    tokens.insert(
        hash(&token),
        Token {
            client,
            created: Instant::now(),
            ready: false,
        },
    );
    pairing.used = true;
    Json(json!({"token":token,"protocolVersion":"1.0","expiresInSeconds":3600})).into_response()
}
pub(super) async fn pairing_code(State(state): State<Arc<Shared>>) -> Response {
    let pairing = match state.pair.lock() {
        Ok(p) => p,
        Err(_) => return reject(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
    };
    let lifetime = Duration::from_secs(300).saturating_sub(pairing.created.elapsed());
    if pairing.used || lifetime.is_zero() || pairing.attempts >= 10 {
        return reject(StatusCode::FORBIDDEN, "PAIRING_DENIED");
    }
    let expires_in = lifetime.as_secs_f64().ceil() as u64;
    Json(json!({"code":pairing.code,"expiresInSeconds":expires_in})).into_response()
}
pub(super) async fn issue_pairing_code(State(state): State<Arc<Shared>>) -> Response {
    let code = match random(6) {
        Ok(code) => code,
        Err(_) => return reject(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
    };
    let mut pairing = match state.pair.lock() {
        Ok(p) => p,
        Err(_) => return reject(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
    };
    *pairing = super::Pair {
        code: code.clone(),
        hash: hash(&code),
        created: Instant::now(),
        used: false,
        attempts: 0,
    };
    Json(json!({"code":code,"expiresInSeconds":300})).into_response()
}
pub(super) fn authorize(
    state: &Shared,
    headers: &HeaderMap,
    require_ready: bool,
) -> Result<(String, [u8; 32]), Box<Response>> {
    reap_expired(state);
    if headers.get_all("authorization").iter().count() != 1 {
        return Err(auth_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED"));
    }
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| v.len() == 64)
        .ok_or_else(|| auth_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED"))?;
    let key = hash(token);
    let tokens = state
        .tokens
        .lock()
        .map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
    let entry = tokens
        .get(&key)
        .filter(|t| t.created.elapsed() < Duration::from_secs(3600))
        .ok_or_else(|| auth_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED"))?;
    if require_ready && !entry.ready {
        return Err(auth_error(StatusCode::CONFLICT, "HANDSHAKE_REQUIRED"));
    }
    Ok((entry.client.clone(), key))
}

fn reap_expired(state: &Shared) {
    let expired = if let Ok(mut tokens) = state.tokens.lock() {
        let ids = tokens
            .values()
            .filter(|t| t.created.elapsed() >= Duration::from_secs(3600))
            .map(|t| t.client.clone())
            .collect::<Vec<_>>();
        tokens.retain(|_, t| t.created.elapsed() < Duration::from_secs(3600));
        ids
    } else {
        Vec::new()
    };
    for client in expired {
        state.service.close_client(&client);
    }
}

fn auth_error(status: StatusCode, code: &str) -> Box<Response> {
    Box::new(reject(status, code))
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::http::{HttpOptions, start};
    use originlm_contracts::{DataMode, Snapshot};
    use originlm_core::ClientPolicy;
    use originlm_discovery::ScanResult;
    use originlm_service::NativeService;
    use std::collections::BTreeMap;
    #[tokio::test]
    async fn expiring_a_pairing_token_releases_only_that_client() {
        let service = Arc::new(NativeService::new(
            Arc::new(|_| {
                Ok(ScanResult {
                    snapshot: Snapshot::empty(DataMode::Live),
                    locations: BTreeMap::new(),
                    metadata_probes: 0,
                })
            }),
            Duration::from_secs(60),
        ));
        service
            .register_client("expired", ClientPolicy::passive())
            .unwrap();
        service
            .register_client("fresh", ClientPolicy::passive())
            .unwrap();
        let server = start(
            service.clone(),
            HttpOptions {
                port: 0,
                policy: ClientPolicy::passive(),
                execution: None,
                assets: None,
            },
        )
        .await
        .unwrap();
        let expired = "a".repeat(64);
        let fresh = "b".repeat(64);
        {
            let mut tokens = server.shared.tokens.lock().unwrap();
            tokens.insert(
                hash(&expired),
                Token {
                    client: "expired".into(),
                    created: Instant::now() - Duration::from_secs(3601),
                    ready: true,
                },
            );
            tokens.insert(
                hash(&fresh),
                Token {
                    client: "fresh".into(),
                    created: Instant::now(),
                    ready: true,
                },
            );
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {expired}").parse().unwrap(),
        );
        assert!(authorize(&server.shared, &headers, true).is_err());
        assert!(
            service
                .handle("expired", "snapshot.get", json!({}))
                .await
                .is_err()
        );
        assert!(
            service
                .handle("fresh", "snapshot.get", json!({}))
                .await
                .is_ok()
        );
        assert_eq!(server.shared.tokens.lock().unwrap().len(), 1);
        server.close().await;
    }
}
