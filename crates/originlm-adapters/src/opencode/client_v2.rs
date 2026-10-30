//! OpenCode 2.0.23 text transport. Not wired to installed-client launch until
//! catalog authentication and safe startup isolation are independently verified.
use crate::http::Endpoint;
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};

const MAX_TEXT: usize = 1024 * 1024;
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "OpenCode 2 returned an invalid text session event",
    )
}
fn safe_id(value: &Value) -> Result<String> {
    value
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() < 200
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        })
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn permissions() -> Value {
    json!([{"action":"*","resource":"*","effect":"deny"}])
}

/// Open an explicitly supplied endpoint; does not discover credentials or spawn.
/// Persistent resume and image input are deliberately unadvertised.
pub async fn open(
    endpoint: Endpoint,
    model: Model,
    provider: String,
    workspace: PathBuf,
) -> Result<Box<dyn Agent>> {
    let directory = workspace
        .to_str()
        .filter(|_| workspace.is_absolute())
        .ok_or_else(invalid)?;
    let selected = json!({"providerID":provider,"id":model.native_id});
    let response = endpoint.json("api/session", Some(json!({"title":"OriginLM text session", "location":{"directory":directory}, "model":selected, "permissions":permissions()}))).await?;
    let session = &response["data"];
    let id = safe_id(&session["id"])?;
    // The service adds its default variant to the returned model; compare the
    // fields the caller selected instead of requiring exact object equality.
    let confirmed_model = session["model"]["providerID"] == selected["providerID"]
        && session["model"]["id"] == selected["id"];
    if session["permissions"] != permissions()
        || !confirmed_model
        || session["location"]["directory"] != directory
    {
        let _ = endpoint.delete(&format!("api/session/{id}"), &[]).await;
        return Err(DomainError::new(
            ErrorCode::IntegrationRestricted,
            "OpenCode 2 did not confirm the requested model, workspace and deny-all permissions",
        ));
    }
    Ok(Box::new(Client {
        endpoint,
        id: Some(id),
        model,
        provider,
        poisoned: false,
        closing: false,
        seen_messages: BTreeSet::new(),
        last_durable_sequence: None,
    }))
}
struct Client {
    endpoint: Endpoint,
    id: Option<String>,
    model: Model,
    provider: String,
    poisoned: bool,
    closing: bool,
    seen_messages: BTreeSet<String>,
    last_durable_sequence: Option<u64>,
}
#[derive(Default)]
struct Block {
    text: String,
    ended: bool,
    started: bool,
}
impl Client {
    async fn interrupt(&self, id: &str) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            self.endpoint
                .action(&format!("api/session/{id}/interrupt"), &[], Value::Null),
        )
        .await;
    }
    async fn run(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        if self.poisoned || self.closing {
            return Err(invalid());
        }
        let id = self.id.clone().ok_or_else(invalid)?;
        for (key, value) in &options {
            self.model
                .options
                .iter()
                .find(|option| option.id == *key)
                .ok_or_else(invalid)?
                .validate(value)
                .map_err(|_| invalid())?;
            if key != "variant" {
                return Err(DomainError::new(
                    ErrorCode::OptionUnsupported,
                    "OpenCode 2 option is unsupported",
                ));
            }
        }
        let mut selected = json!({"providerID":self.provider,"id":self.model.native_id});
        if let Some(variant) = options.get("variant") {
            selected["variant"] = variant.clone();
        }
        let model_path = format!("api/session/{id}/model");
        tokio::select! {
            result=self.endpoint.action(&model_path,&[],json!({"model":selected}))=>{result?;},
            _=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled")),
        }
        // The v2 event stream is volatile: connect before admitting the prompt.
        let mut stream = tokio::select! {
            stream=self.endpoint.event_stream("api/event",&[])=>stream?,
            _=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled")),
        };
        let prompt_path = format!("api/session/{id}/prompt");
        let response = tokio::select! {
            result=self.endpoint.action(&prompt_path,&[],json!({"text":prompt}))=>result?,
            _=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled")),
        };
        if response["data"]["sessionID"] != id {
            return Err(invalid());
        }
        let mut data = String::new();
        let mut started = false;
        let mut blocks: BTreeMap<(String, u64), Block> = BTreeMap::new();
        let mut active: Option<(String, u64)> = None;
        let mut total = 0usize;
        let mut messages = BTreeSet::new();
        let mut ordinals: BTreeMap<String, u64> = BTreeMap::new();
        loop {
            let line=tokio::select! {
                line=tokio::time::timeout(Duration::from_secs(45),stream.line())=>line.map_err(|_|DomainError::new(ErrorCode::Timeout,"OpenCode 2 event stream was silent"))??,
                _=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled")),
            }.ok_or_else(invalid)?;
            if let Some(part) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(part.trim_start());
                if data.len() > MAX_TEXT {
                    return Err(invalid());
                }
                continue;
            }
            if !line.is_empty() || data.is_empty() {
                continue;
            }
            let event: Value =
                serde_json::from_str(&std::mem::take(&mut data)).map_err(|_| invalid())?;
            let kind = event["type"].as_str().ok_or_else(invalid)?;
            let body = &event["data"];
            if body["sessionID"] != id {
                continue;
            }
            if matches!(
                kind,
                "session.execution.started"
                    | "session.execution.succeeded"
                    | "session.execution.failed"
                    | "session.execution.interrupted"
                    | "session.text.started"
                    | "session.text.ended"
            ) {
                // These v2 events are durable within the session aggregate.
                // A replayed old terminal must never complete the new turn.
                let durable = &event["durable"];
                let sequence = durable["seq"].as_u64().ok_or_else(invalid)?;
                if durable["aggregateID"] != id
                    || !durable["version"]
                        .as_u64()
                        .is_some_and(|version| version > 0)
                {
                    return Err(invalid());
                }
                if self
                    .last_durable_sequence
                    .is_some_and(|last| sequence <= last)
                {
                    continue;
                }
                self.last_durable_sequence = Some(sequence);
            }
            if kind == "session.execution.started" {
                if started {
                    return Err(invalid());
                }
                started = true;
                continue;
            }
            if kind == "permission.asked" || kind.starts_with("session.tool.") {
                return Err(DomainError::new(
                    ErrorCode::IntegrationRestricted,
                    "OpenCode 2 requested a native tool on a text-only route",
                ));
            }
            if !started {
                continue;
            }
            match kind {
                "session.text.started" | "session.text.delta" | "session.text.ended" => {
                    let key = (
                        safe_id(&body["assistantMessageID"])?,
                        body["ordinal"].as_u64().ok_or_else(invalid)?,
                    );
                    // A volatile stream must not replay a previous turn's assistant.
                    if self.seen_messages.contains(&key.0) && !messages.contains(&key.0) {
                        return Err(invalid());
                    }
                    if !blocks.contains_key(&key) {
                        // Ordinals may skip reasoning/tool blocks, but text cannot
                        // go backwards or revisit an earlier assistant message.
                        if ordinals.get(&key.0).is_some_and(|last| key.1 <= *last)
                            || (messages.contains(&key.0)
                                && active.as_ref().is_some_and(|last| last.0 != key.0))
                        {
                            return Err(invalid());
                        }
                        if self.seen_messages.len() >= 4096 && !messages.contains(&key.0) {
                            return Err(invalid());
                        }
                        messages.insert(key.0.clone());
                        self.seen_messages.insert(key.0.clone());
                        ordinals.insert(key.0.clone(), key.1);
                    }
                    // An exact repeated final block adds no output and must not
                    // move the active block backwards.
                    if let Some(block) = blocks.get(&key).filter(|block| block.ended) {
                        if kind == "session.text.ended" && body["text"] == block.text {
                            continue;
                        }
                        return Err(invalid());
                    }
                    if active.as_ref().is_some_and(|previous| {
                        previous != &key && blocks.get(previous).is_some_and(|block| !block.ended)
                    }) {
                        return Err(invalid());
                    }
                    if blocks.len() >= 128 && !blocks.contains_key(&key) {
                        return Err(invalid());
                    }
                    active = Some(key.clone());
                    let block = blocks.entry(key).or_default();
                    if kind == "session.text.started" {
                        if block.started || !block.text.is_empty() {
                            return Err(invalid());
                        }
                        block.started = true;
                        continue;
                    }
                    let text = if kind == "session.text.delta" {
                        body["delta"].as_str()
                    } else {
                        body["text"].as_str()
                    }
                    .ok_or_else(invalid)?;
                    let append = if kind == "session.text.ended" {
                        if !text.starts_with(&block.text) {
                            return Err(invalid());
                        }
                        &text[block.text.len()..]
                    } else {
                        text
                    };
                    total = total
                        .checked_add(append.len())
                        .filter(|total| *total <= MAX_TEXT)
                        .ok_or_else(invalid)?;
                    emit(&cx, append)?;
                    block.text.push_str(append);
                    block.ended = kind == "session.text.ended";
                }
                "session.execution.succeeded" => {
                    if total == 0 || blocks.values().any(|block| !block.ended) {
                        return Err(invalid());
                    }
                    return Ok(());
                }
                "session.execution.failed" => {
                    let error = &body["error"];
                    let message = error["message"]
                        .as_str()
                        .unwrap_or("text generation failed");
                    let status = error["status"].as_u64();
                    let bounded: String = message
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(400)
                        .collect();
                    let code = match (error["type"].as_str(), status) {
                        (Some("provider.quota"), _)
                        | (_, Some(402))
                        | (_, Some(429)) => ErrorCode::RateLimited,
                        _ => ErrorCode::ProviderUnavailable,
                    };
                    return Err(DomainError::new(
                        code,
                        format!("OpenCode 2: {bounded}"),
                    ));
                }
                "session.execution.interrupted" => {
                    return Err(DomainError::new(
                        ErrorCode::Cancelled,
                        "OpenCode 2 text generation was interrupted",
                    ));
                }
                _ => {}
            }
        }
    }
}
fn emit(cx: &TurnContext, text: &str) -> Result<()> {
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len() + c.len_utf8() > 8192 {
            cx.emit(EventPayload::TextDelta {
                text: std::mem::take(&mut chunk),
            })?;
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        cx.emit(EventPayload::TextDelta { text: chunk })?;
    }
    Ok(())
}
impl Agent for Client {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let result = self.run(prompt, options, cx).await;
            if result.is_err() {
                self.poisoned = true;
                if let Some(id) = &self.id {
                    self.interrupt(id).await;
                }
            }
            result
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.closing = true;
            if let Some(id) = self.id.as_ref() {
                self.endpoint
                    .delete_with_confirmed_absence(&format!("api/session/{id}"), &[], |body| {
                        body["_tag"] == "SessionNotFoundError"
                            && body["sessionID"] == id.as_str()
                            && body["message"].is_string()
                    })
                    .await?;
                // A failed or dropped DELETE future must retain retry ownership.
                self.id = None;
            }
            Ok(())
        })
    }
}
