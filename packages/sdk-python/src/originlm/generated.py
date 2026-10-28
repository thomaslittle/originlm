# Generated from originlm-contracts. Do not edit; run python scripts/generate_contracts.py.
from typing import Any, Dict, List, Literal, Never, NotRequired, Required, TypeAlias, TypedDict, Union

AccessRoute = TypedDict('AccessRoute', {'billing': Required['Observation4'], 'capabilities': Required[List['Capability']], 'catalogRevision': Required[str], 'connectionId': Required[str], 'displayName': Required[str], 'id': Required[str], 'providerId': Required[str]}, total=False)

Account = TypedDict('Account', {'displayLabel': Required['Observation'], 'id': Required[str], 'identitySource': Required['Observation'], 'providerId': Required[str], 'scopes': NotRequired[List['AccountScope']]}, total=False)

AccountScope = TypedDict('AccountScope', {'id': Required['Observation'], 'kind': Required[str], 'label': Required['Observation']}, total=False)

AdapterDescriptor = TypedDict('AdapterDescriptor', {'cancellation': Required[bool], 'id': Required[str], 'integration': Required[str], 'limitations': Required[List[str]], 'metadata': Required[bool], 'name': Required[str], 'nativeToolApproval': Required[bool], 'passiveDiscovery': Required[bool], 'persistentResumption': Required[bool], 'qualification': Required[str], 'textSessions': Required[bool]}, total=False)

AppRevokeParams = TypedDict('AppRevokeParams', {'targetClientId': Required[str]}, total=False)

ApprovalParams = TypedDict('ApprovalParams', {'allow': Required[bool], 'approvalId': Required[str]}, total=False)

ArtifactChunk = TypedDict('ArtifactChunk', {'artifactId': Required[str], 'dataBase64': Required[str], 'displayName': Required[str], 'eof': Required[bool], 'mediaType': Required[str], 'nextOffset': Required[str], 'offset': Required[str], 'sessionId': Required[str], 'totalBytes': Required[str]}, total=False)

ArtifactReadParams = TypedDict('ArtifactReadParams', {'artifactId': Required[str], 'maxBytes': Required[int], 'offset': NotRequired[str], 'sessionId': Required[str]}, total=False)

AuthState: TypeAlias = Literal['authenticated', 'unauthenticated', 'expired', 'unchecked']

BillingMode: TypeAlias = Literal['subscription', 'api_metered', 'prepaid_credits', 'local', 'mixed', 'unknown']

Candidate = TypedDict('Candidate', {'connectionId': Required[str], 'eligible': Required[bool], 'reasons': Required[List[str]], 'selection': Required['Selection']}, total=False)

Capability = TypedDict('Capability', {'conditions': Required[List[str]], 'evidence': Required['Observation'], 'id': Required[str], 'origin': Required['CapabilityOrigin'], 'support': Required['Support']}, total=False)

CapabilityOrigin: TypeAlias = Literal['model', 'agent_tool', 'runtime', 'transport']

ChatgptAuthStart = TypedDict('ChatgptAuthStart', {'authUrl': Required[str], 'loginId': Required[str]}, total=False)

ChatgptAuthState: TypeAlias = Literal['signed_out', 'pending', 'connected', 'error']

ChatgptAuthStatus = TypedDict('ChatgptAuthStatus', {'email': NotRequired[Union[str, None]], 'errorCode': NotRequired[Union[str, None]], 'loginId': NotRequired[Union[str, None]], 'state': Required['ChatgptAuthState']}, total=False)

ChatgptCancelParams = TypedDict('ChatgptCancelParams', {'loginId': Required[str]}, total=False)

Choice = TypedDict('Choice', {'disabledReason': NotRequired[Union[str, None]], 'label': Required[str], 'value': Required[str]}, total=False)

Connection = TypedDict('Connection', {'accountId': NotRequired[Union[str, None]], 'auth': Required['Observation2'], 'diagnostics': Required[List[str]], 'displayName': Required[str], 'id': Required[str], 'installationId': Required[str], 'plan': Required['Observation3'], 'profileId': Required[str], 'providerId': Required[str], 'status': Required[str]}, total=False)

