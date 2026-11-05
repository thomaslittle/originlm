//! OriginLM-owned conversation transcript and provider handoff rendering.
//!
//! The transcript is provider-independent: it records the user text, the
//! assistant text and compact tool/artifact notes. It is never a copy of a
//! native client's private session. When a host opens a session for a different
//! model, the service renders the transcript as role-tagged messages for HTTP
//! providers or as one delimited prelude for native clients that only accept a
//! single prompt.
use crate::{Agent, BoxFuture, TurnContext};
use originlm_contracts::redact_conversation_text;
use serde_json::Value;
use std::collections::BTreeMap;

/// Conservative fallback when the target model does not report a context window.
pub const DEFAULT_CONTEXT_TOKENS: u64 = 32_000;
/// Stored transcript cap. Older turns are dropped deterministically and counted.
pub const MAX_TRANSCRIPT_TURNS: usize = 512;
pub const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
/// Never seed more than this many bytes even to a very large context window.
pub const MAX_SEED_BYTES: usize = 512 * 1024;
/// The HTTP adapter retains at most 62 messages. Carry at most 20 prior turns
/// (40 messages plus an optional system message) so a fresh HTTP session still
/// has room for many new turns; native sessions accept the same bounded seed.
pub const MAX_SEED_TURNS: usize = 20;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptTurn {
    pub user: String,
    pub assistant: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    pub turns: Vec<TranscriptTurn>,
    /// Turns dropped from the stored transcript because it reached its own cap.
    pub omitted_turns: u32,
}

impl Transcript {
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Records a completed turn after redacting credential-shaped values.
    pub fn push_turn(&mut self, user: &str, assistant: &str) {
        let user = redact_conversation_text(user);
        let assistant = redact_conversation_text(assistant);
        if user.is_empty() && assistant.is_empty() {
            return;
        }
        self.turns.push(TranscriptTurn { user, assistant });
        if self.turns.len() > MAX_TRANSCRIPT_TURNS {
            let drop = self.turns.len() - MAX_TRANSCRIPT_TURNS;
            self.turns.drain(..drop);
            self.omitted_turns = self.omitted_turns.saturating_add(drop as u32);
        }
        while self.turns.len() > 1 && self.bytes() > MAX_TRANSCRIPT_BYTES {
            self.turns.remove(0);
            self.omitted_turns = self.omitted_turns.saturating_add(1);
        }
    }

