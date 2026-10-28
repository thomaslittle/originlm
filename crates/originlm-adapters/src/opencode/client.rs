use crate::http::Endpoint;
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "OpenCode session response is invalid",
    )
}
fn sid(value: &Value) -> Result<String> {
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
pub async fn open(
    endpoint: Endpoint,
    model: Model,
    provider: String,
    workspace: PathBuf,
) -> Result<Box<dyn Agent>> {
    open_session(endpoint, model, provider, workspace, false, None).await
}
/// Opens a text session. `persistent` keeps the native session on suspend
/// (instead of deleting it); `resume` reattaches such a session by id after
/// checking it still exists in this directory with deny-all permissions.
pub async fn open_session(
    endpoint: Endpoint,
    model: Model,
    provider: String,
    workspace: PathBuf,
    persistent: bool,
    resume: Option<String>,
) -> Result<Box<dyn Agent>> {
    let directory = workspace.to_str().ok_or_else(invalid)?.to_owned();
    let permission = json!([{"permission":"*","pattern":"*","action":"deny"}]);
    if let Some(id) = resume {
        let gone = || {
            DomainError::new(
                ErrorCode::ResumeUnavailable,
                "OpenCode no longer has this session",
            )
        };
        let id = sid(&json!(id)).map_err(|_| gone())?;
        let response = endpoint
            .json_query(&format!("session/{id}"), &[("directory", &directory)], None)
            .await
            .map_err(|_| gone())?;
        if response["id"] != id.as_str() {
            return Err(gone());
        }
        if response["permission"] != permission {
            return Err(DomainError::new(
                ErrorCode::IntegrationRestricted,
                "OpenCode did not confirm deny-all tool permissions",
            ));
        }
        // The newest existing reply must never be mistaken for a new one.
        let last_reply = endpoint
            .json_query(
                &format!("session/{id}/message"),
                &[("directory", &directory), ("limit", "2")],
                None,
            )
            .await
            .ok()
            .and_then(|messages| {
                messages.as_array().and_then(|messages| {
                    messages
                        .iter()
                        .rev()
                        .find(|m| m["info"]["role"] == "assistant")
                        .and_then(|m| sid(&m["info"]["id"]).ok())
                })
            });
        return Ok(Box::new(Client {
            endpoint,
            model,
            provider,
            directory,
            id: Some(id),
            last_reply,
            poisoned: false,
            persistent: true,
            resumable: true,
        }));
    }
    let response=endpoint.json_query("session",&[("directory",&directory)],Some(json!({"title":"OriginLM text session","model":{"id":model.native_id,"providerID":provider},"permission":permission}))).await?;
    let id = sid(&response["id"])?;
    if response["permission"] != permission {
        let _ = endpoint
            .delete(&format!("session/{id}"), &[("directory", &directory)])
            .await;
        return Err(DomainError::new(
            ErrorCode::IntegrationRestricted,
            "OpenCode did not confirm deny-all tool permissions",
        ));
    }
    Ok(Box::new(Client {
        endpoint,
        model,
        provider,
        directory,
        id: Some(id),
        last_reply: None,
        poisoned: false,
        persistent,
        resumable: false,
    }))
}
struct Client {
    endpoint: Endpoint,
    model: Model,
    provider: String,
    directory: String,
    id: Option<String>,
    last_reply: Option<String>,
    poisoned: bool,
    /// Keep the native session on suspend instead of deleting it.
    persistent: bool,
    /// A turn completed (or the session was reattached), so history exists.
    resumable: bool,
}
impl Client {
    async fn abort(&self, id: &str) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            self.endpoint.action(
                &format!("session/{id}/abort"),
                &[("directory", &self.directory)],
                json!({}),
            ),
        )
        .await;
    }
    async fn run(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        if self.poisoned {
            return Err(invalid());
        }
        let id = self.id.clone().ok_or_else(invalid)?;
        for (key, value) in &options {
            self.model
                .options
                .iter()
                .find(|d| d.id == *key)
                .ok_or_else(invalid)?
                .validate(value)
                .map_err(|_| invalid())?;
        }
        let query = [("directory", self.directory.as_str())];
        let mut stream = tokio::select! {s=self.endpoint.event_stream("event",&query)=>s?,_=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled"))};
        let mut body = json!({"model":{"providerID":self.provider,"modelID":self.model.native_id},"parts":[{"type":"text","text":prompt}]});
        if let Some(variant) = options.get("variant") {
            body["variant"] = variant.clone();
        }
        let prompt_path = format!("session/{id}/prompt_async");
        tokio::select! {r=self.endpoint.action(&prompt_path,&query,body)=>{r?;},_=cx.cancelled()=>{self.abort(&id).await;return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled"));}}
        let mut data = String::new();
        let mut emitted = String::new();
        let mut assistants = BTreeSet::new();
        let mut saw_unidentified_delta = false;
        loop {
            let line=tokio::select!{r=stream.line()=>r?,_=cx.cancelled()=>{self.abort(&id).await;return Err(DomainError::new(ErrorCode::Cancelled,"Session cancelled"));}}.ok_or_else(invalid)?;
            if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
                if data.len() > 1024 * 1024 {
                    return Err(invalid());
                }
                continue;
            }
            if !line.is_empty() || data.is_empty() {
                continue;
            }
            let event: Value =
                serde_json::from_str(&std::mem::take(&mut data)).map_err(|_| invalid())?;
            let p = &event["properties"];
            if event["type"] == "message.updated"
                && p["info"]["sessionID"] == id
                && p["info"]["role"] == "assistant"
            {
                if p["info"]
                    .get("modelID")
                    .is_some_and(|v| v != &self.model.native_id)
                    || p["info"]
                        .get("providerID")
                        .is_some_and(|v| v != &self.provider)
                {
                    return Err(invalid());
                }
                if assistants.len() >= 64 {
                    return Err(invalid());
                }
                assistants.insert(sid(&p["info"]["id"])?);
            }
            if p["sessionID"] != id {
                continue;
            }
            match event["type"].as_str() {
                Some("message.part.delta") if p["field"] == "text" => {
                    let message = sid(&p["messageID"])?;
                    if !assistants.contains(&message) {
                        saw_unidentified_delta = true;
                        continue;
                    }
                    if saw_unidentified_delta {
                        continue;
                    }
                    let text = p["delta"].as_str().ok_or_else(invalid)?;
                    if emitted.len() + text.len() > 1024 * 1024 {
                        return Err(invalid());
                    }
                    emit(&cx, text)?;
                    emitted.push_str(text);
                }
                Some("session.error") => {
                    // Surface OpenCode's own reason (redacted), e.g. an upstream
                    // provider error or rate limit, instead of a bare failure.
                    let error = &p["error"];
                    let detail = error["data"]["message"]
                        .as_str()
                        .or_else(|| error["name"].as_str())
                        .map(|m| m.chars().take(400).collect::<String>())
                        .map(|m| originlm_contracts::redact_diagnostic_detail(&m))
                        .filter(|m| !m.trim().is_empty());
                    return Err(DomainError::new(
                        ErrorCode::ProviderUnavailable,
                        match detail {
                            Some(detail) => format!("OpenCode reported a session error: {detail}"),
                            None => "OpenCode reported a session error".into(),
                        },
                    ));
                }
                Some("permission.asked" | "permission.updated") => {
                    return Err(DomainError::new(
                        ErrorCode::IntegrationRestricted,
                        "OpenCode requested a denied tool",
                    ));
                }
                Some("session.status") if p["status"]["type"] == "idle" => break,
                _ => {}
            }
        }
        let result = self
            .endpoint
            .json_query(
                &format!("session/{id}/message"),
                &[("directory", &self.directory), ("limit", "2")],
                None,
            )
            .await?;
        let messages = result.as_array().ok_or_else(invalid)?;
        let reply = messages
            .iter()
            .rev()
            .find(|m| m["info"]["role"] == "assistant")
            .ok_or_else(invalid)?;
        let reply_id = sid(&reply["info"]["id"])?;
        if self.last_reply.as_ref() == Some(&reply_id)
            || reply["info"].get("error").is_some()
            || reply["info"]["time"].get("completed").is_none()
            || reply["info"]["providerID"] != self.provider
            || reply["info"]["modelID"] != self.model.native_id
        {
            return Err(invalid());
        }
        let mut full = String::new();
        for part in reply["parts"].as_array().ok_or_else(invalid)? {
            if part["type"] == "tool" {
                return Err(DomainError::new(
                    ErrorCode::IntegrationRestricted,
                    "Unexpected tool result on text-only route",
                ));
            }
            if part["type"] == "text" {
                full.push_str(part["text"].as_str().ok_or_else(invalid)?);
                if full.len() > 1024 * 1024 {
                    return Err(invalid());
                }
            }
        }
        if !full.starts_with(&emitted) {
            return Err(invalid());
        }
        emit(&cx, &full[emitted.len()..])?;
        for (key, unit) in [("input", "input_tokens"), ("output", "output_tokens")] {
            if let Some(n) = reply["info"]["tokens"][key].as_u64() {
                cx.emit(EventPayload::Usage {
                    unit: unit.into(),
                    used: n.to_string(),
                })?;
            }
        }
        self.last_reply = Some(reply_id);
        self.resumable = true;
        Ok(())
    }
}
fn emit(cx: &TurnContext, text: &str) -> Result<()> {
    let mut chunk = String::new();
    for c in text.chars() {
        chunk.push(c);
        if chunk.len() >= 8192 {
            cx.emit(EventPayload::TextDelta {
                text: std::mem::take(&mut chunk),
            })?;
        }
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
            let r = self.run(prompt, options, cx).await;
            if r.is_err() {
                self.poisoned = true;
                if let Some(id) = &self.id {
                    self.abort(id).await;
                }
            }
            r
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if let Some(id) = self.id.take() {
                self.endpoint
                    .delete(&format!("session/{id}"), &[("directory", &self.directory)])
                    .await?;
            }
            Ok(())
        })
    }
    fn native_resumption(&self) -> bool {
        self.persistent
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            if !(self.persistent && self.resumable) || self.poisoned {
                return Err(DomainError::new(
                    ErrorCode::Conflict,
                    "Nothing to resume until a turn completes",
                ));
            }
            // Taking the id keeps close() from deleting the native session.
            self.id.take().ok_or_else(invalid)
        })
    }
}