ConnectionMetadata = TypedDict('ConnectionMetadata', {'account': NotRequired[Union['Account', None]], 'additionalRoutes': NotRequired[List['AccessRoute']], 'auth': Required['Observation2'], 'billing': Required['Observation4'], 'connectionId': Required[str], 'diagnostics': Required[List[str]], 'includedUsageAllowed': Required['Observation6'], 'models': Required[List['Model']], 'plan': Required['Observation3'], 'providers': NotRequired[List['Provider']], 'quotaPools': Required[List['QuotaPool']], 'routeId': Required[str], 'version': Required['Observation']}, total=False)

ConnectionParams = TypedDict('ConnectionParams', {'connectionId': Required[str]}, total=False)

CostRecord = TypedDict('CostRecord', {'amountUsd': NotRequired[Union['DecimalString', None]], 'basis': Required[str], 'nativeModelId': Required[str], 'price': NotRequired[Union['ModelPrice', None]], 'providerId': Required[str], 'usage': Required['CostUsage']}, total=False)

CostReport = TypedDict('CostReport', {'canEditPrices': Required[bool], 'prices': Required[List['ModelPrice']], 'pricesPersisted': Required[bool], 'records': Required[List['CostRecord']], 'scope': Required[str], 'storageError': NotRequired[Union[str, None]]}, total=False)

CostUsage = TypedDict('CostUsage', {'complete': Required[bool], 'connectionId': Required[str], 'inputTokens': NotRequired[Union['DecimalString', None]], 'modelId': Required[str], 'observedAt': Required['Timestamp'], 'outputTokens': NotRequired[Union['DecimalString', None]], 'reportedUsd': NotRequired[Union['DecimalString', None]], 'routeId': Required[str], 'sessionId': Required[str], 'turnId': Required[str]}, total=False)

DataMode: TypeAlias = Literal['live', 'synthetic']

DecimalString: TypeAlias = str

DiscoveryParams = TypedDict('DiscoveryParams', {'level': NotRequired[Union[str, None]]}, total=False)

DiscoveryResult = TypedDict('DiscoveryResult', {'mode': Required['DataMode'], 'revision': Required[str]}, total=False)

DiscoveryWatchParams = TypedDict('DiscoveryWatchParams', {'enabled': Required[bool]}, total=False)

DomainError = TypedDict('DomainError', {'code': Required['ErrorCode'], 'message': Required[str], 'retryAfterMs': NotRequired[Union[int, None]], 'retryable': Required[bool]}, total=False)

EmptyParams: TypeAlias = Dict[str, Never]

ErrorCode: TypeAlias = Union[Literal['INVALID_REQUEST', 'LOCAL_ACCESS_DENIED', 'AUTH_REQUIRED', 'KEYRING_LOCKED', 'RUNTIME_MISSING', 'INTEGRATION_RESTRICTED', 'PROTOCOL_INCOMPATIBLE', 'CAPABILITY_UNKNOWN', 'CAPABILITY_UNSUPPORTED', 'OPTION_UNSUPPORTED', 'SELECTION_STALE', 'RATE_LIMITED', 'BILLING_CONSENT_REQUIRED', 'APPROVAL_REQUIRED', 'SESSION_ROUTE_MISMATCH', 'EVENT_GAP', 'PROVIDER_UNAVAILABLE', 'STORAGE_UNAVAILABLE', 'TIMEOUT', 'CANCELLED', 'CONFLICT', 'FRAME_TOO_LARGE', 'NOT_FOUND', 'NOT_IMPLEMENTED'], Literal['RESUME_UNAVAILABLE']]

Event = TypedDict('Event', {'adapterId': Required[str], 'payload': Required['EventPayload'], 'schemaVersion': Required[str], 'sequence': Required[str], 'sessionId': Required[str], 'timestamp': Required['Timestamp'], 'turnId': NotRequired[Union[str, None]]}, total=False)

EventPage = TypedDict('EventPage', {'cursor': Required[str], 'events': Required[List['Event']]}, total=False)

EventPayload_Case0 = TypedDict('EventPayload_Case0', {'state': Required['SessionState'], 'type': Required[Literal['state']]}, total=False)

EventPayload_Case1 = TypedDict('EventPayload_Case1', {'text': Required[str], 'type': Required[Literal['text_delta']]}, total=False)

EventPayload_Case2 = TypedDict('EventPayload_Case2', {'text': Required[str], 'type': Required[Literal['reasoning_summary']]}, total=False)

