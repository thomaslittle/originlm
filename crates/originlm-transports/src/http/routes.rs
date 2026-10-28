use super::{
    Shared,
    auth::{authorize, reject},
};
use crate::wire;
use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use originlm_contracts::ErrorCode;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc, time::Duration};
pub(super) async fn info() -> Json<Value> {
    Json(json!({"mode":"live","pairingRequired":true,"protocolVersion":"1.0"}))
}
fn bounded_json(value: Value) -> Response {
    match serde_json::to_vec(&value) {
        Ok(bytes) if bytes.len() <= 1_048_576 => {
            ([("content-type", "application/json")], bytes).into_response()
        }
        _ => reject(StatusCode::PAYLOAD_TOO_LARGE, "FRAME_TOO_LARGE"),
    }
}
pub(super) async fn rpc(
    State(state): State<Arc<Shared>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (client, key) = match authorize(&state, &headers, false) {
        Ok(c) => c,
        Err(e) => return *e,
    };
    if !headers
        .get("content-type")
        .is_some_and(|h| h.as_bytes().starts_with(b"application/json"))
    {
        return reject(StatusCode::UNSUPPORTED_MEDIA_TYPE, "CONTENT_TYPE_REQUIRED");
    }
    let request = match wire::parse(&body) {
        Ok(Some(r)) => r,
        Ok(None) => return StatusCode::NO_CONTENT.into_response(),
        Err(e) => return bounded_json(e),
    };
    if request.method != "runtime.handshake"
        && let Err(e) = authorize(&state, &headers, true)
    {
        return *e;
    }
    let outcome = state
        .service
        .handle(&client, &request.method, request.params)
        .await;
    if request.method == "runtime.handshake"
        && outcome.is_ok()
        && let Ok(mut tokens) = state.tokens.lock()
        && let Some(token) = tokens.get_mut(&key)
    {
        token.ready = true;
    }
    if ["grants.revoke", "runtime.closeClient"].contains(&request.method.as_str())
        && outcome.is_ok()
    {
        if let Ok(mut tokens) = state.tokens.lock() {
            tokens.remove(&key);
        }
        state.service.close_client(&client);
    }
    bounded_json(wire::reply(request.id, outcome))
}
async fn read(state: Arc<Shared>, headers: HeaderMap, method: &str) -> Response {
    let (client, _) = match authorize(&state, &headers, true) {
        Ok(c) => c,
        Err(e) => return *e,
    };
    match state.service.handle(&client, method, json!({})).await {
        Ok(value) => bounded_json(value),
        Err(_) => reject(StatusCode::FORBIDDEN, "ACCESS_DENIED"),
    }
}
pub(super) async fn snapshot(State(s): State<Arc<Shared>>, h: HeaderMap) -> Response {
    read(s, h, "snapshot.get").await
}
pub(super) async fn connections(State(s): State<Arc<Shared>>, h: HeaderMap) -> Response {
    read(s, h, "connections.list").await
}
pub(super) async fn models(State(s): State<Arc<Shared>>, h: HeaderMap) -> Response {
    read(s, h, "models.list").await
}
pub(super) async fn usage(State(s): State<Arc<Shared>>, h: HeaderMap) -> Response {
    read(s, h, "usage.read").await
}
pub(super) async fn diagnostics(State(s): State<Arc<Shared>>, h: HeaderMap) -> Response {
    read(s, h, "diagnostics.report").await
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EventQuery {
    scope: Option<String>,
    session_id: Option<String>,
    after: Option<String>,
}
pub(super) async fn events(
    State(state): State<Arc<Shared>>,
    headers: HeaderMap,
    query: Result<Query<EventQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(q) => q,
        Err(_) => return reject(StatusCode::BAD_REQUEST, "INVALID_REQUEST"),
    };
    if query
        .scope
        .as_deref()
        .is_some_and(|scope| scope != "inventory")
        || (query.scope.is_some() && query.session_id.is_some())
        || query.session_id.as_ref().is_some_and(|s| s.len() > 256)
        || query.after.as_ref().is_some_and(|s| {
            if query.scope.as_deref() == Some("inventory") {
                s.is_empty() || s.len() > 128 || s.chars().any(char::is_control)
            } else {
                s.len() > 20 || s.parse::<u64>().is_err()
            }
        })
    {
        return reject(StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    }
    let (client, _) = match authorize(&state, &headers, true) {
        Ok(c) => c,
        Err(e) => return *e,
    };
    let permit = match state.streams.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return reject(StatusCode::TOO_MANY_REQUESTS, "STREAM_LIMIT"),
    };
    let mut params = json!({});
    if let Some(scope) = query.scope {
        params["scope"] = json!(scope);
    }
    if let Some(id) = query.session_id {
        params["sessionId"] = json!(id);
    }
    if let Some(after) = query.after {
        params["after"] = json!(after);
    }
    let initial = match state
        .service
        .handle(&client, "events.read", params.clone())
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return reject(
                if e.code == ErrorCode::EventGap {
                    StatusCode::CONFLICT
                } else {
                    StatusCode::FORBIDDEN
                },
                "EVENTS_UNAVAILABLE",
            );
        }
    };
    let stream = async_stream::stream! {
     let _permit=permit;let mut page=initial;
     loop {
      if authorize(&state,&headers,true).is_err(){yield Ok::<_,Infallible>(Event::default().event("gap").data("{\"code\":\"AUTH_REQUIRED\"}"));break;}
      if let Some(events)=page["events"].as_array(){for event in events{let data=match serde_json::to_string(event){Ok(s)=>s,Err(_)=>break};yield Ok::<_,Infallible>(Event::default().event("update").id(event["sequence"].as_str().unwrap_or("0")).data(data));}}
      if let Some(cursor)=page["cursor"].as_str(){params["after"]=json!(cursor);}
      tokio::time::sleep(Duration::from_millis(75)).await;
      page=match state.service.handle(&client,"events.read",params.clone()).await{Ok(value)=>value,Err(_)=>{yield Ok::<_,Infallible>(Event::default().event("gap").data("{}"));break;}};
     }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}