    pub fn bytes(&self) -> usize {
        self.turns
            .iter()
            .map(|turn| turn.user.len() + turn.assistant.len() + 64)
            .sum()
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum SeedRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedMessage {
    pub role: SeedRole,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorTurn {
    pub user: String,
    pub assistant: String,
}

/// Provider-neutral history ready to hand to a fresh model session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PriorHistory {
    /// Application instructions. Kept separate from the transcript so trimming
    /// never removes the system/app layer.
    pub instructions: Option<String>,
    /// Newest turns kept verbatim, oldest first.
    pub turns: Vec<PriorTurn>,
    /// Older turns omitted from the stored transcript or by target budgeting.
    pub omitted_turns: u32,
    /// Conservative byte-based token estimate of the rendered seed.
    pub estimated_tokens: u64,
    /// True when any turn was omitted. Always reported to the host.
    pub trimmed: bool,
}

fn omission_note(turns: u32) -> String {
    format!(
        "Earlier conversation: {turns} turn(s) were omitted to fit the target model's context window."
    )
}

impl PriorHistory {
    pub fn is_empty(&self) -> bool {
        self.instructions.is_none() && self.turns.is_empty()
    }

    /// Role-tagged messages for providers that accept a message array.
    pub fn http_messages(&self) -> Vec<SeedMessage> {
        let mut messages = Vec::new();
        let mut system = self.instructions.clone().unwrap_or_default();
        if self.omitted_turns > 0 {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(&omission_note(self.omitted_turns));
        }
        if !system.is_empty() {
            messages.push(SeedMessage {
                role: SeedRole::System,
                text: system,
            });
        }
        for turn in &self.turns {
            if !turn.user.is_empty() {
                messages.push(SeedMessage {
                    role: SeedRole::User,
                    text: turn.user.clone(),
                });
            }
            if !turn.assistant.is_empty() {
                messages.push(SeedMessage {
                    role: SeedRole::Assistant,
                    text: turn.assistant.clone(),
                });
            }
        }
        messages
    }

    /// One clearly delimited block for native clients that only take a prompt.
    /// Injected on the first turn of a fresh native session, never on resume.
    pub fn prelude(&self) -> String {
        let mut output = String::from(
            "[OriginLM conversation handoff]\n\
             This session continues an earlier conversation. The material below is prior context. \
             Do not repeat it verbatim; use it to answer the current message.\n",
        );
        if let Some(instructions) = &self.instructions {
            output.push_str("\n[Application instructions]\n");
            output.push_str(instructions);
            output.push('\n');
        }
        if self.omitted_turns > 0 {
            output.push('\n');
            output.push_str(&omission_note(self.omitted_turns));
            output.push('\n');
        }
        output.push_str("\n[Prior conversation]\n");
        for turn in &self.turns {
            output.push_str("\nUser:\n");
            output.push_str(&turn.user);
            output.push_str("\n\nAssistant:\n");
            output.push_str(&turn.assistant);
            output.push('\n');
        }
        output.push_str("\n[End prior conversation]\n\n[Current message]\n");
        output
    }

    pub fn seeded_bytes(&self) -> usize {
        let instructions = self.instructions.as_ref().map(String::len).unwrap_or(0);
        let turns: usize = self
            .turns
            .iter()
            .map(|turn| turn.user.len() + turn.assistant.len() + 64)
            .sum();
        instructions + turns + if self.omitted_turns > 0 { 160 } else { 0 }
    }
}

/// Builds a target-model-budgeted handoff. The newest turns are kept verbatim;
/// the oldest whole turns are dropped deterministically. Instructions always
/// survive. Returns `None` when there is nothing to carry.
pub fn build_prior(
    transcript: &Transcript,
    instructions: Option<&str>,
    context_window: Option<u64>,
) -> Option<PriorHistory> {
    let instructions = instructions
        .map(redact_conversation_text)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    if transcript.is_empty() && instructions.is_none() {
        return None;
    }
    let target = context_window
        .filter(|tokens| *tokens > 0)
        .unwrap_or(DEFAULT_CONTEXT_TOKENS)
        .clamp(1_024, 2_000_000);
    let reserve = (target / 8).clamp(512, 8_192);
    let budget_bytes = ((target.saturating_sub(reserve)) * 4).min(MAX_SEED_BYTES as u64) as usize;
    let mut used = instructions.as_ref().map(String::len).unwrap_or(0) + 128;
    let mut kept: Vec<PriorTurn> = Vec::new();
    for turn in transcript.turns.iter().rev() {
        if kept.len() >= MAX_SEED_TURNS {
            break;
        }
        let estimate = turn.user.len() + turn.assistant.len() + 64;
        if !kept.is_empty() && used.saturating_add(estimate) > budget_bytes {
            break;
        }
        used = used.saturating_add(estimate);
        kept.push(PriorTurn {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
        });
    }
    kept.reverse();
    let newly_omitted = transcript.turns.len().saturating_sub(kept.len()) as u32;
    let omitted_turns = transcript.omitted_turns.saturating_add(newly_omitted);
    let mut prior = PriorHistory {
        instructions,
        turns: kept,
        omitted_turns,
        estimated_tokens: 0,
        trimmed: omitted_turns > 0,
    };
    let bytes = prior.seeded_bytes();
    prior.estimated_tokens = ((bytes as u64) + 3) / 4;
    Some(prior)
}

/// A native agent wrapper that injects the handoff exactly once, on the first
/// turn of a fresh session. Subsequent turns go to the native session as-is.
pub fn prelude_agent(inner: Box<dyn Agent>, prior: PriorHistory) -> Box<dyn Agent> {
    if prior.is_empty() {
        return inner;
    }
    let prelude = prior.prelude();
    Box::new(PreludeAgent {
        inner: Some(inner),
        prelude: Some(prelude),
    })
}

struct PreludeAgent {
    inner: Option<Box<dyn Agent>>,
    prelude: Option<String>,
}

impl Agent for PreludeAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        context: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let prompt = match self.prelude.take() {
                Some(prelude) => format!("{prelude}{prompt}"),
                None => prompt,
            };
            let inner = self
                .inner
                .as_mut()
                .ok_or_else(|| crate::err(originlm_contracts::ErrorCode::ProviderUnavailable))?;
            inner.turn(prompt, options, context).await
        })
    }

    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if let Some(mut inner) = self.inner.take() {
                inner.close().await
            } else {
                Ok(())
            }
        })
    }

    fn native_resumption(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(|inner| inner.native_resumption())
    }

    fn image_input(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| inner.image_input())
    }

    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            self.inner
                .as_mut()
                .ok_or_else(|| crate::err(originlm_contracts::ErrorCode::ProviderUnavailable))?
                .suspend()
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(turns: &[(&str, &str)]) -> Transcript {
        let mut transcript = Transcript::default();
        for (user, assistant) in turns {
            transcript.push_turn(user, assistant);
        }
        transcript
    }

    #[test]
    fn newest_turns_survive_and_trimming_is_reported() {
        let turns: Vec<(String, String)> = (0..20)
            .map(|index| {
                (
                    format!("question {index} {}", "u".repeat(400)),
                    format!("answer {index} {}", "a".repeat(400)),
                )
            })
            .collect();
        let transcript = transcript(
            &turns
                .iter()
                .map(|(user, assistant)| (user.as_str(), assistant.as_str()))
                .collect::<Vec<_>>(),
        );
        let prior = build_prior(&transcript, Some("home summary"), Some(4_096)).unwrap();
        assert!(prior.trimmed);
        assert!(prior.omitted_turns > 0);
        assert!(prior.instructions.is_some());
        assert_eq!(
            prior.turns.last().unwrap().user,
            turns.last().unwrap().0,
            "newest turn stays verbatim"
        );
        assert!(prior.estimated_tokens > 0);
    }

    #[test]
    fn no_prior_when_nothing_to_carry() {
        assert!(build_prior(&Transcript::default(), None, Some(32_000)).is_none());
    }

    #[test]
    fn credentials_never_reach_the_seed() {
        let transcript = transcript(&[(
            "use Bearer abcdefghijklmnop and sk-ant-secretsecretsecret",
            "ok, token=another-secret and password: hunter2",
        )]);
        let prior = build_prior(&transcript, Some("api_key=instructions-secret"), None).unwrap();
        let rendered = format!("{:?}{:?}", prior.http_messages(), prior.prelude());
        for secret in [
            "abcdefghijklmnop",
            "secretsecretsecret",
            "another-secret",
            "hunter2",
            "instructions-secret",
        ] {
            assert!(!rendered.contains(secret), "leaked {secret}");
        }
    }
}