EventPayload_Case3 = TypedDict('EventPayload_Case3', {'action': Required[str], 'approvalId': Required[str], 'detail': Required[str], 'type': Required[Literal['approval_request']]}, total=False)

EventPayload_Case4 = TypedDict('EventPayload_Case4', {'allowed': Required[bool], 'approvalId': Required[str], 'type': Required[Literal['approval_resolved']]}, total=False)

EventPayload_Case5 = TypedDict('EventPayload_Case5', {'name': Required[str], 'status': Required[str], 'type': Required[Literal['tool']]}, total=False)

EventPayload_Case6 = TypedDict('EventPayload_Case6', {'type': Required[Literal['usage']], 'unit': Required[str], 'used': Required[str]}, total=False)

EventPayload_Case7 = TypedDict('EventPayload_Case7', {'artifactId': Required[str], 'mediaType': Required[str], 'size': Required[str], 'type': Required[Literal['artifact']]}, total=False)

EventPayload_Case8 = TypedDict('EventPayload_Case8', {'revision': Required[str], 'type': Required[Literal['options_changed']]}, total=False)

EventPayload_Case9 = TypedDict('EventPayload_Case9', {'code': Required[str], 'message': Required[str], 'type': Required[Literal['warning']]}, total=False)

EventPayload_Case10 = TypedDict('EventPayload_Case10', {'code': Required[str], 'message': Required[str], 'type': Required[Literal['error']]}, total=False)

EventPayload_Case11 = TypedDict('EventPayload_Case11', {'status': Required[str], 'type': Required[Literal['final']]}, total=False)

EventPayload_Case12 = TypedDict('EventPayload_Case12', {'data': Required[Any], 'namespace': Required[str], 'type': Required[Literal['extension']]}, total=False)

EventPayload: TypeAlias = Union[EventPayload_Case0, EventPayload_Case1, EventPayload_Case2, EventPayload_Case3, EventPayload_Case4, EventPayload_Case5, EventPayload_Case6, EventPayload_Case7, EventPayload_Case8, EventPayload_Case9, EventPayload_Case10, EventPayload_Case11, EventPayload_Case12]

EventsParams = TypedDict('EventsParams', {'after': NotRequired[Union[str, None]], 'sessionId': NotRequired[Union[str, None]]}, total=False)

EvidenceKind: TypeAlias = Literal['official_protocol', 'official_api', 'local_config', 'local_log', 'community_endpoint', 'user_supplied', 'derived', 'synthetic']

EvidenceSource = TypedDict('EvidenceSource', {'adapterId': Required[str], 'adapterVersion': Required[str], 'clientVersion': NotRequired[Union[str, None]], 'kind': Required['EvidenceKind'], 'reference': NotRequired[Union[str, None]]}, total=False)

Freshness: TypeAlias = Literal['fresh', 'stale']

GrantParams = TypedDict('GrantParams', {'allowPaid': NotRequired[bool], 'connectionIds': NotRequired[List[str]], 'scopes': NotRequired[List[str]]}, total=False)

HandshakeParams = TypedDict('HandshakeParams', {'clientName': NotRequired[Union[str, None]], 'protocolVersion': NotRequired[Union[str, None]]}, total=False)

HandshakeResult = TypedDict('HandshakeResult', {'capabilities': NotRequired[List[str]], 'methods': Required[List[str]], 'protocolVersion': Required[str], 'runtimeVersion': Required[str]}, total=False)

HealthStatus: TypeAlias = Literal['healthy', 'degraded', 'unavailable', 'unknown']

HealthSummary = TypedDict('HealthSummary', {'checkedAt': NotRequired[Union['Timestamp', None]], 'hydrated': Required[bool], 'openIssues': Required[int], 'status': Required['HealthStatus'], 'worstSeverity': NotRequired[Union['IssueSeverity', None]]}, total=False)

HealthView = TypedDict('HealthView', {'issues': Required[List['Issue']], 'summary': Required['HealthSummary']}, total=False)

HostMcpServer = TypedDict('HostMcpServer', {'allowedTools': Required[List[str]], 'bearerToken': Required[str], 'name': Required[str], 'url': Required[str]}, total=False)

InputMediaType: TypeAlias = Literal['image/jpeg', 'image/png']

