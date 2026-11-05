//! Canonical conversation handoff across model and provider switches.
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_runtime::{
    Agent, BoxFuture, PriorHistory, SeedMessage, SeedRole, TurnContext, prelude_agent,
};
use originlm_service::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug)]
struct Opened {
    prior: Option<PriorHistory>,
    native_resume: Option<String>,
}

#[derive(Clone, Debug)]
struct Turn {
    model: String,
    prompt: String,
    /// Messages an HTTP provider actually received, including the current one.
    messages: Vec<SeedMessage>,
}

#[derive(Clone, Default)]
struct Recorder {
    opened: Arc<Mutex<Vec<Opened>>>,
    turns: Arc<Mutex<Vec<Turn>>>,
}

struct NativeAgent {
    model: String,
    persistent: bool,
    recorder: Recorder,
}

impl Agent for NativeAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.recorder.turns.lock().unwrap().push(Turn {
                model: self.model.clone(),
                prompt,
                messages: Vec::new(),
            });
            cx.emit(EventPayload::TextDelta {
                text: format!("native reply from {}", self.model),
            })?;
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn native_resumption(&self) -> bool {
        self.persistent
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move { Ok(format!("native-session-{}", self.model)) })
    }
}

struct HttpAgent {
    model: String,
    history: Vec<SeedMessage>,
    recorder: Recorder,
}

impl Agent for HttpAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let mut messages = self.history.clone();
            messages.push(SeedMessage {
                role: SeedRole::User,
                text: prompt,
            });
            self.recorder.turns.lock().unwrap().push(Turn {
                model: self.model.clone(),
                prompt: String::new(),
                messages: messages.clone(),
            });
            let reply = format!("http reply from {}", self.model);
            self.history = messages;
            self.history.push(SeedMessage {
                role: SeedRole::Assistant,
                text: reply.clone(),
            });
            cx.emit(EventPayload::TextDelta { text: reply })?;
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn model(id: &str, context: Option<&str>, evidence: &EvidenceSource) -> Model {
    Model {
        id: id.into(),
        route_id: "route-personal".into(),
        native_id: id.into(),
        name: id.into(),
        revision: "1".into(),
        is_default: false,
        capabilities: vec![Capability {
            id: "generation.text".into(),
            support: Support::Supported,
            origin: CapabilityOrigin::Model,
            evidence: Observation::reported("reported".into(), evidence.clone()),
            conditions: vec![],
        }],
        options: vec![],
        context_window: context
            .map(|window| Observation::reported(window.into(), evidence.clone()))
            .unwrap_or_else(|| Observation::unknown("NOT_REPORTED")),
    }
}

fn snapshot() -> Snapshot {
    let mut snapshot = common::fixture();
    let evidence = EvidenceSource::new(EvidenceKind::Synthetic, "synthetic", "handoff test");
    for (id, context) in [
        ("native-a", None),
        ("native-b", None),
        ("native-persistent", None),
        ("http-a", None),
        ("http-b", None),
        ("http-small", Some("2048")),
    ] {
        snapshot.models.push(model(id, context, &evidence));
    }
    snapshot
}

fn setup(state: Option<std::path::PathBuf>) -> (Arc<NativeService>, Recorder) {
    let recorder = Recorder::default();
    let seen = recorder.clone();
    let inventory = Arc::new(Mutex::new(snapshot()));
    let scan = inventory.clone();
    let mut service = NativeService::new(
        Arc::new(move |_| {
            let snapshot = scan.lock().unwrap().clone();
            let locations = snapshot
                .connections
                .iter()
                .map(|connection| {
                    (
                        connection.id.clone(),
                        LocatedClient {
                            executable_identity: None,
                            provider_id: "test".into(),
                            profile_home: std::env::temp_dir(),
                            executable: None,
                            scope: None,
                        },
                    )
                })
                .collect();
            Ok(ScanResult {
                snapshot,
                locations,
                metadata_probes: 0,
            })
        }),
        Duration::ZERO,
    )
    .with_execution(
        BTreeSet::from(["test".into()]),
        Arc::new(move |input| {
            seen.opened.lock().unwrap().push(Opened {
                prior: input.prior_history.clone(),
                native_resume: input.native_resume.clone(),
            });
            let recorder = seen.clone();
            Box::pin(async move {
                let model = input.model.id.clone();
                let agent: Box<dyn Agent> = if model.starts_with("http") {
                    let history = input
                        .prior_history
                        .as_ref()
                        .map(PriorHistory::http_messages)
                        .unwrap_or_default();
                    Box::new(HttpAgent {
                        model,
                        history,
                        recorder,
                    })
                } else {
                    let native: Box<dyn Agent> = Box::new(NativeAgent {
                        model,
                        persistent: input.persistent,
                        recorder,
                    });
                    match input.prior_history {
                        Some(prior) => prelude_agent(native, prior),
                        None => native,
                    }
                };
                Ok(agent)
            })
        }),
    );
    if let Some(state) = state {
        service =
            service.with_persistent_resume(state, BTreeSet::from(["test".into()]), BTreeSet::new());
    }
    let service = Arc::new(service);
    service
        .register_client("app", ClientPolicy::passive())
        .unwrap();
    service
        .register_execution(
            "app",
            ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
                ..Default::default()
            },
        )
        .unwrap();
    (service, recorder)
}

