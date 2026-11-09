//! Authenticated, stateless streamable-HTTP MCP bridge for native agents.
//! The host retains this handle until its agent session closes.
use crate::*;
use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use originlm_contracts::HostMcpServer;
use std::sync::mpsc::{Receiver, Sender};
use tokio::sync::Semaphore;

struct StateData {
    tool: ImageTool,
    token: String,
    events: Sender<Vec<GeneratedImage>>,
    gate: Semaphore,
    stopped: tokio::sync::watch::Receiver<bool>,
}
pub struct ImageServer {
    pub descriptor: HostMcpServer,
    /// Trusted output channel. Hosts should render these paths, never parse
    /// arbitrary local paths from model prose.
    pub results: Receiver<Vec<GeneratedImage>>,
    task: tokio::task::JoinHandle<()>,
    stop: tokio::sync::watch::Sender<bool>,
}
impl Drop for ImageServer {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.task.abort();
    }
}
impl ImageServer {
    pub async fn start(tool: ImageTool) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
        let address = listener
            .local_addr()
            .map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|_| failed())?;
        let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let descriptor = HostMcpServer {
            name: "originlm_images".into(),
            url: format!("http://{address}/mcp"),
            bearer_token: token.clone(),
            allowed_tools: vec!["generate_image".into()],
        };
        let (events, results) = std::sync::mpsc::channel();
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let state = Arc::new(StateData {
            tool,
            token,
            events,
            gate: Semaphore::new(1),
            stopped,
        });
        let router = Router::new()
            .route("/mcp", post(handle))
            .layer(DefaultBodyLimit::max(32 * 1024))
            .with_state(state);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self {
            descriptor,
            results,
            task,
            stop,
        })
    }
}
fn authenticated(headers: &HeaderMap, token: &str) -> bool {
    let Some(value) = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    else {
        return false;
    };
    value.len() == token.len()
        && value
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}
fn rpc_error(id: Value, code: i64, message: &str) -> Response {
    axum::Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}))
        .into_response()
}
async fn handle(
    State(state): State<Arc<StateData>>,
    headers: HeaderMap,
    axum::Json(value): axum::Json<Value>,
) -> Response {
    if !authenticated(&headers, &state.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    // No browser origin is needed for native agent calls. Fail closed against
    // cross-site requests even if a browser somehow learns a local endpoint.
    if headers.contains_key("origin") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    if value["jsonrpc"] != "2.0" {
        return rpc_error(id, -32600, "Invalid JSON-RPC request");
    }
    let method = value["method"].as_str().unwrap_or("");
    if value.get("id").is_none() {
        return if method.starts_with("notifications/") {
            StatusCode::ACCEPTED.into_response()
        } else {
            rpc_error(id, -32600, "Request id required")
        };
    }
    let result = match method {
        "initialize" => {
            json!({"protocolVersion": "2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"originlm-images","version":env!("CARGO_PKG_VERSION")}})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools":[ImageTool::definition()]}),
        "tools/call" => {
            if value["params"]["name"] != "generate_image" {
                return rpc_error(id, -32602, "Unknown image tool");
            }
            let request: GenerateRequest =
                match serde_json::from_value(value["params"]["arguments"].clone()) {
                    Ok(r) => r,
                    Err(_) => return rpc_error(id, -32602, "Invalid image arguments"),
                };
            let Ok(_permit) = state.gate.try_acquire() else {
                return rpc_error(id, -32000, "An image generation is already running");
            };
            let mut stopped = state.stopped.clone();
            if *stopped.borrow() {
                return rpc_error(id, -32000, "Image session closed");
            }
            let generated = tokio::select! {
                biased;
                _ = stopped.changed() => return rpc_error(id,-32000,"Image session closed"),
                result = state.tool.generate(request) => result,
            };
            match generated {
                Ok(images) => {
                    let mut content = vec![
                        json!({"type":"text","text":serde_json::to_string(&images).unwrap_or_default()}),
                    ];
                    // Small visual previews for the calling model; full-resolution
                    // files go only over the trusted host result channel.
                    for image in &images {
                        if let Ok(bytes) = std::fs::read(&image.path)
                            && let Ok(preview) = decode(&bytes)
                        {
                            let mut png = Cursor::new(Vec::new());
                            if preview
                                .thumbnail(512, 512)
                                .write_to(&mut png, ImageFormat::Png)
                                .is_ok()
                            {
                                use base64::Engine;
                                content.push(json!({"type":"image","mimeType":"image/png","data":base64::engine::general_purpose::STANDARD.encode(png.get_ref())}));
                            }
                        }
                    }
                    let _ = state.events.send(images.clone());
                    json!({"content":content,"structuredContent":{"images":images},"isError":false})
                }
                Err(error) => {
                    json!({"content":[{"type":"text","text":error.message}],"structuredContent":{"error":error},"isError":true})
                }
            }
        }
        _ => return rpc_error(id, -32601, "Method not found"),
    };
    axum::Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
}