InputPutParams = TypedDict('InputPutParams', {'dataBase64': Required[str], 'displayName': NotRequired[Union[str, None]], 'mediaType': Required['InputMediaType'], 'sessionId': Required[str]}, total=False)

InputPutResult = TypedDict('InputPutResult', {'byteLength': Required[int], 'height': Required[int], 'inputId': Required[str], 'width': Required[int]}, total=False)

Installation = TypedDict('Installation', {'environmentId': Required[str], 'id': Required[str], 'locationHint': Required[str], 'providerId': Required[str], 'version': Required['Observation']}, total=False)

InventoryChange = TypedDict('InventoryChange', {'changes': Required[List['ResourceChange']], 'cursor': Required[str], 'issuesChanged': Required[bool], 'observedAt': Required['Timestamp'], 'previousRevision': Required[str], 'reason': Required[str], 'revision': Required[str], 'sequence': Required[str]}, total=False)

Issue = TypedDict('Issue', {'accountId': NotRequired[Union[str, None]], 'connectionId': NotRequired[Union[str, None]], 'evidence': Required[List['IssueEvidence']], 'fingerprint': Required[str], 'firstSeenAt': Required['Timestamp'], 'id': Required[str], 'kind': Required['IssueKind'], 'lastObservation': NotRequired[Union['Observation', None]], 'lastSeenAt': Required['Timestamp'], 'occurrences': Required[int], 'poolId': NotRequired[Union[str, None]], 'providerId': NotRequired[Union[str, None]], 'remediation': Required[List['Remediation']], 'resolvedAt': NotRequired[Union['Timestamp', None]], 'routeId': NotRequired[Union[str, None]], 'severity': Required['IssueSeverity'], 'snoozedUntil': NotRequired[Union['Timestamp', None]], 'state': Required['IssueState'], 'summary': Required[str], 'title': Required[str], 'windowId': NotRequired[Union[str, None]]}, total=False)

IssueEvidence = TypedDict('IssueEvidence', {'code': Required[str], 'freshness': NotRequired[Union['Freshness', None]], 'message': Required[str], 'observedAt': NotRequired[Union['Timestamp', None]], 'source': NotRequired[Union['EvidenceSource', None]], 'verification': NotRequired[Union['Verification', None]]}, total=False)

IssueKind: TypeAlias = Literal['auth_required', 'auth_expired', 'keyring_locked', 'unsupported_client', 'usage_unavailable', 'quota_exhausted', 'rate_limited', 'adapter_failure', 'storage_unavailable', 'permission_denied', 'service_unavailable', 'configuration_invalid', 'unknown']

IssueSeverity: TypeAlias = Literal['info', 'warning', 'error', 'critical']

IssueState: TypeAlias = Union[Literal['open'], Literal['acknowledged'], Literal['snoozed'], Literal['resolved']]

IssueUpdateParams = TypedDict('IssueUpdateParams', {'issueId': Required[str], 'until': NotRequired[Union['Timestamp', None]]}, total=False)

IssueUpdateResult = TypedDict('IssueUpdateResult', {'issue': Required['Issue']}, total=False)

MethodInputs = TypedDict('MethodInputs', {'adapters.list': Required['EmptyParams'], 'approvals.respond': Required['ApprovalParams'], 'apps.list': Required['EmptyParams'], 'apps.revoke': Required['AppRevokeParams'], 'artifacts.read': Required['ArtifactReadParams'], 'auth.chatgpt.cancel': Required['ChatgptCancelParams'], 'auth.chatgpt.disconnect': Required['EmptyParams'], 'auth.chatgpt.start': Required['EmptyParams'], 'auth.chatgpt.status': Required['EmptyParams'], 'capabilities.read': Required['ModelParams'], 'connections.list': Required['EmptyParams'], 'connections.probe': Required['ConnectionParams'], 'costs.read': Required['EmptyParams'], 'costs.setPrices': Required['SetPricesParams'], 'diagnostics.report': Required['EmptyParams'], 'discovery.refresh': Required['DiscoveryParams'], 'discovery.watch': Required['DiscoveryWatchParams'], 'events.read': Required['EventsParams'], 'events.subscribe': Required['EventsParams'], 'events.unsubscribe': Required['SubscriptionParams'], 'grants.list': Required['EmptyParams'], 'grants.request': Required['GrantParams'], 'grants.revoke': Required['EmptyParams'], 'health.get': Required['EmptyParams'], 'inputs.put': Required['InputPutParams'], 'issues.acknowledge': Required['IssueUpdateParams'], 'issues.list': Required['EmptyParams'], 'issues.snooze': Required['IssueUpdateParams'], 'models.list': Required['ModelsParams'], 'models.page': Required['ModelPageParams'], 'notifications.ack': Required['NotificationAckParams'], 'notifications.configure': Required['NotificationPreferences'], 'notifications.poll': Required['EmptyParams'], 'runtime.closeClient': Required['EmptyParams'], 'runtime.handshake': Required['HandshakeParams'], 'runtime.health': Required['EmptyParams'], 'selection.resolve': Required['Requirements'], 'selection.validate': Required['SelectionParams'], 'sessions.close': Required['SessionParams'], 'sessions.create': Required['SessionCreateParams'], 'sessions.get': Required['SessionParams'], 'sessions.resume': Required['SessionResumeParams'], 'sessions.suspend': Required['SessionParams'], 'sessions.updateOptions': Required['SessionOptionsParams'], 'snapshot.get': Required['SnapshotParams'], 'turns.cancel': Required['TurnParams'], 'turns.start': Required['TurnStartParams'], 'usage.read': Required['EmptyParams']}, total=False)

