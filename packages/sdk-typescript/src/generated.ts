// Generated from originlm-contracts. Do not edit; run python scripts/generate_contracts.py.

export type AccessRoute = { "billing": Observation4; "capabilities": Array<Capability>; "catalogRevision": string; "connectionId": string; "displayName": string; "id": string; "providerId": string; };

export type Account = { "displayLabel": Observation; "id": string; "identitySource": Observation; "providerId": string; "scopes"?: Array<AccountScope>; };

export type AccountScope = { "id": Observation; "kind": string; "label": Observation; };

export type AdapterDescriptor = { "cancellation": boolean; "id": string; "integration": string; "limitations": Array<string>; "metadata": boolean; "name": string; "nativeToolApproval": boolean; "passiveDiscovery": boolean; "persistentResumption": boolean; "qualification": string; "textSessions": boolean; };

export type AppRevokeParams = { "targetClientId": string; };

export type ApprovalParams = { "allow": boolean; "approvalId": string; };

export type ArtifactChunk = { "artifactId": string; "dataBase64": string; "displayName": string; "eof": boolean; "mediaType": string; "nextOffset": string; "offset": string; "sessionId": string; "totalBytes": string; };

export type ArtifactReadParams = { "artifactId": string; "maxBytes": number; "offset"?: string; "sessionId": string; };

export type AuthState = "authenticated" | "unauthenticated" | "expired" | "unchecked";

export type BillingMode = "subscription" | "api_metered" | "prepaid_credits" | "local" | "mixed" | "unknown";

export type Candidate = { "connectionId": string; "eligible": boolean; "reasons": Array<string>; "selection": Selection; [key: string]: unknown; };

export type Capability = { "conditions": Array<string>; "evidence": Observation; "id": string; "origin": CapabilityOrigin; "support": Support; };

export type CapabilityOrigin = "model" | "agent_tool" | "runtime" | "transport";

export type ChatgptAuthStart = { "authUrl": string; "loginId": string; [key: string]: unknown; };

export type ChatgptAuthState = "signed_out" | "pending" | "connected" | "error";

export type ChatgptAuthStatus = { "email"?: string | null; "errorCode"?: string | null; "loginId"?: string | null; "state": ChatgptAuthState; [key: string]: unknown; };

export type ChatgptCancelParams = { "loginId": string; };

export type Choice = { "disabledReason"?: string | null; "label": string; "value": string; };

export type Connection = { "accountId"?: string | null; "auth": Observation2; "diagnostics": Array<string>; "displayName": string; "id": string; "installationId": string; "plan": Observation3; "profileId": string; "providerId": string; "status": string; };

export type ConnectionMetadata = { "account"?: Account | null; "additionalRoutes"?: Array<AccessRoute>; "auth": Observation2; "billing": Observation4; "connectionId": string; "diagnostics": Array<string>; "includedUsageAllowed": Observation6; "models": Array<Model>; "plan": Observation3; "providers"?: Array<Provider>; "quotaPools": Array<QuotaPool>; "routeId": string; "version": Observation; };

export type ConnectionParams = { "connectionId": string; };

export type CostRecord = { "amountUsd"?: DecimalString | null; "basis": string; "nativeModelId": string; "price"?: ModelPrice | null; "providerId": string; "usage": CostUsage; };

export type CostReport = { "canEditPrices": boolean; "prices": Array<ModelPrice>; "pricesPersisted": boolean; "records": Array<CostRecord>; "scope": string; "storageError"?: string | null; };

export type CostUsage = { "complete": boolean; "connectionId": string; "inputTokens"?: DecimalString | null; "modelId": string; "observedAt": Timestamp; "outputTokens"?: DecimalString | null; "reportedUsd"?: DecimalString | null; "routeId": string; "sessionId": string; "turnId": string; };

export type DataMode = "live" | "synthetic";

export type DecimalString = string;

export type DiscoveryParams = { "level"?: string | null; };

export type DiscoveryResult = { "mode": DataMode; "revision": string; [key: string]: unknown; };

export type DiscoveryWatchParams = { "enabled": boolean; };

export type DomainError = { "code": ErrorCode; "message": string; "retryAfterMs"?: number | null; "retryable": boolean; };

