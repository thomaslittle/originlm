use super::{HttpKind, frames};
use crate::http::Endpoint;
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, ImageInput, TurnContext};
use serde_json::{Value, json};
use std::collections::BTreeMap;
struct Chat {
    kind: HttpKind,
    endpoint: Endpoint,
    model: Model,
    history: Vec<Value>,
    closed: bool,
}
pub fn executor(kind: HttpKind, endpoint: Endpoint, model: Model) -> Box<dyn Agent> {
    Box::new(Chat {
        kind,
        endpoint: if kind == HttpKind::Anthropic {
            endpoint.anthropic_version()
        } else {
            endpoint
        },
        model,
        history: vec![],
        closed: false,
    })
}
/// The current user message in each provider's documented multimodal shape.
/// Text-only turns keep the plain string form every route already used.
fn user_message(kind: HttpKind, prompt: &str, images: &[ImageInput]) -> Value {
    if images.is_empty() {
        return json!({"role":"user","content":prompt});
    }
    match kind {
        // Ollama /api/chat carries raw base64 images beside the text content.
        HttpKind::Ollama => {
            json!({"role":"user","content":prompt,"images":images.iter().map(ImageInput::base64).collect::<Vec<_>>()})
        }
        // OpenAI Responses API input content parts.
        HttpKind::Openai | HttpKind::Xai => {
            let mut content = vec![json!({"type":"input_text","text":prompt})];
            content.extend(
                images
                    .iter()
                    .map(|image| json!({"type":"input_image","image_url":image.data_url()})),
            );
            json!({"role":"user","content":content})
        }
        HttpKind::Anthropic => {
            let mut content = images
                .iter()
                .map(|image| json!({"type":"image","source":{"type":"base64","media_type":image.media_type.as_str(),"data":image.base64()}}))
                .collect::<Vec<_>>();
            content.push(json!({"type":"text","text":prompt}));
            json!({"role":"user","content":content})
        }
        // Gemini parts are assembled in the request body from this neutral form.
        HttpKind::Gemini => {
            let mut parts = vec![json!({"text":prompt})];
            parts.extend(images.iter().map(
                |image| json!({"inline_data":{"mime_type":image.media_type.as_str(),"data":image.base64()}}),
            ));
            json!({"role":"user","parts":parts})
        }
        // LM Studio and compatible endpoints use Chat Completions image_url parts.
        _ => {
            let mut content = vec![json!({"type":"text","text":prompt})];
            content.extend(
                images
                    .iter()
                    .map(|image| json!({"type":"image_url","image_url":{"url":image.data_url()}})),
            );
            json!({"role":"user","content":content})
        }
    }
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "HTTP text stream was not completed",
    )
}
impl Chat {
    async fn run(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        if self.closed {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                "HTTP conversation is closed",
            ));
        }
        for (key, value) in &options {
            self.model
                .options
                .iter()
                .find(|o| o.id == *key)
                .ok_or_else(invalid)?
                .validate(value)
                .map_err(|_| invalid())?;
        }
        if self.history.len() > 62 {
            return Err(DomainError::new(
                ErrorCode::FrameTooLarge,
                "Conversation history limit reached",
            ));
        }
        let mut history = self.history.clone();
        history.push(json!({"role":"user","content":prompt}));
        if serde_json::to_vec(&history).map_err(|_| invalid())?.len() > 1024 * 1024 {
            return Err(DomainError::new(
                ErrorCode::FrameTooLarge,
                "Conversation byte limit reached",
            ));
        }
        // Images belong to the turn that attached them. Retained history stays text-only so
        // follow-up turns neither resend nor accumulate image payloads.
        let mut request = history.clone();
        if !cx.images().is_empty()
            && let Some(last) = request.last_mut()
        {
            *last = user_message(self.kind, &prompt, cx.images());
        }
        let (path, body) = match self.kind {
            HttpKind::Opencode => return Err(invalid()),
            HttpKind::Ollama => (
                "api/chat".into(),
                json!({"model":self.model.native_id,"messages":request,"stream":true}),
            ),
            HttpKind::Openai | HttpKind::Xai => (
                "responses".into(),
                json!({"model":self.model.native_id,"input":request,"stream":true,"store":false,"max_output_tokens":1024}),
            ),
            HttpKind::Anthropic => {
                let mut body = json!({"model":self.model.native_id,"messages":request,"stream":true,"max_tokens":1024});
                if let Some(effort) = options.get("effort") {
                    body["output_config"] = json!({"effort":effort});
                }
                ("messages".into(), body)
            }
            HttpKind::Gemini => {
                let name = self
                    .model
                    .native_id
                    .strip_prefix("models/")
                    .unwrap_or(&self.model.native_id);
                if name.is_empty()
                    || name
                        .bytes()
                        .any(|c| !c.is_ascii_alphanumeric() && !b"._-".contains(&c))
                {
                    return Err(invalid());
                }
                let contents=request.iter().map(|v|match v.get("parts"){Some(parts)=>json!({"role":"user","parts":parts}),None=>json!({"role":if v["role"]=="assistant"{"model"}else{"user"},"parts":[{"text":v["content"]}]})}).collect::<Vec<_>>();
                (
                    format!("models/{name}:streamGenerateContent"),
                    json!({"contents":contents,"generationConfig":{"maxOutputTokens":1024}}),
                )
            }
            _ => (
                "v1/chat/completions".into(),
                json!({"model":self.model.native_id,"messages":request,"stream":true}),
            ),
        };
        // Compatible endpoints use a base ending in /v1/, LM Studio uses its root for native catalogs.
        let path = if self.kind == HttpKind::Compatible {
            "chat/completions".to_owned()
        } else {
            path
        };
        let query = if self.kind == HttpKind::Gemini {
            vec![("alt", "sse")]
        } else {
            vec![]
        };
        let mut stream = tokio::select! {r=self.endpoint.stream_query(&path,&query,body)=>r?,_=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"HTTP request cancelled"))};
        let mut reply = String::new();
        let mut terminal = false;
        let mut data = String::new();
        loop {
            let line = tokio::select! {r=stream.line()=>r?,_=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"HTTP request cancelled; remote compute may already have occurred"))};
            let Some(line) = line else {
                break;
            };
            let packet = if self.kind == HttpKind::Ollama {
                Some(line)
            } else if line.is_empty() {
                if data.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut data))
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
                if data.len() > 1024 * 1024 {
                    return Err(invalid());
                }
                None
            } else {
                None
            };
            let Some(packet) = packet else {
                continue;
            };
            if packet == "[DONE]" {
                if !terminal {
                    return Err(invalid());
                }
                break;
            }
            let value = serde_json::from_str(&packet).map_err(|_| invalid())?;
            let frame = frames::decode(self.kind, &value)?;
            // OpenRouter reports the charge in USD credits in the final SSE usage frame.
            // Generic compatible endpoints may use a different currency or meaning.
            if self.kind == HttpKind::Compatible
                && self.endpoint.reports_usd_cost()
                && let Some(cost) = value
                    .pointer("/usage/cost")
                    .and_then(serde_json::Value::as_number)
                && let Some(amount) = DecimalString::from_json_number(cost)
            {
                cx.emit(EventPayload::Usage {
                    unit: "cost_usd".into(),
                    used: amount.into(),
                })?;
            }
            if terminal && !frame.text.is_empty() {
                return Err(invalid());
            }
            if reply.len() + frame.text.len() > 1024 * 1024 {
                return Err(invalid());
            }
            reply.push_str(&frame.text);
            let mut chunk = String::new();
            for ch in frame.text.chars() {
                chunk.push(ch);
                if chunk.len() >= 8192 {
                    cx.emit(EventPayload::TextDelta {
                        text: std::mem::take(&mut chunk),
                    })?;
                }
            }
            if !chunk.is_empty() {
                cx.emit(EventPayload::TextDelta { text: chunk })?;
            }
            for (unit, used) in frame.usage {
                cx.emit(EventPayload::Usage { unit, used })?;
            }
            terminal |= frame.done;
            if frame.done
                && matches!(
                    self.kind,
                    HttpKind::Ollama
                        | HttpKind::Openai
                        | HttpKind::Xai
                        | HttpKind::Anthropic
                        | HttpKind::Gemini
                )
            {
                break;
            }
        }
        if !terminal || !data.is_empty() {
            return Err(invalid());
        }
        history.push(json!({"role":"assistant","content":reply}));
        self.history = history;
        Ok(())
    }
}
impl Agent for Chat {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let result = self.run(prompt, options, cx).await;
            if result.is_err() {
                self.closed = true;
            }
            result
        })
    }
    fn image_input(&self) -> bool {
        self.kind != HttpKind::Opencode
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.history.clear();
            self.closed = true;
            Ok(())
        })
    }
}