MethodOutputs = TypedDict('MethodOutputs', {'adapters.list': Required[List['AdapterDescriptor']], 'approvals.respond': Required[Any], 'apps.list': Required[Any], 'apps.revoke': Required[Any], 'artifacts.read': Required['ArtifactChunk'], 'auth.chatgpt.cancel': Required[Any], 'auth.chatgpt.disconnect': Required[Any], 'auth.chatgpt.start': Required['ChatgptAuthStart'], 'auth.chatgpt.status': Required['ChatgptAuthStatus'], 'capabilities.read': Required[List['Capability']], 'connections.list': Required[List['Connection']], 'connections.probe': Required[Any], 'costs.read': Required['CostReport'], 'costs.setPrices': Required[List['ModelPrice']], 'diagnostics.report': Required[Any], 'discovery.refresh': Required['DiscoveryResult'], 'discovery.watch': Required[Any], 'events.read': Required['EventPage'], 'events.subscribe': Required[Any], 'events.unsubscribe': Required[Any], 'grants.list': Required[Any], 'grants.request': Required[Any], 'grants.revoke': Required[Any], 'health.get': Required['HealthView'], 'inputs.put': Required['InputPutResult'], 'issues.acknowledge': Required['IssueUpdateResult'], 'issues.list': Required[List['Issue']], 'issues.snooze': Required['IssueUpdateResult'], 'models.list': Required[List['Model']], 'models.page': Required['ModelPage'], 'notifications.ack': Required[Any], 'notifications.configure': Required['NotificationPreferences'], 'notifications.poll': Required[List['NotificationDecision']], 'runtime.closeClient': Required[Any], 'runtime.handshake': Required['HandshakeResult'], 'runtime.health': Required['RuntimeHealth'], 'selection.resolve': Required[List['Candidate']], 'selection.validate': Required['ValidationResult'], 'sessions.close': Required[Any], 'sessions.create': Required['Session'], 'sessions.get': Required['Session'], 'sessions.resume': Required['Session'], 'sessions.suspend': Required['SessionSuspendResult'], 'sessions.updateOptions': Required['Session'], 'snapshot.get': Required['Snapshot'], 'turns.cancel': Required[Any], 'turns.start': Required['TurnStarted'], 'usage.read': Required[List['QuotaPool']]}, total=False)

Model = TypedDict('Model', {'capabilities': Required[List['Capability']], 'contextWindow': Required['Observation'], 'id': Required[str], 'isDefault': Required[bool], 'name': Required[str], 'nativeId': Required[str], 'options': Required[List['OptionDescriptor']], 'revision': Required[str], 'routeId': Required[str]}, total=False)

ModelPage = TypedDict('ModelPage', {'models': Required[List['Model']], 'nextCursor': NotRequired[Union[str, None]], 'revision': Required[str]}, total=False)

ModelPageParams = TypedDict('ModelPageParams', {'cursor': NotRequired[Union[str, None]], 'limit': NotRequired[Union[int, None]], 'routeId': NotRequired[Union[str, None]]}, total=False)