export type EmptyParams = {  };

export type ErrorCode = "INVALID_REQUEST" | "LOCAL_ACCESS_DENIED" | "AUTH_REQUIRED" | "KEYRING_LOCKED" | "RUNTIME_MISSING" | "INTEGRATION_RESTRICTED" | "PROTOCOL_INCOMPATIBLE" | "CAPABILITY_UNKNOWN" | "CAPABILITY_UNSUPPORTED" | "OPTION_UNSUPPORTED" | "SELECTION_STALE" | "RATE_LIMITED" | "BILLING_CONSENT_REQUIRED" | "APPROVAL_REQUIRED" | "SESSION_ROUTE_MISMATCH" | "EVENT_GAP" | "PROVIDER_UNAVAILABLE" | "STORAGE_UNAVAILABLE" | "TIMEOUT" | "CANCELLED" | "CONFLICT" | "FRAME_TOO_LARGE" | "NOT_FOUND" | "NOT_IMPLEMENTED" | "RESUME_UNAVAILABLE";

export type Event = { "adapterId": string; "payload": EventPayload; "schemaVersion": string; "sequence": string; "sessionId": string; "timestamp": Timestamp; "turnId"?: string | null; };

export type EventPage = { "cursor": string; "events": Array<Event>; [key: string]: unknown; };

export type EventPayload = { "state": SessionState; "type": "state"; [key: string]: unknown; } | { "text": string; "type": "text_delta"; [key: string]: unknown; } | { "text": string; "type": "reasoning_summary"; [key: string]: unknown; } | { "action": string; "approvalId": string; "detail": string; "type": "approval_request"; [key: string]: unknown; } | { "allowed": boolean; "approvalId": string; "type": "approval_resolved"; [key: string]: unknown; } | { "name": string; "status": string; "type": "tool"; [key: string]: unknown; } | { "type": "usage"; "unit": string; "used": string; [key: string]: unknown; } | { "artifactId": string; "mediaType": string; "size": string; "type": "artifact"; [key: string]: unknown; } | { "revision": string; "type": "options_changed"; [key: string]: unknown; } | { "code": string; "message": string; "type": "warning"; [key: string]: unknown; } | { "code": string; "message": string; "type": "error"; [key: string]: unknown; } | { "status": string; "type": "final"; [key: string]: unknown; } | { "data": unknown; "namespace": string; "type": "extension"; [key: string]: unknown; };

export type EventsParams = { "after"?: string | null; "sessionId"?: string | null; };

export type EvidenceKind = "official_protocol" | "official_api" | "local_config" | "local_log" | "community_endpoint" | "user_supplied" | "derived" | "synthetic";

export type EvidenceSource = { "adapterId": string; "adapterVersion": string; "clientVersion"?: string | null; "kind": EvidenceKind; "reference"?: string | null; };

export type Freshness = "fresh" | "stale";

export type GrantParams = { "allowPaid"?: boolean; "connectionIds"?: Array<string>; "scopes"?: Array<string>; };

export type HandshakeParams = { "clientName"?: string | null; "protocolVersion"?: string | null; };

export type HandshakeResult = { "capabilities"?: Array<string>; "methods": Array<string>; "protocolVersion": string; "runtimeVersion": string; [key: string]: unknown; };

export type HealthStatus = "healthy" | "degraded" | "unavailable" | "unknown";

export type HealthSummary = { "checkedAt"?: Timestamp | null; "hydrated": boolean; "openIssues": number; "status": HealthStatus; "worstSeverity"?: IssueSeverity | null; };

export type HealthView = { "issues": Array<Issue>; "summary": HealthSummary; [key: string]: unknown; };

export type HostMcpServer = { "allowedTools": Array<string>; "bearerToken": string; "name": string; "url": string; };

export type InputMediaType = "image/jpeg" | "image/png";

export type InputPutParams = { "dataBase64": string; "displayName"?: string | null; "mediaType": InputMediaType; "sessionId": string; };

export type InputPutResult = { "byteLength": number; "height": number; "inputId": string; "width": number; [key: string]: unknown; };

export type Installation = { "environmentId": string; "id": string; "locationHint": string; "providerId": string; "version": Observation; };