/// Exact single-segment client-side routes of the demo SPA; each serves `index.html`.
const CLIENT_ROUTES: [&str; 8] = [
    "/overview",
    "/connections",
    "/models",
    "/usage",
    "/selector-lab",
    "/playground",
    "/diagnostics",
    "/settings",
];
pub(super) async fn asset(State(state): State<Arc<Shared>>, method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return reject(StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED");
    }
    let Some(root) = &state.options.assets else {
        return reject(StatusCode::NOT_FOUND, "NOT_FOUND");
    };
    let path = uri.path();
    let name = if path == "/" || CLIENT_ROUTES.contains(&path) {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };
    if name.len() > 256
        || name.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return reject(StatusCode::NOT_FOUND, "NOT_FOUND");
    }
    let extension = name.rsplit('.').next().unwrap_or("");
    let mime = match extension {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        _ => return reject(StatusCode::NOT_FOUND, "NOT_FOUND"),
    };
    let Ok(root) = tokio::fs::canonicalize(root).await else {
        return reject(StatusCode::NOT_FOUND, "ASSETS_UNAVAILABLE");
    };
    let Ok(file) = tokio::fs::canonicalize(root.join(name)).await else {
        return reject(StatusCode::NOT_FOUND, "NOT_FOUND");
    };
    if !file.starts_with(&root) {
        return reject(StatusCode::FORBIDDEN, "ACCESS_DENIED");
    }
    let Ok(metadata) = tokio::fs::metadata(&file).await else {
        return reject(StatusCode::NOT_FOUND, "NOT_FOUND");
    };
    if !metadata.is_file() || metadata.len() > 2_097_152 {
        return reject(StatusCode::NOT_FOUND, "NOT_FOUND");
    }
    if method == Method::HEAD {
        return ([("content-type", mime)], Vec::<u8>::new()).into_response();
    }
    use tokio::io::AsyncReadExt;
    let Ok(file) = tokio::fs::File::open(file).await else {
        return reject(StatusCode::NOT_FOUND, "NOT_FOUND");
    };
    let mut data = Vec::new();
    if file.take(2_097_153).read_to_end(&mut data).await.is_err() || data.len() > 2_097_152 {
        return reject(StatusCode::PAYLOAD_TOO_LARGE, "ASSET_TOO_LARGE");
    }
    ([("content-type", mime)], data).into_response()
}
