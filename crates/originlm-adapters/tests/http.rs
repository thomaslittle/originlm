use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use originlm_adapters::http::*;
use originlm_contracts::ErrorCode;
use serde_json::json;
fn upstream_error() -> Json<serde_json::Value> {
    Json(json!({"error":{"message":"Authorization: Bearer secret-sentinel"}}))
}
async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let app=Router::new().route("/ok",get(||async{Json(json!({"ok":true}))})).route("/auth",get(|h:HeaderMap|async move{Json(json!({"authorized":h.get("authorization").is_some_and(|s|s=="Bearer synthetic-secret")}))})).route("/redirect",get(||async{(StatusCode::FOUND,[("location","http://example.invalid/steal")],"secret-sentinel")})).route("/denied",get(move||async{(StatusCode::UNAUTHORIZED,upstream_error())})).route("/limited",get(move||async{(StatusCode::TOO_MANY_REQUESTS,upstream_error())})).route("/limited-retry",get(move||async{(StatusCode::TOO_MANY_REQUESTS,[("retry-after","3")],upstream_error())})).route("/limited-retry-long",get(move||async{(StatusCode::TOO_MANY_REQUESTS,[("retry-after","18446744073709551615")],upstream_error())})).route("/large",get(||async{Json(json!({"body":"x".repeat(2_200_000)}))})).route("/stream",post(||async{([("content-type","application/x-ndjson")],"{\"text\":\"native\"}\n{\"done\":true}\n")})).route("/unfinished",post(||async{"{\"text\":\"truncated\"}"}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}/"), task)
}
#[test]
fn endpoints_reject_cleartext_remote_userinfo_and_query_credentials() {
    for url in [
        "http://example.com/",
        "https://user:pass@example.com/",
        "https://example.com/?key=secret",
        "ftp://example.com/",
        "http://127.0.0.1/#token",
    ] {
        assert!(Endpoint::new(url, Authentication::None, false).is_err());
    }
    assert!(Endpoint::new("https://example.com/", Authentication::None, true).is_err());
    assert!(Endpoint::new("https://example.com/v1/", Authentication::None, false).is_ok());
}
#[tokio::test]
async fn native_json_and_credentials_stay_bound_to_the_endpoint() {
    let (url, t) = server().await;
    let endpoint = Endpoint::new(
        &url,
        Authentication::Bearer("synthetic-secret".into()),
        true,
    )
    .unwrap();
    assert_eq!(
        endpoint.json("auth", None).await.unwrap()["authorized"],
        true
    );
    assert_eq!(endpoint.json("ok", None).await.unwrap()["ok"], true);
    t.abort();
}
#[tokio::test]
async fn redirects_are_not_followed_and_paths_cannot_escape() {
    let (url, t) = server().await;
    let endpoint = Endpoint::new(&url, Authentication::None, true).unwrap();
    assert_eq!(
        endpoint.json("redirect", None).await.unwrap_err().code,
        ErrorCode::IntegrationRestricted
    );
    for path in [
        "../ok",
        "/ok",
        "https://example.com/",
        "ok?token=foo",
        "%2e%2e/ok",
    ] {
        assert!(endpoint.json(path, None).await.is_err());
    }
    t.abort();
}
#[tokio::test]
async fn auth_rate_limits_and_large_bodies_return_safe_errors() {
    let (url, t) = server().await;
    let endpoint = Endpoint::new(&url, Authentication::None, true).unwrap();
    for (path, code) in [
        ("denied", ErrorCode::AuthRequired),
        ("limited", ErrorCode::RateLimited),
        ("large", ErrorCode::FrameTooLarge),
    ] {
        let error = endpoint.json(path, None).await.unwrap_err();
        assert_eq!(error.code, code);
        assert!(!error.to_string().contains("secret-sentinel"));
        if path != "large" {
            assert!(error.message.contains("Authorization: Bearer [REDACTED]"));
        }
    }
    let retry = endpoint.json("limited-retry", None).await.unwrap_err();
    assert_eq!(retry.code, ErrorCode::RateLimited);
    assert!(retry.retryable);
    assert_eq!(retry.retry_after_ms, Some(3000));
    let long_retry = endpoint.json("limited-retry-long", None).await.unwrap_err();
    assert_eq!(long_retry.retry_after_ms, Some(86_400_000));
    t.abort();
}
#[tokio::test]
async fn explicitly_bounded_provider_metadata_reads_allow_large_compressed_catalogs() {
    let (url, t) = server().await;
    let endpoint = Endpoint::new(&url, Authentication::None, true).unwrap();
    let result = endpoint
        .json_query_limited("large", &[], None, 3 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(result["body"].as_str().unwrap().len(), 2_200_000);
    assert_eq!(
        endpoint.json("large", None).await.unwrap_err().code,
        ErrorCode::FrameTooLarge
    );
    t.abort();
}
#[tokio::test]
async fn line_stream_preserves_frames_and_rejects_truncated_output() {
    let (url, t) = server().await;
    let endpoint = Endpoint::new(&url, Authentication::None, true).unwrap();
    let mut stream = endpoint.stream("stream", json!({})).await.unwrap();
    assert!(stream.line().await.unwrap().unwrap().contains("native"));
    assert!(stream.line().await.unwrap().unwrap().contains("done"));
    assert!(stream.line().await.unwrap().is_none());
    let mut stream = endpoint.stream("unfinished", json!({})).await.unwrap();
    assert_eq!(
        stream.line().await.unwrap_err().code,
        ErrorCode::ProtocolIncompatible
    );
    t.abort();
}