export type InventoryChange = { "changes": Array<ResourceChange>; "cursor": string; "issuesChanged": boolean; "observedAt": Timestamp; "previousRevision": string; "reason": string; "revision": string; "sequence": string; };

export type Issue = { "accountId"?: string | null; "connectionId"?: string | null; "evidence": Array<IssueEvidence>; "fingerprint": string; "firstSeenAt": Timestamp; "id": string; "kind": IssueKind; "lastObservation"?: Observation | null; "lastSeenAt": Timestamp; "occurrences": number; "poolId"?: string | null; "providerId"?: string | null; "remediation": Array<Remediation>; "resolvedAt"?: Timestamp | null; "routeId"?: string | null; "severity": IssueSeverity; "snoozedUntil"?: Timestamp | null; "state": IssueState; "summary": string; "title": string; "windowId"?: string | null; };

export type IssueEvidence = { "code": string; "freshness"?: Freshness | null; "message": string; "observedAt"?: Timestamp | null; "source"?: EvidenceSource | null; "verification"?: Verification | null; };

export type IssueKind = "auth_required" | "auth_expired" | "keyring_locked" | "unsupported_client" | "usage_unavailable" | "quota_exhausted" | "rate_limited" | "adapter_failure" | "storage_unavailable" | "permission_denied" | "service_unavailable" | "configuration_invalid" | "unknown";

export type IssueSeverity = "info" | "warning" | "error" | "critical";

export type IssueState = "open" | "acknowledged" | "snoozed" | "resolved";

export type IssueUpdateParams = { "issueId": string; "until"?: Timestamp | null; };

export type IssueUpdateResult = { "issue": Issue; };

export type MethodInputs = { "adapters.list": EmptyParams; "approvals.respond": ApprovalParams; "apps.list": EmptyParams; "apps.revoke": AppRevokeParams; "artifacts.read": ArtifactReadParams; "auth.chatgpt.cancel": ChatgptCancelParams; "auth.chatgpt.disconnect": EmptyParams; "auth.chatgpt.start": EmptyParams; "auth.chatgpt.status": EmptyParams; "capabilities.read": ModelParams; "connections.list": EmptyParams; "connections.probe": ConnectionParams; "costs.read": EmptyParams; "costs.setPrices": SetPricesParams; "diagnostics.report": EmptyParams; "discovery.refresh": DiscoveryParams; "discovery.watch": DiscoveryWatchParams; "events.read": EventsParams; "events.subscribe": EventsParams; "events.unsubscribe": SubscriptionParams; "grants.list": EmptyParams; "grants.request": GrantParams; "grants.revoke": EmptyParams; "health.get": EmptyParams; "inputs.put": InputPutParams; "issues.acknowledge": IssueUpdateParams; "issues.list": EmptyParams; "issues.snooze": IssueUpdateParams; "models.list": ModelsParams; "models.page": ModelPageParams; "notifications.ack": NotificationAckParams; "notifications.configure": NotificationPreferences; "notifications.poll": EmptyParams; "runtime.closeClient": EmptyParams; "runtime.handshake": HandshakeParams; "runtime.health": EmptyParams; "selection.resolve": Requirements; "selection.validate": SelectionParams; "sessions.close": SessionParams; "sessions.create": SessionCreateParams; "sessions.get": SessionParams; "sessions.resume": SessionResumeParams; "sessions.suspend": SessionParams; "sessions.updateOptions": SessionOptionsParams; "snapshot.get": SnapshotParams; "turns.cancel": TurnParams; "turns.start": TurnStartParams; "usage.read": EmptyParams; };

