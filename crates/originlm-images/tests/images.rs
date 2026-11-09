use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use originlm_images::mcp::ImageServer;
use originlm_images::*;
use serde_json::{Value, json};
use std::{
    io::Cursor,
    sync::{Arc, Mutex},
};

fn png() -> Vec<u8> {
    let image = image::DynamicImage::new_rgb8(96, 64);
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}
fn request() -> GenerateRequest {
    GenerateRequest {
        prompt: "Starting Soon backdrop for a calm midnight collection".into(),
        size: Some(Canvas {
            width: 160,
            height: 90,
        }),
        aspect: None,
        style: Some("ink and moonlight".into()),
        reference_image: None,
        count: 1,
    }
}
struct Fake {
    images: Vec<Vec<u8>>,
    seen: Arc<Mutex<Vec<ProviderRequest>>>,
}
impl ImageProvider for Fake {
    fn provider(&self) -> &str {
        "mock"
    }
    fn model(&self) -> &str {
        "mock-image"
    }
    fn generate<'a>(&'a self, r: ProviderRequest) -> Generation<'a> {
        self.seen.lock().unwrap().push(r);
        Box::pin(async { Ok(self.images.clone()) })
    }
}
fn tool(
    root: &tempfile::TempDir,
    images: Vec<Vec<u8>>,
) -> (ImageTool, Arc<Mutex<Vec<ProviderRequest>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let provider = Arc::new(Fake {
        images,
        seen: seen.clone(),
    });
    (
        ImageTool::new(Some(provider), root.path().to_path_buf(), Canvas::default()).unwrap(),
        seen,
    )
}
#[tokio::test]
async fn tool_saves_exact_canvas_metadata_and_context_and_scoped_reference() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("reference.png"), png()).unwrap();
    let (tool, seen) = tool(&root, vec![png(), png()]);
    let mut r = request();
    r.count = 2;
    r.reference_image = Some("reference.png".into());
    let result = tool.generate(r).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_ne!(result[0].path, result[1].path);
    for file in result {
        assert!(file.path.starts_with(root.path().canonicalize().unwrap()));
        assert_eq!((file.width, file.height), (160, 90));
        assert_eq!((file.original_width, file.original_height), (96, 64));
        assert_eq!(file.model, "mock-image");
        assert_eq!(image::image_dimensions(file.path).unwrap(), (160, 90));
    }
    let seen = seen.lock().unwrap();
    assert!(seen[0].prompt.contains("ink and moonlight"));
    assert!(seen[0].prompt.contains("160×90"));
    assert!(seen[0].reference_png.is_some());
}
#[tokio::test]
async fn tool_validation_happens_before_provider_and_no_partial_files_escape() {
    let root = tempfile::tempdir().unwrap();
    let (tool, seen) = tool(&root, vec![png(), b"not an image".to_vec()]);
    for count in [0, 5] {
        let mut r = request();
        r.count = count;
        assert_eq!(
            tool.generate(r).await.unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }
    for path in ["../outside.png", "/outside.png", "C:\\outside.png"] {
        let mut r = request();
        r.reference_image = Some(path.into());
        assert_eq!(
            tool.generate(r).await.unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }
    let mut r = request();
    r.aspect = Some("1:0".into());
    assert_eq!(
        tool.generate(r).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    let mut r = request();
    r.aspect = Some("1:1".into());
    assert_eq!(
        tool.generate(r).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    let mut r = request();
    r.prompt = " ".into();
    assert_eq!(
        tool.generate(r).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    let mut r = request();
    r.size = Some(Canvas {
        width: 4097,
        height: 1080,
    });
    assert_eq!(
        tool.generate(r).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    assert!(seen.lock().unwrap().is_empty());
    let mut r = request();
    r.count = 2;
    assert_eq!(
        tool.generate(r).await.unwrap_err().kind,
        ErrorKind::ProviderFailure
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
#[tokio::test]
async fn missing_provider_is_typed_and_aspect_uses_host_canvas() {
    let root = tempfile::tempdir().unwrap();
    let missing = ImageTool::new(None, root.path().to_path_buf(), Canvas::default()).unwrap();
    assert_eq!(
        missing.generate(request()).await.unwrap_err().kind,
        ErrorKind::NoProvider
    );
    let (tool, _) = tool(&root, vec![png()]);
    let mut r = request();
    r.size = None;
    r.aspect = Some("4:3".into());
    let result = tool.generate(r).await.unwrap();
    assert_eq!((result[0].width, result[0].height), (1920, 1440));
}

type Capture = Arc<Mutex<Vec<(String, HeaderMap, Vec<u8>)>>>;
#[derive(Clone)]
struct Mock {
    status: StatusCode,
    value: Value,
    capture: Capture,
}
async fn reply(
    State(state): State<Mock>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, axum::Json<Value>) {
    state
        .capture
        .lock()
        .unwrap()
        .push((uri.path().into(), headers, body.to_vec()));
    (state.status, axum::Json(state.value))
}
async fn api(
    kind: ProviderKind,
    status: StatusCode,
    value: Value,
) -> (ApiProvider, Capture, tokio::task::JoinHandle<()>) {
    api_model(kind, status, value, None).await
}
async fn api_model(
    kind: ProviderKind,
    status: StatusCode,
    value: Value,
    model: Option<&str>,
) -> (ApiProvider, Capture, tokio::task::JoinHandle<()>) {
    let capture: Capture = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new().fallback(reply).with_state(Mock {
        status,
        value,
        capture: capture.clone(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let mut config = RouteConfig::standard(kind);
    config.model = model.map(str::to_owned);
    config.endpoint.base_url = format!("http://{address}/v1/");
    (
        config
            .with_credential("mock-provider-credential".into())
            .unwrap(),
        capture,
        task,
    )
}
fn provider_request() -> ProviderRequest {
    ProviderRequest {
        prompt: "Generate a backdrop".into(),
        canvas: Canvas {
            width: 1920,
            height: 1080,
        },
        count: 1,
        reference_png: None,
    }
}
#[tokio::test]
async fn openai_generation_and_multipart_edit_use_corrected_default_and_explicit_model() {
    let (provider, capture, task) = api(
        ProviderKind::Openai,
        StatusCode::OK,
        json!({"data":[{"b64_json":STANDARD.encode(png())}]}),
    )
    .await;
    assert_eq!(provider.model(), "gpt-image-2.6-flare");
    provider.generate(provider_request()).await.unwrap();
    let mut r = provider_request();
    r.reference_png = Some(png());
    provider.generate(r).await.unwrap();
    let capture = capture.lock().unwrap();
    assert_eq!(capture[0].0, "/v1/images/generations");
    let body: Value = serde_json::from_slice(&capture[0].2).unwrap();
    assert_eq!(body["model"], "gpt-image-2.6-flare");
    assert_eq!(body["size"], "1920x1080");
    assert_eq!(capture[1].0, "/v1/images/edits");
    assert!(
        capture[1].1["content-type"]
            .to_str()
            .unwrap()
            .starts_with("multipart/form-data")
    );
    assert!(String::from_utf8_lossy(&capture[1].2).contains("name=\"image\""));
    assert_eq!(capture.len(), 2);
    task.abort();
    let mut config = RouteConfig::standard(ProviderKind::Openai);
    config.model = Some("gpt-image-2.6-sunburst".into());
    assert_eq!(
        config.with_credential("mock".into()).unwrap().model(),
        "gpt-image-2.6-sunburst"
    );
    config.model = Some("gpt-image-1".into());
    assert!(config.with_credential("mock".into()).is_err());
}
#[tokio::test]
async fn gemini_and_openrouter_send_image_modalities_and_reference() {
    let response = json!({"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"image/png","data":STANDARD.encode(png())}}]}}]});
    let (provider, capture, task) = api(ProviderKind::Gemini, StatusCode::OK, response).await;
    let mut r = provider_request();
    r.count = 2;
    r.reference_png = Some(png());
    assert_eq!(provider.generate(r).await.unwrap().len(), 2);
    let capture = capture.lock().unwrap();
    assert_eq!(capture.len(), 2);
    assert_eq!(
        capture[0].0,
        "/v1/models/gemini-nano-banana-2.1:generateContent"
    );
    assert!(capture[0].1.contains_key("x-goog-api-key"));
    let body: Value = serde_json::from_slice(&capture[0].2).unwrap();
    assert_eq!(
        body["generationConfig"]["imageConfig"]["aspectRatio"],
        "16:9"
    );
    assert!(body["contents"][0]["parts"][1]["inlineData"]["data"].is_string());
    task.abort();
    let response = json!({"choices":[{"message":{"images":[{"image_url":{"url":format!("data:image/png;base64,{}",STANDARD.encode(png()))}}]}}]});
    let (provider, capture, task) = api(ProviderKind::Openrouter, StatusCode::OK, response).await;
    let mut r = provider_request();
    r.reference_png = Some(png());
    provider.generate(r).await.unwrap();
    let capture = capture.lock().unwrap();
    let body: Value = serde_json::from_slice(&capture[0].2).unwrap();
    assert_eq!(capture[0].0, "/v1/chat/completions");
    assert_eq!(body["model"], "google/gemini-nano-banana-2.1");
    assert_eq!(body["modalities"], json!(["image", "text"]));
    assert!(
        body["messages"][0]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    task.abort();
}
#[tokio::test]
async fn provider_failures_are_typed_redacted_and_never_retried() {
    for (status, code, kind) in [
        (429, "insufficient_quota", ErrorKind::QuotaBilling),
        (400, "content_policy_violation", ErrorKind::ContentRefusal),
        (500, "unknown", ErrorKind::ProviderFailure),
        (401, "invalid_api_key", ErrorKind::NoProvider),
    ] {
        let (provider, capture, task) = api(
            ProviderKind::Openai,
            StatusCode::from_u16(status).unwrap(),
            json!({"error":{"code":code,"message":"sensitive upstream prose"}}),
        )
        .await;
        let error = provider.generate(provider_request()).await.unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(!error.message.contains("sensitive"));
        assert_eq!(capture.lock().unwrap().len(), 1);
        task.abort();
    }
    let (provider, _, task) = api(
        ProviderKind::Gemini,
        StatusCode::OK,
        json!({"promptFeedback":{"blockReason":"SAFETY"}}),
    )
    .await;
    assert_eq!(
        provider
            .generate(provider_request())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::ContentRefusal
    );
    task.abort();
    let (provider, _, task) = api(
        ProviderKind::Openai,
        StatusCode::OK,
        json!({"data":[{"b64_json":"malformed"}]}),
    )
    .await;
    assert_eq!(
        provider
            .generate(provider_request())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::ProviderFailure
    );
    task.abort();
}
#[tokio::test]
async fn mcp_ask_to_image_returns_preview_and_trusted_host_file() {
    let root = tempfile::tempdir().unwrap();
    let (tool, _) = tool(&root, vec![png()]);
    let server = ImageServer::start(tool).await.unwrap();
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .post(&server.descriptor.url)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let call = |body: Value| {
        client
            .post(&server.descriptor.url)
            .bearer_auth(&server.descriptor.bearer_token)
            .json(&body)
    };
    let init: Value = call(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(init["result"]["capabilities"]["tools"], json!({}));
    let list: Value = call(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["result"]["tools"][0]["name"], "generate_image");
    let result:Value=call(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"generate_image","arguments":request()}})).send().await.unwrap().json().await.unwrap();
    assert_eq!(result["result"]["isError"], false);
    assert_eq!(result["result"]["content"][1]["type"], "image");
    let images = server.results.try_recv().unwrap();
    assert_eq!(images.len(), 1);
    assert!(images[0].path.is_file());
    assert_eq!(
        result["result"]["structuredContent"]["images"][0]["width"],
        160
    );
    assert_eq!(
        client
            .post(&server.descriptor.url)
            .bearer_auth(&server.descriptor.bearer_token)
            .header("Origin", "https://example.com")
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}

#[tokio::test]
async fn all_corrected_image_models_reach_the_wire_without_substitution() {
    for (kind, configured, expected) in [
        (ProviderKind::Openai, None, "gpt-image-2.6-flare"),
        (
            ProviderKind::Openai,
            Some("gpt-image-2.6-sunburst"),
            "gpt-image-2.6-sunburst",
        ),
        (ProviderKind::Gemini, None, "gemini-nano-banana-2.1"),
        (
            ProviderKind::Gemini,
            Some("gemini-3-pro-image"),
            "gemini-3-pro-image",
        ),
    ] {
        let response = if kind == ProviderKind::Openai {
            json!({"data":[{"b64_json":STANDARD.encode(png())}]})
        } else {
            json!({"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"image/png","data":STANDARD.encode(png())}}]}}]})
        };
        let (provider, capture, task) = api_model(kind, StatusCode::OK, response, configured).await;
        assert_eq!(provider.model(), expected);
        assert_eq!(
            provider.generate(provider_request()).await.unwrap().len(),
            1
        );
        let captured = capture.lock().unwrap();
        assert_eq!(captured.len(), 1, "no retries or fallback");
        if kind == ProviderKind::Openai {
            let body: Value = serde_json::from_slice(&captured[0].2).unwrap();
            assert_eq!(body["model"], expected);
        } else {
            assert_eq!(
                captured[0].0,
                format!("/v1/models/{expected}:generateContent")
            );
        }
        task.abort();
    }
}
