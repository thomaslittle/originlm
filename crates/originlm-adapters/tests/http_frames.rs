use originlm_adapters::http_provider::{HttpKind, frames::decode};
use serde_json::json;
#[test]
fn provider_frames_preserve_text_and_native_terminal_signals() {
    for (kind, data, text, done) in [
        (
            HttpKind::Ollama,
            json!({"message":{"content":"hello"},"done":false}),
            "hello",
            false,
        ),
        (
            HttpKind::Ollama,
            json!({"message":{"content":""},"done":true,"eval_count":3}),
            "",
            true,
        ),
        (
            HttpKind::Lmstudio,
            json!({"choices":[{"delta":{"content":"hello"},"finish_reason":null}]}),
            "hello",
            false,
        ),
        (
            HttpKind::Compatible,
            json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
            "",
            true,
        ),
        (
            HttpKind::Openai,
            json!({"type":"response.output_text.delta","delta":"hello"}),
            "hello",
            false,
        ),
        (
            HttpKind::Xai,
            json!({"type":"response.completed","response":{"usage":{"output_tokens":2}}}),
            "",
            true,
        ),
        (
            HttpKind::Anthropic,
            json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}),
            "hello",
            false,
        ),
        (
            HttpKind::Anthropic,
            json!({"type":"message_stop"}),
            "",
            true,
        ),
        (
            HttpKind::Gemini,
            json!({"candidates":[{"content":{"parts":[{"text":"private","thought":true},{"text":"hello"}]},"finishReason":"STOP"}]}),
            "hello",
            true,
        ),
    ] {
        let frame = decode(kind, &data).unwrap();
        assert_eq!(frame.text, text);
        assert_eq!(frame.done, done);
    }
}
#[test]
fn provider_errors_do_not_leak_error_bodies_or_execute_tools() {
    for kind in [
        HttpKind::Ollama,
        HttpKind::Compatible,
        HttpKind::Openai,
        HttpKind::Anthropic,
        HttpKind::Gemini,
    ] {
        let e = decode(kind, &json!({"error":"secret-sentinel"}))
            .err()
            .unwrap();
        assert!(!e.to_string().contains("secret-sentinel"));
    }
    assert!(
        decode(
            HttpKind::Compatible,
            &json!({"choices":[{"delta":{"tool_calls":[{}]},"finish_reason":null}]})
        )
        .is_err()
    );
    assert!(
        decode(
            HttpKind::Anthropic,
            &json!({"type":"content_block_start","content_block":{"type":"tool_use"}})
        )
        .is_err()
    );
    assert!(
        decode(
            HttpKind::Openai,
            &json!({"type":"response.failed","response":{"error":{"message":"secret"}}})
        )
        .is_err()
    );
}