fn selection(model: &str) -> Value {
    json!({"routeId":"route-personal","modelId":model,"revision":"1","options":{}})
}

async fn create(
    service: &NativeService,
    model: &str,
    conversation: &str,
    instructions: Option<&str>,
) -> Value {
    service
        .handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let mut params = json!({
        "selection": selection(model),
        "workspaceId": "scratch",
        "conversationId": conversation,
    });
    if let Some(instructions) = instructions {
        params["instructions"] = json!(instructions);
    }
    service
        .handle("app", "sessions.create", params)
        .await
        .unwrap()
}

async fn switch(
    service: &NativeService,
    session: &Value,
    model: &str,
    persistent: Option<bool>,
) -> Value {
    let mut params = json!({
        "sessionId": session["id"],
        "selection": selection(model),
    });
    if let Some(persistent) = persistent {
        params["persistent"] = json!(persistent);
    }
    service
        .handle("app", "sessions.switchModel", params)
        .await
        .unwrap()
}

async fn turn(service: &NativeService, session: &Value, prompt: &str, request: &str) {
    let id = session["id"].as_str().unwrap();
    service
        .handle(
            "app",
            "turns.start",
            json!({"sessionId":id,"prompt":prompt,"requestId":request}),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let current = service
                .handle("app", "sessions.get", json!({"sessionId":id}))
                .await
                .unwrap();
            if current["state"] == "completed" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}

fn last_turn(recorder: &Recorder, model: &str) -> Turn {
    recorder
        .turns
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|turn| turn.model == model)
        .cloned()
        .unwrap()
}

#[tokio::test]
async fn native_to_native_seeds_once_and_keeps_the_conversation() {
    let (service, recorder) = setup(None);
    let first = create(&service, "native-a", "conv-native", Some("home summary")).await;
    assert_eq!(first["conversationId"], "conv-native");
    turn(&service, &first, "first question", "1").await;

    let second = switch(&service, &first, "native-b", None).await;
    assert_eq!(second["conversationId"], "conv-native");
    assert_eq!(second["history"]["turns"], 1);
    assert_eq!(second["history"]["trimmed"], false);
    let closed = service
        .handle("app", "sessions.get", json!({"sessionId":first["id"]}))
        .await
        .unwrap();
    assert_eq!(closed["state"], "closed");

    turn(&service, &second, "second question", "2").await;
    let seeded = last_turn(&recorder, "native-b");
    assert!(seeded.prompt.contains("[OriginLM conversation handoff]"));
    assert!(seeded.prompt.contains("home summary"));
    assert!(seeded.prompt.contains("first question"));
    assert!(seeded.prompt.contains("native reply from native-a"));
    assert!(seeded.prompt.ends_with("second question"));

    turn(&service, &second, "third question", "3").await;
    let follow_up = last_turn(&recorder, "native-b");
    assert_eq!(follow_up.prompt, "third question");
    assert!(!follow_up.prompt.contains("handoff"));
}

#[tokio::test]
async fn http_to_native_and_native_to_http_carry_real_messages_and_one_prelude() {
    let (service, recorder) = setup(None);
    let first = create(&service, "http-a", "conv-mixed", Some("app instructions")).await;
    turn(&service, &first, "http one", "1").await;

    let native = switch(&service, &first, "native-b", None).await;
    turn(&service, &native, "native two", "2").await;
    let native_prompt = last_turn(&recorder, "native-b").prompt;
    assert!(native_prompt.contains("http one"));
    assert!(native_prompt.contains("http reply from http-a"));
    assert!(native_prompt.contains("app instructions"));

    let http = switch(&service, &native, "http-b", None).await;
    assert_eq!(http["history"]["turns"], 2);
    turn(&service, &http, "http three", "3").await;
    let request = last_turn(&recorder, "http-b").messages;
    assert_eq!(request[0].role, SeedRole::System);
    assert!(request[0].text.contains("app instructions"));
    let text = request
        .iter()
        .map(|message| message.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("http one"));
    assert!(text.contains("native two"));
    assert!(text.contains("http reply from http-a"));
    assert!(text.contains("native reply from native-b"));
    assert_eq!(request.last().unwrap().text, "http three");
}

