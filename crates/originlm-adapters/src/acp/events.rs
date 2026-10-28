use super::{invalid, text, wire::Wire};
use originlm_contracts::*;
use originlm_runtime::TurnContext;
use serde_json::{Value, json};
impl Wire {
    pub async fn permission(
        &mut self,
        message: &Value,
        cx: &TurnContext,
        review: bool,
    ) -> Result<()> {
        if message["params"]["sessionId"] != self.session["sessionId"] {
            return Err(DomainError::new(
                ErrorCode::SessionRouteMismatch,
                "ACP permission belongs to another session",
            ));
        }
        if message["method"] != "session/request_permission" {
            self.deny_request(message).await?;
            return Err(DomainError::new(
                ErrorCode::IntegrationRestricted,
                "ACP requested an unadvertised client capability",
            ));
        }
        let p = &message["params"];
        let choices = p["options"]
            .as_array()
            .filter(|a| a.len() <= 32)
            .ok_or_else(invalid)?;
        let yes = choices
            .iter()
            .find(|c| c["kind"] == "allow_once")
            .map(|c| text(&c["optionId"]))
            .transpose()?;
        let no = choices
            .iter()
            .find(|c| c["kind"] == "reject_once" || c["kind"] == "reject_always")
            .map(|c| text(&c["optionId"]))
            .transpose()?;
        let allowed = if review && yes.is_some() {
            let title = p["toolCall"]["title"]
                .as_str()
                .unwrap_or("Native tool request")
                .chars()
                .take(200)
                .collect();
            cx.approve(title,"The native client requests one tool operation. Approval does not change future permissions.".into()).await?
        } else {
            false
        };
        let choice = if allowed { yes } else { no };
        let outcome = if cx.is_cancelled() {
            json!({"outcome":"cancelled"})
        } else if let Some(option) = choice {
            json!({"outcome":"selected","optionId":option})
        } else {
            json!({"outcome":"cancelled"})
        };
        self.peer
            .send(json!({"jsonrpc":"2.0","id":message["id"],"result":{"outcome":outcome}}))
            .await
    }
    pub fn update(&mut self, message: &Value, cx: &TurnContext, model: &Model) -> Result<()> {
        if message["params"]["sessionId"] != self.session["sessionId"] {
            return Err(DomainError::new(
                ErrorCode::SessionRouteMismatch,
                "ACP event belongs to another session",
            ));
        }
        let update = &message["params"]["update"];
        match update["sessionUpdate"].as_str() {
            Some("agent_message_chunk" | "agent_thought_chunk") => {
                let content = &update["content"];
                if content["type"] == "text" {
                    let text = content["text"].as_str().ok_or_else(invalid)?;
                    emit_text(cx, text, update["sessionUpdate"] == "agent_thought_chunk")?;
                } else {
                    cx.emit(EventPayload::Warning {
                        code: "UNSUPPORTED_CONTENT_BLOCK".into(),
                        message: "ACP returned a non-text content block".into(),
                    })?;
                }
            }
            Some("tool_call" | "tool_call_update") => {
                cx.emit(EventPayload::Tool {
                    name: update["title"]
                        .as_str()
                        .unwrap_or("Native tool")
                        .chars()
                        .take(200)
                        .collect(),
                    status: update["status"]
                        .as_str()
                        .unwrap_or("pending")
                        .chars()
                        .take(40)
                        .collect(),
                })?;
            }
            Some("config_option_update") => {
                self.session["configOptions"] = update["configOptions"].clone();
                let catalog = self.catalog()?;
                if !catalog
                    .models
                    .iter()
                    .any(|m| m.native_id == model.native_id && m.is_default)
                {
                    return Err(DomainError::new(
                        ErrorCode::SelectionStale,
                        "Native model changed during the turn",
                    ));
                }
                cx.emit(EventPayload::OptionsChanged {
                    revision: catalog
                        .models
                        .iter()
                        .find(|m| m.is_default)
                        .ok_or_else(invalid)?
                        .revision
                        .clone(),
                })?;
            }
            Some("usage_update") => {
                if let Some(used) = update["used"].as_u64() {
                    cx.emit(EventPayload::Usage {
                        unit: "context_tokens".into(),
                        used: used.to_string(),
                    })?;
                }
            }
            Some("current_mode_update") => {
                return Err(DomainError::new(
                    ErrorCode::SelectionStale,
                    "Native permission mode changed",
                ));
            }
            Some(
                "user_message_chunk" | "plan" | "available_commands_update" | "session_info_update",
            ) => {}
            // A session notice is not assistant text. Reimplemented from T3 Code
            // 323d9611 (ACP session notices). An empty title is omitted. A
            // malformed notice stays an unknown update.
            Some("notice") => match session_notice(update) {
                SessionNotice::Message(message) => {
                    cx.emit(EventPayload::Warning {
                        code: "ACP_SESSION_NOTICE".into(),
                        message,
                    })?;
                }
                SessionNotice::Omit => {}
                SessionNotice::Unknown => {
                    cx.emit(EventPayload::Warning {
                        code: "UNKNOWN_ACP_UPDATE".into(),
                        message: "An unsupported ACP update was omitted".into(),
                    })?;
                }
            },
            Some(_) => {
                cx.emit(EventPayload::Warning {
                    code: "UNKNOWN_ACP_UPDATE".into(),
                    message: "An unsupported ACP update was omitted".into(),
                })?;
            }
            None => return Err(invalid()),
        }
        Ok(())
    }
}
enum SessionNotice {
    Message(String),
    Omit,
    Unknown,
}
fn bounded_notice(text: &str, max_chars: usize) -> String {
    text.trim().chars().take(max_chars).collect()
}
/// ACP `sessionUpdate: "notice"`. Title and description are bounded. Severity
/// is required; a blank severity is `info`. This does not copy T3's parser.
fn session_notice(update: &Value) -> SessionNotice {
    let title = match update.get("title") {
        Some(Value::String(text)) => {
            let title = bounded_notice(text, 1024);
            if title.is_empty() {
                return SessionNotice::Omit;
            }
            title
        }
        _ => return SessionNotice::Unknown,
    };
    let description = match update.get("description") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => {
            let text = bounded_notice(text, 4096);
            (!text.is_empty()).then_some(text)
        }
        Some(_) => return SessionNotice::Unknown,
    };
    let severity = match update.get("severity") {
        Some(Value::String(text)) => {
            let text = bounded_notice(text, 64);
            if text.is_empty() {
                "info".to_string()
            } else {
                text
            }
        }
        _ => return SessionNotice::Unknown,
    };
    let mut message = if severity == "info" {
        title
    } else {
        format!("[{severity}] {title}")
    };
    if let Some(description) = description {
        message.push_str("\n\n");
        message.push_str(&description);
    }
    SessionNotice::Message(message)
}
fn emit_text(cx: &TurnContext, text: &str, thought: bool) -> Result<()> {
    let mut chunk = String::new();
    for ch in text.chars() {
        chunk.push(ch);
        if chunk.len() >= 16000 {
            let text = std::mem::take(&mut chunk);
            cx.emit(if thought {
                EventPayload::ReasoningSummary { text }
            } else {
                EventPayload::TextDelta { text }
            })?;
        }
    }
    if !chunk.is_empty() {
        cx.emit(if thought {
            EventPayload::ReasoningSummary { text: chunk }
        } else {
            EventPayload::TextDelta { text: chunk }
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_notice_keeps_title_and_description_out_of_a_model_reply() {
        let shown = session_notice(&json!({
            "sessionUpdate": "notice",
            "severity": "warning",
            "title": "  Quota refresh  ",
            "description": "  remaining  "
        }));
        match shown {
            SessionNotice::Message(message) => {
                assert_eq!(message, "[warning] Quota refresh\n\nremaining");
            }
            _ => panic!("expected a notice"),
        }
        assert!(matches!(
            session_notice(&json!({"sessionUpdate":"notice","severity":"info","title":"   "})),
            SessionNotice::Omit
        ));
        assert!(matches!(
            session_notice(
                &json!({"sessionUpdate":"notice","description":"secret-notice-sentinel"})
            ),
            SessionNotice::Unknown
        ));
        let long = "n".repeat(1100);
        match session_notice(&json!({"sessionUpdate":"notice","severity":"","title": long})) {
            SessionNotice::Message(message) => assert_eq!(message.chars().count(), 1024),
            _ => panic!("expected a bounded notice"),
        }
    }
}