export type MethodOutputs = { "adapters.list": Array<AdapterDescriptor>; "approvals.respond": unknown; "apps.list": unknown; "apps.revoke": unknown; "artifacts.read": ArtifactChunk; "auth.chatgpt.cancel": unknown; "auth.chatgpt.disconnect": unknown; "auth.chatgpt.start": ChatgptAuthStart; "auth.chatgpt.status": ChatgptAuthStatus; "capabilities.read": Array<Capability>; "connections.list": Array<Connection>; "connections.probe": unknown; "costs.read": CostReport; "costs.setPrices": Array<ModelPrice>; "diagnostics.report": unknown; "discovery.refresh": DiscoveryResult; "discovery.watch": unknown; "events.read": EventPage; "events.subscribe": unknown; "events.unsubscribe": unknown; "grants.list": unknown; "grants.request": unknown; "grants.revoke": unknown; "health.get": HealthView; "inputs.put": InputPutResult; "issues.acknowledge": IssueUpdateResult; "issues.list": Array<Issue>; "issues.snooze": IssueUpdateResult; "models.list": Array<Model>; "models.page": ModelPage; "notifications.ack": unknown; "notifications.configure": NotificationPreferences; "notifications.poll": Array<NotificationDecision>; "runtime.closeClient": unknown; "runtime.handshake": HandshakeResult; "runtime.health": RuntimeHealth; "selection.resolve": Array<Candidate>; "selection.validate": ValidationResult; "sessions.close": unknown; "sessions.create": Session; "sessions.get": Session; "sessions.resume": Session; "sessions.suspend": SessionSuspendResult; "sessions.updateOptions": Session; "snapshot.get": Snapshot; "turns.cancel": unknown; "turns.start": TurnStarted; "usage.read": Array<QuotaPool>; };

export type Model = { "capabilities": Array<Capability>; "contextWindow": Observation; "id": string; "isDefault": boolean; "name": string; "nativeId": string; "options": Array<OptionDescriptor>; "revision": string; "routeId": string; };

export type ModelPage = { "models": Array<Model>; "nextCursor"?: string | null; "revision": string; };

export type ModelPageParams = { "cursor"?: string | null; "limit"?: number | null; "routeId"?: string | null; };

export type ModelParams = { "modelId": string; };

export type ModelPrice = { "inputUsdPerMillion": DecimalString; "nativeModelId": string; "outputUsdPerMillion": DecimalString; "providerId": string; "reviewedAt": string; "source": string; };

export type ModelsParams = { "routeId"?: string | null; };

export type NotificationAckParams = { "dedupeKey": string; "issueId": string; };

export type NotificationDecision = { "body": string; "dedupeKey": string; "issueId": string; "reason": string; "severity": IssueSeverity; "shouldNotify": boolean; "title": string; };

export type NotificationPreferences = { "enabled"?: boolean; "minimumSeverity"?: IssueSeverity; "privateContent"?: boolean; "quietHoursEndMinute"?: number | null; "quietHoursStartMinute"?: number | null; "reminderAfterSeconds"?: number; };

export type Observation = { "expiresAt"?: Timestamp | null; "freshness": Freshness; "observedAt": Timestamp; "source": EvidenceSource; "state": "known"; "value": string; "verification": Verification; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unknown"; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unavailable"; };

export type Observation2 = { "expiresAt"?: Timestamp | null; "freshness": Freshness; "observedAt": Timestamp; "source": EvidenceSource; "state": "known"; "value": AuthState; "verification": Verification; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unknown"; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unavailable"; };

export type Observation3 = { "expiresAt"?: Timestamp | null; "freshness": Freshness; "observedAt": Timestamp; "source": EvidenceSource; "state": "known"; "value": Plan; "verification": Verification; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unknown"; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unavailable"; };

export type Observation4 = { "expiresAt"?: Timestamp | null; "freshness": Freshness; "observedAt": Timestamp; "source": EvidenceSource; "state": "known"; "value": BillingMode; "verification": Verification; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unknown"; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unavailable"; };

export type Observation5 = { "expiresAt"?: Timestamp | null; "freshness": Freshness; "observedAt": Timestamp; "source": EvidenceSource; "state": "known"; "value": UsageReading; "verification": Verification; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unknown"; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unavailable"; };