ModelParams = TypedDict('ModelParams', {'modelId': Required[str]}, total=False)

ModelPrice = TypedDict('ModelPrice', {'inputUsdPerMillion': Required['DecimalString'], 'nativeModelId': Required[str], 'outputUsdPerMillion': Required['DecimalString'], 'providerId': Required[str], 'reviewedAt': Required[str], 'source': Required[str]}, total=False)

ModelsParams = TypedDict('ModelsParams', {'routeId': NotRequired[Union[str, None]]}, total=False)

NotificationAckParams = TypedDict('NotificationAckParams', {'dedupeKey': Required[str], 'issueId': Required[str]}, total=False)

NotificationDecision = TypedDict('NotificationDecision', {'body': Required[str], 'dedupeKey': Required[str], 'issueId': Required[str], 'reason': Required[str], 'severity': Required['IssueSeverity'], 'shouldNotify': Required[bool], 'title': Required[str]}, total=False)

NotificationPreferences = TypedDict('NotificationPreferences', {'enabled': NotRequired[bool], 'minimumSeverity': NotRequired['IssueSeverity'], 'privateContent': NotRequired[bool], 'quietHoursEndMinute': NotRequired[Union[int, None]], 'quietHoursStartMinute': NotRequired[Union[int, None]], 'reminderAfterSeconds': NotRequired[int]}, total=False)

Observation_Case0 = TypedDict('Observation_Case0', {'expiresAt': NotRequired[Union['Timestamp', None]], 'freshness': Required['Freshness'], 'observedAt': Required['Timestamp'], 'source': Required['EvidenceSource'], 'state': Required[Literal['known']], 'value': Required[str], 'verification': Required['Verification']}, total=False)

