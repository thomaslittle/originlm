//! Request-shape fixtures for image turns over the HTTP provider adapters. No live provider is contacted.
use axum::{Json, Router, extract::State, http::Uri, routing::any};
use originlm_adapters::{
    http::{Authentication, Endpoint},
    http_provider::*,
};
use originlm_contracts::*;
use originlm_runtime::{ImageInput, Runtime};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
type Captured = Arc<Mutex<Vec<(String, Value)>>>;
const PIXELS: &[u8] = b"\x89PNG\r\n\x1a\nfixture";
const BASE64: &str = "iVBORw0KGgpmaXh0dXJl";
async fn reply(State(captured): State<Captured>, uri: Uri, Json(body): Json<Value>) -> String {
    let path = uri.path().to_owned();
    captured.lock().unwrap().push((path.clone(), body));
    if path == "/api/chat" {
        "{\"message\":{\"content\":\"ok\"},\"done\":true}\n".into()
    } else if path.ends_with("/chat/completions") {
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".into()
    } else if path.ends_with("/responses") {
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{}}}\n\n".into()
    } else if path.ends_with("/messages") {
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n".into()
    } else {
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}]}\n\n".into()
    }
}
fn model() -> Model {
    Model {
        id: "m".into(),
        route_id: "r".into(),
        native_id: "native-vision".into(),
        name: "Native vision".into(),
        revision: "1".into(),
        is_default: true,
        capabilities: vec![],
        options: vec![],
        context_window: Observation::unknown("NOT_REPORTED"),
    }
}
fn image() -> ImageInput {
    ImageInput {
        media_type: InputMediaType::Png,
        bytes: PIXELS.into(),
        width: 1,
        height: 1,
    }
}
/// Run an image turn followed by a text-only turn and return both captured request bodies.
async fn exchange(kind: HttpKind, base: &str) -> Vec<(String, Value)> {
    let captured: Captured = Arc::default();
    let app = Router::new()
        .fallback(any(reply))
        .with_state(captured.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let endpoint = Endpoint::new(
        &format!("http://127.0.0.1:{port}/{base}"),
        Authentication::None,
        true,
    )
    .unwrap();
    let runtime = Runtime::new();
    let agent = executor(kind, endpoint, model());
    assert!(agent.image_input());
    let session = runtime
        .insert(
            "app",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: "m".into(),
                revision: "1".into(),
                options: BTreeMap::new(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    for (index, images) in [vec![image(), image()], vec![]].into_iter().enumerate() {
        runtime
            .start_with_inputs(
                "app",
                &session.id,
                "describe".into(),
                format!("request-{index}"),
                images,
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while runtime.get("app", &session.id).unwrap().state == SessionState::Running {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            runtime.get("app", &session.id).unwrap().state,
            SessionState::Completed,
            "{kind:?}"
        );
    }
    runtime.close("app", &session.id).await.unwrap();
    task.abort();
    let bodies = captured.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2);
    bodies
}
fn data_url() -> String {
    format!("data:image/png;base64,{BASE64}")
}
#[test]
fn fixture_base64_matches_bytes() {
    assert_eq!(image().base64(), BASE64);
}
#[tokio::test]
async fn ollama_sends_images_beside_message_content() {
    let bodies = exchange(HttpKind::Ollama, "").await;
    assert_eq!(bodies[0].0, "/api/chat");
    assert_eq!(
        bodies[0].1["messages"],
        json!([{"role":"user","content":"describe","images":[BASE64,BASE64]}])
    );
    // Follow-up history is text-only: images are not resent or accumulated.
    assert_eq!(
        bodies[1].1["messages"][0],
        json!({"role":"user","content":"describe"})
    );
    assert!(!bodies[1].1.to_string().contains(BASE64));
}
#[tokio::test]
async fn chat_completions_routes_use_image_url_data_parts() {
    for (kind, base, path) in [
        (HttpKind::Lmstudio, "", "/v1/chat/completions"),
        (HttpKind::Compatible, "v1/", "/v1/chat/completions"),
    ] {
        let bodies = exchange(kind, base).await;
        assert_eq!(bodies[0].0, path);
        assert_eq!(
            bodies[0].1["messages"],
            json!([{"role":"user","content":[
                {"type":"text","text":"describe"},
                {"type":"image_url","image_url":{"url":data_url()}},
                {"type":"image_url","image_url":{"url":data_url()}}
            ]}])
        );
        assert!(!bodies[1].1.to_string().contains(BASE64));
    }
}
#[tokio::test]
async fn responses_api_routes_use_input_image_parts() {
    for kind in [HttpKind::Openai, HttpKind::Xai] {
        let bodies = exchange(kind, "").await;
        assert_eq!(bodies[0].0, "/responses");
        assert_eq!(
            bodies[0].1["input"],
            json!([{"role":"user","content":[
                {"type":"input_text","text":"describe"},
                {"type":"input_image","image_url":data_url()},
                {"type":"input_image","image_url":data_url()}
            ]}])
        );
        assert!(!bodies[1].1.to_string().contains(BASE64));
    }
}
#[tokio::test]
async fn anthropic_messages_use_base64_image_blocks() {
    let bodies = exchange(HttpKind::Anthropic, "").await;
    assert_eq!(bodies[0].0, "/messages");
    let block =
        json!({"type":"image","source":{"type":"base64","media_type":"image/png","data":BASE64}});
    assert_eq!(
        bodies[0].1["messages"],
        json!([{"role":"user","content":[block,block,{"type":"text","text":"describe"}]}])
    );
    assert!(!bodies[1].1.to_string().contains(BASE64));
}
#[tokio::test]
async fn gemini_uses_inline_data_parts() {
    let bodies = exchange(HttpKind::Gemini, "").await;
    assert_eq!(bodies[0].0, "/models/native-vision:streamGenerateContent");
    let part = json!({"inline_data":{"mime_type":"image/png","data":BASE64}});
    assert_eq!(
        bodies[0].1["contents"],
        json!([{"role":"user","parts":[{"text":"describe"},part,part]}])
    );
    assert_eq!(
        bodies[1].1["contents"][0],
        json!({"role":"user","parts":[{"text":"describe"}]})
    );
}