export type Observation6 = { "expiresAt"?: Timestamp | null; "freshness": Freshness; "observedAt": Timestamp; "source": EvidenceSource; "state": "known"; "value": boolean; "verification": Verification; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unknown"; } | { "checkedAt"?: Timestamp | null; "reason": string; "state": "unavailable"; };

export type OptionControl = { "choices": Array<Choice>; "defaultValue"?: string | null; "type": "select"; } | { "type": "boolean"; } | { "max": number; "min": number; "step": number; "type": "number"; } | { "maxLength": number; "type": "text"; } | { "nativeType": string; "type": "unknown"; };

export type OptionDescriptor = { "billingSensitive"?: boolean; "category"?: string | null; "control": OptionControl; "currentValue"?: unknown; "id": string; "label": string; "permissionSensitive"?: boolean; "revision": string; "scope": OptionScope; "source"?: EvidenceSource | null; };

export type OptionScope = "model" | "session" | "turn";

export type Plan = { "label": string; "nativeId": string; };

export type Profile = { "id": string; "installationId": string; "label": string; };

export type Provider = { "id": string; "kind": string; "name": string; };

export type QuotaPool = { "accountId"?: string | null; "connectionIds": Array<string>; "id": string; "routeIds": Array<string>; "sharing": string; "windows": Array<UsageWindow>; };

export type Remediation = { "description": string; "kind": RemediationKind; "label": string; "requiresUserAction": boolean; };

export type RemediationKind = "refresh" | "sign_in" | "unlock_keyring" | "update_client" | "open_settings" | "retry_later" | "restart_service" | "free_storage" | "contact_support" | "none";

export type Requirements = { "allowPaidFallback"?: boolean; "billingPreference"?: string | null; "eligibleOnly"?: boolean; "query"?: string | null; "requiredCapabilities"?: Array<string>; };

export type ResourceChange = { "change": ResourceChangeKind; "id": string; "reason"?: string | null; "resource": string; };

export type ResourceChangeKind = "added" | "updated" | "removed";

export type RuntimeHealth = { "executionAvailable": boolean; "metadataAvailable": boolean; "mode": string; "status": string; [key: string]: unknown; };

export type Selection = { "modelId": string; "options"?: { [key: string]: unknown; }; "revision": string; "routeId": string; };

export type SelectionParams = { "allowPaid"?: boolean; "selection": Selection; };

export type Session = { "connectionId": string; "createdAt": Timestamp; "id": string; "nativeResumption": boolean; "selection": Selection; "state": SessionState; };

export type SessionCreateParams = { "allowPaid"?: boolean; "hostMcpServers"?: Array<HostMcpServer>; "permissionMode"?: string | null; "persistent"?: boolean; "reviewRequests"?: boolean; "selection": Selection; "workspaceId": string; };

export type SessionOptionsParams = { "options": { [key: string]: unknown; }; "sessionId": string; };

export type SessionParams = { "sessionId": string; };

export type SessionResumeParams = { "hostMcpServers"?: Array<HostMcpServer>; "resumeToken": string; };

export type SessionState = "ready" | "running" | "awaiting_approval" | "completed" | "error" | "cancelled" | "closed";

export type SessionSuspendResult = { "resumeToken": string; };

export type SetPricesParams = { "prices": Array<ModelPrice>; };

export type Snapshot = { "accounts": Array<Account>; "connections": Array<Connection>; "cursor"?: string; "diagnostics": Array<string>; "health"?: HealthSummary; "installations": Array<Installation>; "issues"?: Array<Issue>; "mode": DataMode; "models": Array<Model>; "observedAt": Timestamp; "profiles": Array<Profile>; "protocolVersion": string; "providers": Array<Provider>; "quotaPools": Array<QuotaPool>; "revision": string; "routes": Array<AccessRoute>; };

export type SnapshotParams = { "fields"?: Array<string> | null; };

export type SubscriptionParams = { "subscriptionId": string; };

export type Support = "supported" | "unsupported" | "conditional" | "unknown";

export type Timestamp = string;

export type TurnInputRef = { "inputId": string; };

export type TurnParams = { "turnId": string; };

export type TurnStartParams = { "inputs"?: Array<TurnInputRef>; "prompt": string; "requestId": string; "sessionId": string; };

export type TurnStarted = { "turnId": string; [key: string]: unknown; };

export type UsageReading = { "limit"?: DecimalString | null; "remaining"?: DecimalString | null; "unit": string; "used": DecimalString; "usedFraction"?: number | null; };

export type UsageWindow = { "durationSeconds"?: number | null; "id": string; "label": string; "reading": Observation5; "resetsAt"?: Timestamp | null; };

export type ValidationResult = { "mode": DataMode; "valid": boolean; [key: string]: unknown; };

export type Verification = "reported" | "observed" | "inferred";
