//! Canonical public method contracts, shared by the service and generated SDK types.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatgptCancelParams {
    pub login_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatgptAuthStatus {
    pub state: ChatgptAuthState,
    pub login_id: Option<String>,
    pub email: Option<String>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatgptAuthState {
    SignedOut,
    Pending,
    Connected,
    Error,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatgptAuthStart {
    pub login_id: String,
    pub auth_url: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandshakeParams {
    #[serde(default)]
    pub protocol_version: Option<String>,
    #[serde(default)]
    pub client_name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeResult {
    pub protocol_version: String,
    pub runtime_version: String,
    pub methods: Vec<String>,
    /// Runtime-level features such as `turn.input.image` or `session.host_mcp`. Model and adapter support is still per selection.
    #[serde(default)]
    pub capabilities: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotParams {
    #[serde(default)]
    pub fields: Option<Vec<String>>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelsParams {
    #[serde(default)]
    pub route_id: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryParams {
    #[serde(default)]
    pub level: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryWatchParams {
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionParams {
    pub connection_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelParams {
    pub model_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirements {
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub allow_paid_fallback: bool,
    #[serde(default)]
    pub eligible_only: bool,
    #[serde(default)]
    pub billing_preference: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub selection: Selection,
    pub connection_id: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionParams {
    pub selection: Selection,
    #[serde(default)]
    pub allow_paid: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantParams {
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub connection_ids: Vec<String>,
    #[serde(default)]
    pub allow_paid: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionCreateParams {
    pub selection: Selection,
    pub workspace_id: String,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub review_requests: bool,
    #[serde(default)]
    pub allow_paid: bool,
    /// Host-owned loopback MCP servers attached to a native-client session.
    /// In the default read-only mode, only these listed tools may be called.
    /// The separately granted `native_policy` mode also enables the native
    /// client's own tools and policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_mcp_servers: Vec<HostMcpServer>,
    /// Keep the native conversation in the client's own history so a
    /// `sessions.suspend` token can resume it after the helper restarts.
    /// Requires the route capability `session.resume.persistent`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub persistent: bool,
}
/// Most host MCP servers a single session may attach.
pub const MAX_HOST_MCP_SERVERS: usize = 2;
/// Most allowed tools across every host MCP server of a session.
pub const MAX_HOST_MCP_TOOLS: usize = 256;
/// A loopback streamable-HTTP MCP server run by the host application.
///
/// `url` must be `http://127.0.0.1:<port>/...` or `http://[::1]:<port>/...`.
/// The bearer token is a host secret: it is never logged, echoed, or placed on
/// a command line, and `Debug` redacts it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostMcpServer {
    /// `^[a-z][a-z0-9_]{0,31}$`
    pub name: String,
    pub url: String,
    /// 32-512 printable ASCII characters.
    pub bearer_token: String,
    /// Tool names (`^[a-z][a-z0-9_]{0,63}$`) the agent may call on this server.
    pub allowed_tools: Vec<String>,
}
impl std::fmt::Debug for HostMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostMcpServer")
            .field("name", &self.name)
            .field("url", &self.url)
            .field("bearer_token", &"[redacted]")
            .field("allowed_tools", &self.allowed_tools)
            .finish()
    }
}
fn host_identifier(value: &str, max: usize) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= max
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}
/// Loopback-only `http://127.0.0.1:<port>/...` or `http://[::1]:<port>/...`.
fn host_mcp_url(url: &str) -> bool {
    if url.len() > 2048 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Some(rest) = url
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| url.strip_prefix("http://[::1]:"))
    else {
        return false;
    };
    let (port, path) = match rest.find('/') {
        Some(index) => rest.split_at(index),
        None => return false,
    };
    !port.is_empty()
        && port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|port| port != 0)
        && !path.contains(['\\', '@', '#'])
}
impl HostMcpServer {
    pub fn validate(&self) -> bool {
        host_identifier(&self.name, 32)
            && host_mcp_url(&self.url)
            && (32..=512).contains(&self.bearer_token.len())
            && self
                .bearer_token
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b))
            && !self.allowed_tools.is_empty()
            && self
                .allowed_tools
                .iter()
                .all(|tool| host_identifier(tool, 64))
            && self
                .allowed_tools
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.allowed_tools.len()
    }
}
/// Validates a whole `hostMcpServers` list: limits, unique names, every entry.
pub fn validate_host_mcp_servers(servers: &[HostMcpServer]) -> bool {
    servers.len() <= MAX_HOST_MCP_SERVERS
        && servers.iter().map(|s| s.allowed_tools.len()).sum::<usize>() <= MAX_HOST_MCP_TOOLS
        && servers
            .iter()
            .map(|s| &s.name)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == servers.len()
        && servers.iter().all(HostMcpServer::validate)
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionParams {
    pub session_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionResumeParams {
    pub resume_token: String,
    /// Host MCP servers for the resumed session. Tokens never carry the
    /// servers' bearer tokens, so a host re-supplies them here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_mcp_servers: Vec<HostMcpServer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSuspendResult {
    /// Opaque, host-storable token. With `session.resume.persistent` it is
    /// sealed with a per-install key, carries no credentials, and survives
    /// helper restarts; otherwise it lives only as long as the helper.
    pub resume_token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionOptionsParams {
    pub session_id: String,
    pub options: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnStartParams {
    pub session_id: String,
    pub prompt: String,
    pub request_id: String,
    /// Staged `inputs.put` references. Each input is consumed by the turn that starts with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<TurnInputRef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnInputRef {
    pub input_id: String,
}
/// Largest decoded image accepted by `inputs.put`.
pub const MAX_INPUT_IMAGE_BYTES: usize = 4 * 1024 * 1024;
/// Largest accepted image width or height, in pixels.
pub const MAX_INPUT_IMAGE_EDGE: u32 = 2048;
/// Inputs a single turn may reference.
pub const MAX_INPUTS_PER_TURN: usize = 4;
/// Staged, unconsumed inputs a single session may hold.
pub const MAX_INPUTS_PER_SESSION: usize = 8;
/// Largest UTF-8 prompt accepted by a text turn.
pub const MAX_TURN_PROMPT_BYTES: usize = 512 * 1024;
/// Largest serialized `turns.start` params object, reserving 8 KiB for protocol
/// envelopes. Transports and native clients also enforce their own frame limits.
pub const MAX_TURN_START_PARAMS_BYTES: usize = 1024 * 1024 - 8192;
/// Request frame size that native stdio and HTTP accept, sized for one base64 image plus envelope.
pub const MAX_REQUEST_FRAME_BYTES: usize = 8 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum InputMediaType {
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
}
impl InputMediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputPutParams {
    pub session_id: String,
    pub media_type: InputMediaType,
    pub data_base64: String,
    /// Host-side label only; validated and never forwarded to a provider.
    #[serde(default)]
    pub display_name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputPutResult {
    pub input_id: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnParams {
    pub turn_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalParams {
    pub approval_id: String,
    pub allow: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsParams {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubscriptionParams {
    pub subscription_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    pub events: Vec<Event>,
    pub cursor: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnStarted {
    pub turn_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub revision: String,
    pub mode: DataMode,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub valid: bool,
    pub mode: DataMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationAckParams {
    pub issue_id: String,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppRevokeParams {
    pub target_client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthView {
    pub summary: HealthSummary,
    pub issues: Vec<Issue>,
}

/// This is the superset of compiled public methods, not a grant or an assertion of live provider support.
#[derive(JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct MethodInputs {
    #[serde(rename = "auth.chatgpt.status")]
    chatgpt_status: EmptyParams,
    #[serde(rename = "auth.chatgpt.start")]
    chatgpt_start: EmptyParams,
    #[serde(rename = "auth.chatgpt.cancel")]
    chatgpt_cancel: ChatgptCancelParams,
    #[serde(rename = "auth.chatgpt.disconnect")]
    chatgpt_disconnect: EmptyParams,
    #[serde(rename = "runtime.closeClient")]
    close_client: EmptyParams,
    #[serde(rename = "artifacts.read")]
    artifact: ArtifactReadParams,
    #[serde(rename = "runtime.handshake")]
    handshake: HandshakeParams,
    #[serde(rename = "runtime.health")]
    health: EmptyParams,
    #[serde(rename = "snapshot.get")]
    snapshot: SnapshotParams,
    #[serde(rename = "discovery.refresh")]
    discovery: DiscoveryParams,
    #[serde(rename = "discovery.watch")]
    discovery_watch: DiscoveryWatchParams,
    #[serde(rename = "connections.list")]
    connections: EmptyParams,
    #[serde(rename = "connections.probe")]
    probe: ConnectionParams,
    #[serde(rename = "models.list")]
    models: ModelsParams,
    #[serde(rename = "models.page")]
    model_page: ModelPageParams,
    #[serde(rename = "usage.read")]
    usage: EmptyParams,
    #[serde(rename = "costs.read")]
    costs: EmptyParams,
    #[serde(rename = "costs.setPrices")]
    cost_prices: SetPricesParams,
    #[serde(rename = "capabilities.read")]
    capabilities: ModelParams,
    #[serde(rename = "selection.resolve")]
    resolve: Requirements,
    #[serde(rename = "selection.validate")]
    validate: SelectionParams,
    #[serde(rename = "grants.request")]
    grant_request: GrantParams,
    #[serde(rename = "grants.list")]
    grants: EmptyParams,
    #[serde(rename = "grants.revoke")]
    revoke: EmptyParams,
    #[serde(rename = "sessions.create")]
    create: SessionCreateParams,
    #[serde(rename = "sessions.get")]
    session: SessionParams,
    #[serde(rename = "sessions.suspend")]
    suspend_session: SessionParams,
    #[serde(rename = "sessions.resume")]
    resume_session: SessionResumeParams,
    #[serde(rename = "sessions.close")]
    close: SessionParams,
    #[serde(rename = "sessions.updateOptions")]
    options: SessionOptionsParams,
    #[serde(rename = "inputs.put")]
    put_input: InputPutParams,
    #[serde(rename = "turns.start")]
    start: TurnStartParams,
    #[serde(rename = "turns.cancel")]
    cancel: TurnParams,
    #[serde(rename = "approvals.respond")]
    approve: ApprovalParams,
    #[serde(rename = "events.read")]
    events: EventsParams,
    #[serde(rename = "events.subscribe")]
    subscribe: EventsParams,
    #[serde(rename = "events.unsubscribe")]
    unsubscribe: SubscriptionParams,
    #[serde(rename = "adapters.list")]
    adapters: EmptyParams,
    #[serde(rename = "diagnostics.report")]
    diagnostics: EmptyParams,
    #[serde(rename = "health.get")]
    health_view: EmptyParams,
    #[serde(rename = "issues.list")]
    issues_list: EmptyParams,
    #[serde(rename = "issues.acknowledge")]
    issues_acknowledge: IssueUpdateParams,
    #[serde(rename = "issues.snooze")]
    issues_snooze: IssueUpdateParams,
    #[serde(rename = "notifications.poll")]
    notifications_poll: EmptyParams,
    #[serde(rename = "notifications.configure")]
    notifications_configure: NotificationPreferences,
    #[serde(rename = "notifications.ack")]
    notifications_ack: NotificationAckParams,
    #[serde(rename = "apps.list")]
    apps_list: EmptyParams,
    #[serde(rename = "apps.revoke")]
    apps_revoke: AppRevokeParams,
}
#[derive(JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct MethodOutputs {
    #[serde(rename = "auth.chatgpt.status")]
    chatgpt_status: ChatgptAuthStatus,
    #[serde(rename = "auth.chatgpt.start")]
    chatgpt_start: ChatgptAuthStart,
    #[serde(rename = "auth.chatgpt.cancel")]
    chatgpt_cancel: Value,
    #[serde(rename = "auth.chatgpt.disconnect")]
    chatgpt_disconnect: Value,
    #[serde(rename = "runtime.closeClient")]
    close_client: Value,
    #[serde(rename = "artifacts.read")]
    artifact: ArtifactChunk,
    #[serde(rename = "runtime.handshake")]
    handshake: HandshakeResult,
    #[serde(rename = "runtime.health")]
    health: RuntimeHealth,
    #[serde(rename = "snapshot.get")]
    snapshot: Snapshot,
    #[serde(rename = "discovery.refresh")]
    discovery: DiscoveryResult,
    #[serde(rename = "discovery.watch")]
    discovery_watch: Value,
    #[serde(rename = "connections.list")]
    connections: Vec<Connection>,
    #[serde(rename = "connections.probe")]
    probe: Value,
    #[serde(rename = "models.list")]
    models: Vec<Model>,
    #[serde(rename = "models.page")]
    model_page: ModelPage,
    #[serde(rename = "usage.read")]
    usage: Vec<QuotaPool>,
    #[serde(rename = "costs.read")]
    costs: CostReport,
    #[serde(rename = "costs.setPrices")]
    cost_prices: Vec<ModelPrice>,
    #[serde(rename = "capabilities.read")]
    capabilities: Vec<Capability>,
    #[serde(rename = "selection.resolve")]
    resolve: Vec<Candidate>,
    #[serde(rename = "selection.validate")]
    validate: ValidationResult,
    #[serde(rename = "grants.request")]
    grant_request: Value,
    #[serde(rename = "grants.list")]
    grants: Value,
    #[serde(rename = "grants.revoke")]
    revoke: Value,
    #[serde(rename = "sessions.create")]
    create: Session,
    #[serde(rename = "sessions.get")]
    session: Session,
    #[serde(rename = "sessions.suspend")]
    suspend_session: SessionSuspendResult,
    #[serde(rename = "sessions.resume")]
    resume_session: Session,
    #[serde(rename = "sessions.close")]
    close: Value,
    #[serde(rename = "sessions.updateOptions")]
    options: Session,
    #[serde(rename = "inputs.put")]
    put_input: InputPutResult,
    #[serde(rename = "turns.start")]
    start: TurnStarted,
    #[serde(rename = "turns.cancel")]
    cancel: Value,
    #[serde(rename = "approvals.respond")]
    approve: Value,
    #[serde(rename = "events.read")]
    events: EventPage,
    #[serde(rename = "events.subscribe")]
    subscribe: Value,
    #[serde(rename = "events.unsubscribe")]
    unsubscribe: Value,
    #[serde(rename = "adapters.list")]
    adapters: Vec<AdapterDescriptor>,
    #[serde(rename = "diagnostics.report")]
    diagnostics: Value,
    #[serde(rename = "health.get")]
    health_view: HealthView,
    #[serde(rename = "issues.list")]
    issues_list: Vec<Issue>,
    #[serde(rename = "issues.acknowledge")]
    issues_acknowledge: IssueUpdateResult,
    #[serde(rename = "issues.snooze")]
    issues_snooze: IssueUpdateResult,
    #[serde(rename = "notifications.poll")]
    notifications_poll: Vec<NotificationDecision>,
    #[serde(rename = "notifications.configure")]
    notifications_configure: NotificationPreferences,
    #[serde(rename = "notifications.ack")]
    notifications_ack: Value,
    #[serde(rename = "apps.list")]
    apps_list: Value,
    #[serde(rename = "apps.revoke")]
    apps_revoke: Value,
}