#[tokio::test]
async fn http_to_http_switch_uses_role_tagged_prior_messages() {
    let (service, recorder) = setup(None);
    let first = create(&service, "http-a", "conv-http", None).await;
    turn(&service, &first, "alpha", "1").await;

    let second = switch(&service, &first, "http-b", None).await;
    turn(&service, &second, "beta", "2").await;
    let messages = last_turn(&recorder, "http-b").messages;
    let roles: Vec<_> = messages.iter().map(|message| message.role).collect();
    assert_eq!(
        roles,
        vec![SeedRole::User, SeedRole::Assistant, SeedRole::User]
    );
    assert_eq!(messages[0].text, "alpha");
    assert!(messages[1].text.starts_with("http reply from http-a"));
    assert_eq!(messages[2].text, "beta");
}

#[tokio::test]
async fn target_context_budget_trims_oldest_turns_and_reports_it() {
    let (service, recorder) = setup(None);
    let first = create(&service, "http-a", "conv-trim", Some("must stay")).await;
    for index in 0..8 {
        let prompt = format!("turn {index} {}", "x".repeat(900));
        turn(&service, &first, &prompt, &format!("trim-{index}")).await;
    }
    let second = switch(&service, &first, "http-small", None).await;
    assert_eq!(second["history"]["trimmed"], true);
    assert!(second["history"]["omittedTurns"].as_u64().unwrap() > 0);
    assert!(second["history"]["turns"].as_u64().unwrap() >= 1);

    turn(&service, &second, "after trim", "after").await;
    let messages = last_turn(&recorder, "http-small").messages;
    assert_eq!(messages[0].role, SeedRole::System);
    assert!(messages[0].text.starts_with("must stay"));
    let joined = messages
        .iter()
        .map(|message| message.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("omitted to fit the target model"));
    assert!(!joined.contains("turn 0 "), "oldest turn was trimmed");
    assert!(joined.contains("turn 7 "), "newest turn stays verbatim");
}

#[tokio::test]
async fn resume_after_switch_does_not_seed_history_again() {
    let state = tempfile::tempdir().unwrap();
    let (service, recorder) = setup(Some(state.path().to_path_buf()));
    let first = create(&service, "native-a", "conv-resume", None).await;
    turn(&service, &first, "before switch", "1").await;
    let second = switch(&service, &first, "native-persistent", Some(true)).await;
    assert!(second["nativeResumption"].as_bool().unwrap());
    turn(&service, &second, "after switch", "2").await;

    let token = service
        .handle("app", "sessions.suspend", json!({"sessionId":second["id"]}))
        .await
        .unwrap()["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let resumed = service
        .handle("app", "sessions.resume", json!({"resumeToken":token}))
        .await
        .unwrap();
    assert_eq!(resumed["conversationId"], "conv-resume");
    assert!(resumed.get("history").is_none());
    let reopened = recorder.opened.lock().unwrap().last().unwrap().clone();
    assert!(reopened.native_resume.is_some());
    assert!(
        reopened.prior.is_none(),
        "a native resume already has history and must not be seeded"
    );

    turn(&service, &resumed, "after resume", "3").await;
    assert_eq!(
        last_turn(&recorder, "native-persistent").prompt,
        "after resume"
    );
}

#[tokio::test]
async fn credentials_are_redacted_before_the_handoff_is_stored() {
    let (service, recorder) = setup(None);
    let first = create(&service, "native-a", "conv-secret", None).await;
    turn(
        &service,
        &first,
        "here is sk-ant-supersecretsupersecret and Bearer abcdefghijklmnop",
        "1",
    )
    .await;
    let second = switch(&service, &first, "native-b", None).await;
    turn(&service, &second, "continue", "2").await;
    let prompt = last_turn(&recorder, "native-b").prompt;
    assert!(!prompt.contains("supersecretsupersecret"));
    assert!(!prompt.contains("abcdefghijklmnop"));
    assert!(prompt.contains("[REDACTED]"));
}

#[tokio::test]
async fn handshake_advertises_the_handoff_and_switch_method() {
    let (service, _) = setup(None);
    let handshake = service
        .handle("app", "runtime.handshake", json!({"protocolVersion":"1.0"}))
        .await
        .unwrap();
    let methods = handshake["methods"].as_array().unwrap();
    assert!(
        methods
            .iter()
            .any(|method| method == "sessions.switchModel")
    );
    let capabilities = handshake["capabilities"].as_array().unwrap();
    assert!(
        capabilities
            .iter()
            .any(|capability| capability == "session.conversation.handoff")
    );
}
