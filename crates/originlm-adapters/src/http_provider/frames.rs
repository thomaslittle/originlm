use super::HttpKind;
use originlm_contracts::*;
use serde_json::Value;
pub struct Frame {
    pub text: String,
    pub done: bool,
    pub usage: Vec<(String, String)>,
}
fn fail() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Provider stream is invalid or interrupted",
    )
}
fn string(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(fail)
}
fn no_tools() -> DomainError {
    DomainError::new(
        ErrorCode::IntegrationRestricted,
        "Tools are disabled for this text-only HTTP route",
    )
}
fn usage(out: &mut Frame, value: &Value, keys: &[(&str, &str)]) {
    for (key, unit) in keys {
        if let Some(n) = value[*key].as_u64() {
            out.usage.push(((*unit).into(), n.to_string()));
        }
    }
}
pub fn decode(kind: HttpKind, v: &Value) -> Result<Frame> {
    if !v.is_object() || v.get("error").is_some() {
        return Err(fail());
    }
    let mut out = Frame {
        text: String::new(),
        done: false,
        usage: vec![],
    };
    match kind {
        HttpKind::Opencode => return Err(fail()),
        HttpKind::Ollama => {
            if v["message"]["tool_calls"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
            {
                return Err(no_tools());
            }
            if let Some(text) = v["message"].get("content") {
                out.text = string(text)?.into();
            }
            out.done = v["done"].as_bool().ok_or_else(fail)?;
            usage(
                &mut out,
                v,
                &[
                    ("prompt_eval_count", "input_tokens"),
                    ("eval_count", "output_tokens"),
                ],
            );
        }
        HttpKind::Openai | HttpKind::Xai => match v["type"].as_str() {
            Some("response.output_text.delta") => out.text = string(&v["delta"])?.into(),
            Some("response.completed") => {
                out.done = true;
                usage(
                    &mut out,
                    &v["response"]["usage"],
                    &[
                        ("input_tokens", "input_tokens"),
                        ("output_tokens", "output_tokens"),
                    ],
                );
            }
            Some("response.failed" | "response.incomplete" | "error") => return Err(fail()),
            Some("response.output_item.added") if v["item"]["type"] == "function_call" => {
                return Err(no_tools());
            }
            Some(_) => {}
            None => return Err(fail()),
        },
        HttpKind::Anthropic => match v["type"].as_str() {
            Some("content_block_delta") => {
                if v["delta"]["type"] == "text_delta" {
                    out.text = string(&v["delta"]["text"])?.into();
                }
            }
            Some("content_block_start") if v["content_block"]["type"] == "tool_use" => {
                return Err(no_tools());
            }
            Some("message_stop") => out.done = true,
            Some("message_start") => usage(
                &mut out,
                &v["message"]["usage"],
                &[("input_tokens", "input_tokens")],
            ),
            Some("message_delta") => {
                usage(&mut out, &v["usage"], &[("output_tokens", "output_tokens")])
            }
            Some("error") => return Err(fail()),
            Some(_) => {}
            None => return Err(fail()),
        },
        HttpKind::Gemini => {
            if v.get("promptFeedback")
                .is_some_and(|p| p.get("blockReason").is_some())
            {
                return Err(no_tools());
            }
            if let Some(candidates) = v["candidates"].as_array() {
                if candidates.len() > 1 {
                    return Err(fail());
                }
                for c in candidates {
                    if let Some(parts) = c["content"]["parts"].as_array() {
                        for p in parts {
                            if p.get("functionCall").is_some() {
                                return Err(no_tools());
                            }
                            if p["thought"] != true
                                && let Some(text) = p.get("text")
                            {
                                out.text.push_str(string(text)?);
                            }
                        }
                    }
                    if let Some(reason) = c["finishReason"].as_str() {
                        if !["STOP", "MAX_TOKENS"].contains(&reason) {
                            return Err(fail());
                        }
                        out.done = true;
                    }
                }
            }
            usage(
                &mut out,
                &v["usageMetadata"],
                &[
                    ("promptTokenCount", "input_tokens"),
                    ("candidatesTokenCount", "output_tokens"),
                ],
            );
        }
        HttpKind::Lmstudio | HttpKind::Compatible => {
            let choices = v["choices"].as_array().ok_or_else(fail)?;
            if choices.len() > 1 {
                return Err(fail());
            }
            for c in choices {
                if c["delta"].get("tool_calls").is_some() {
                    return Err(no_tools());
                }
                if let Some(text) = c["delta"].get("content").filter(|v| !v.is_null()) {
                    out.text.push_str(string(text)?);
                }
                if let Some(reason) = c["finish_reason"].as_str() {
                    if !["stop", "length"].contains(&reason) {
                        return Err(fail());
                    }
                    out.done = true;
                }
            }
            usage(
                &mut out,
                &v["usage"],
                &[
                    ("prompt_tokens", "input_tokens"),
                    ("completion_tokens", "output_tokens"),
                ],
            );
        }
    }
    if out.text.len() > 1024 * 1024 {
        return Err(fail());
    }
    Ok(out)
}