Observation_Case1 = TypedDict('Observation_Case1', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unknown']]}, total=False)

Observation_Case2 = TypedDict('Observation_Case2', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unavailable']]}, total=False)

Observation: TypeAlias = Union[Observation_Case0, Observation_Case1, Observation_Case2]

Observation2_Case0 = TypedDict('Observation2_Case0', {'expiresAt': NotRequired[Union['Timestamp', None]], 'freshness': Required['Freshness'], 'observedAt': Required['Timestamp'], 'source': Required['EvidenceSource'], 'state': Required[Literal['known']], 'value': Required['AuthState'], 'verification': Required['Verification']}, total=False)

Observation2_Case1 = TypedDict('Observation2_Case1', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unknown']]}, total=False)

Observation2_Case2 = TypedDict('Observation2_Case2', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unavailable']]}, total=False)

Observation2: TypeAlias = Union[Observation2_Case0, Observation2_Case1, Observation2_Case2]

Observation3_Case0 = TypedDict('Observation3_Case0', {'expiresAt': NotRequired[Union['Timestamp', None]], 'freshness': Required['Freshness'], 'observedAt': Required['Timestamp'], 'source': Required['EvidenceSource'], 'state': Required[Literal['known']], 'value': Required['Plan'], 'verification': Required['Verification']}, total=False)

Observation3_Case1 = TypedDict('Observation3_Case1', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unknown']]}, total=False)

Observation3_Case2 = TypedDict('Observation3_Case2', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unavailable']]}, total=False)

Observation3: TypeAlias = Union[Observation3_Case0, Observation3_Case1, Observation3_Case2]

Observation4_Case0 = TypedDict('Observation4_Case0', {'expiresAt': NotRequired[Union['Timestamp', None]], 'freshness': Required['Freshness'], 'observedAt': Required['Timestamp'], 'source': Required['EvidenceSource'], 'state': Required[Literal['known']], 'value': Required['BillingMode'], 'verification': Required['Verification']}, total=False)

Observation4_Case1 = TypedDict('Observation4_Case1', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unknown']]}, total=False)

Observation4_Case2 = TypedDict('Observation4_Case2', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unavailable']]}, total=False)

Observation4: TypeAlias = Union[Observation4_Case0, Observation4_Case1, Observation4_Case2]

Observation5_Case0 = TypedDict('Observation5_Case0', {'expiresAt': NotRequired[Union['Timestamp', None]], 'freshness': Required['Freshness'], 'observedAt': Required['Timestamp'], 'source': Required['EvidenceSource'], 'state': Required[Literal['known']], 'value': Required['UsageReading'], 'verification': Required['Verification']}, total=False)

Observation5_Case1 = TypedDict('Observation5_Case1', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unknown']]}, total=False)

Observation5_Case2 = TypedDict('Observation5_Case2', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unavailable']]}, total=False)

Observation5: TypeAlias = Union[Observation5_Case0, Observation5_Case1, Observation5_Case2]

Observation6_Case0 = TypedDict('Observation6_Case0', {'expiresAt': NotRequired[Union['Timestamp', None]], 'freshness': Required['Freshness'], 'observedAt': Required['Timestamp'], 'source': Required['EvidenceSource'], 'state': Required[Literal['known']], 'value': Required[bool], 'verification': Required['Verification']}, total=False)

Observation6_Case1 = TypedDict('Observation6_Case1', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unknown']]}, total=False)

Observation6_Case2 = TypedDict('Observation6_Case2', {'checkedAt': NotRequired[Union['Timestamp', None]], 'reason': Required[str], 'state': Required[Literal['unavailable']]}, total=False)

Observation6: TypeAlias = Union[Observation6_Case0, Observation6_Case1, Observation6_Case2]

OptionControl_Case0 = TypedDict('OptionControl_Case0', {'choices': Required[List['Choice']], 'defaultValue': NotRequired[Union[str, None]], 'type': Required[Literal['select']]}, total=False)

OptionControl_Case1 = TypedDict('OptionControl_Case1', {'type': Required[Literal['boolean']]}, total=False)

OptionControl_Case2 = TypedDict('OptionControl_Case2', {'max': Required[float], 'min': Required[float], 'step': Required[float], 'type': Required[Literal['number']]}, total=False)

OptionControl_Case3 = TypedDict('OptionControl_Case3', {'maxLength': Required[int], 'type': Required[Literal['text']]}, total=False)

OptionControl_Case4 = TypedDict('OptionControl_Case4', {'nativeType': Required[str], 'type': Required[Literal['unknown']]}, total=False)

OptionControl: TypeAlias = Union[OptionControl_Case0, OptionControl_Case1, OptionControl_Case2, OptionControl_Case3, OptionControl_Case4]

OptionDescriptor = TypedDict('OptionDescriptor', {'billingSensitive': NotRequired[bool], 'category': NotRequired[Union[str, None]], 'control': Required['OptionControl'], 'currentValue': NotRequired[Any], 'id': Required[str], 'label': Required[str], 'permissionSensitive': NotRequired[bool], 'revision': Required[str], 'scope': Required['OptionScope'], 'source': NotRequired[Union['EvidenceSource', None]]}, total=False)

OptionScope: TypeAlias = Literal['model', 'session', 'turn']

Plan = TypedDict('Plan', {'label': Required[str], 'nativeId': Required[str]}, total=False)

Profile = TypedDict('Profile', {'id': Required[str], 'installationId': Required[str], 'label': Required[str]}, total=False)

Provider = TypedDict('Provider', {'id': Required[str], 'kind': Required[str], 'name': Required[str]}, total=False)

QuotaPool = TypedDict('QuotaPool', {'accountId': NotRequired[Union[str, None]], 'connectionIds': Required[List[str]], 'id': Required[str], 'routeIds': Required[List[str]], 'sharing': Required[str], 'windows': Required[List['UsageWindow']]}, total=False)

Remediation = TypedDict('Remediation', {'description': Required[str], 'kind': Required['RemediationKind'], 'label': Required[str], 'requiresUserAction': Required[bool]}, total=False)

RemediationKind: TypeAlias = Literal['refresh', 'sign_in', 'unlock_keyring', 'update_client', 'open_settings', 'retry_later', 'restart_service', 'free_storage', 'contact_support', 'none']

Requirements = TypedDict('Requirements', {'allowPaidFallback': NotRequired[bool], 'billingPreference': NotRequired[Union[str, None]], 'eligibleOnly': NotRequired[bool], 'query': NotRequired[Union[str, None]], 'requiredCapabilities': NotRequired[List[str]]}, total=False)

ResourceChange = TypedDict('ResourceChange', {'change': Required['ResourceChangeKind'], 'id': Required[str], 'reason': NotRequired[Union[str, None]], 'resource': Required[str]}, total=False)

ResourceChangeKind: TypeAlias = Literal['added', 'updated', 'removed']

RuntimeHealth = TypedDict('RuntimeHealth', {'executionAvailable': Required[bool], 'metadataAvailable': Required[bool], 'mode': Required[str], 'status': Required[str]}, total=False)

Selection = TypedDict('Selection', {'modelId': Required[str], 'options': NotRequired[Dict[str, Any]], 'revision': Required[str], 'routeId': Required[str]}, total=False)

SelectionParams = TypedDict('SelectionParams', {'allowPaid': NotRequired[bool], 'selection': Required['Selection']}, total=False)

Session = TypedDict('Session', {'connectionId': Required[str], 'createdAt': Required['Timestamp'], 'id': Required[str], 'nativeResumption': Required[bool], 'selection': Required['Selection'], 'state': Required['SessionState']}, total=False)

SessionCreateParams = TypedDict('SessionCreateParams', {'allowPaid': NotRequired[bool], 'hostMcpServers': NotRequired[List['HostMcpServer']], 'permissionMode': NotRequired[Union[str, None]], 'persistent': NotRequired[bool], 'reviewRequests': NotRequired[bool], 'selection': Required['Selection'], 'workspaceId': Required[str]}, total=False)

SessionOptionsParams = TypedDict('SessionOptionsParams', {'options': Required[Dict[str, Any]], 'sessionId': Required[str]}, total=False)

SessionParams = TypedDict('SessionParams', {'sessionId': Required[str]}, total=False)

SessionResumeParams = TypedDict('SessionResumeParams', {'hostMcpServers': NotRequired[List['HostMcpServer']], 'resumeToken': Required[str]}, total=False)

SessionState: TypeAlias = Literal['ready', 'running', 'awaiting_approval', 'completed', 'error', 'cancelled', 'closed']

SessionSuspendResult = TypedDict('SessionSuspendResult', {'resumeToken': Required[str]}, total=False)

SetPricesParams = TypedDict('SetPricesParams', {'prices': Required[List['ModelPrice']]}, total=False)

Snapshot = TypedDict('Snapshot', {'accounts': Required[List['Account']], 'connections': Required[List['Connection']], 'cursor': NotRequired[str], 'diagnostics': Required[List[str]], 'health': NotRequired['HealthSummary'], 'installations': Required[List['Installation']], 'issues': NotRequired[List['Issue']], 'mode': Required['DataMode'], 'models': Required[List['Model']], 'observedAt': Required['Timestamp'], 'profiles': Required[List['Profile']], 'protocolVersion': Required[str], 'providers': Required[List['Provider']], 'quotaPools': Required[List['QuotaPool']], 'revision': Required[str], 'routes': Required[List['AccessRoute']]}, total=False)

SnapshotParams = TypedDict('SnapshotParams', {'fields': NotRequired[Union[List[str], None]]}, total=False)

SubscriptionParams = TypedDict('SubscriptionParams', {'subscriptionId': Required[str]}, total=False)

Support: TypeAlias = Literal['supported', 'unsupported', 'conditional', 'unknown']

Timestamp: TypeAlias = str

TurnInputRef = TypedDict('TurnInputRef', {'inputId': Required[str]}, total=False)

TurnParams = TypedDict('TurnParams', {'turnId': Required[str]}, total=False)

TurnStartParams = TypedDict('TurnStartParams', {'inputs': NotRequired[List['TurnInputRef']], 'prompt': Required[str], 'requestId': Required[str], 'sessionId': Required[str]}, total=False)

TurnStarted = TypedDict('TurnStarted', {'turnId': Required[str]}, total=False)

UsageReading = TypedDict('UsageReading', {'limit': NotRequired[Union['DecimalString', None]], 'remaining': NotRequired[Union['DecimalString', None]], 'unit': Required[str], 'used': Required['DecimalString'], 'usedFraction': NotRequired[Union[float, None]]}, total=False)

UsageWindow = TypedDict('UsageWindow', {'durationSeconds': NotRequired[Union[int, None]], 'id': Required[str], 'label': Required[str], 'reading': Required['Observation5'], 'resetsAt': NotRequired[Union['Timestamp', None]]}, total=False)

ValidationResult = TypedDict('ValidationResult', {'mode': Required['DataMode'], 'valid': Required[bool]}, total=False)

Verification: TypeAlias = Literal['reported', 'observed', 'inferred']
