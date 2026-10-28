use crate::{NativeService, denied, params, unsupported};
use originlm_contracts::{DomainError, ErrorCode, Result, Selection};
use originlm_core::{ClientPolicy, Requirements, resolve, validate_selection};
use serde_json::{Value, json};
const METHODS: &[&str] = &[
    "runtime.handshake",
    "runtime.health",
    "runtime.closeClient",
    "snapshot.get",
    "connections.list",
    "models.list",
    "models.page",
    "usage.read",
    "costs.read",
    "costs.setPrices",
    "capabilities.read",
    "adapters.list",
    "diagnostics.report",
    "discovery.refresh",
    "discovery.watch",
    "selection.resolve",
    "selection.validate",
    "grants.request",
    "grants.list",
    "grants.revoke",
    "health.get",
    "issues.list",
    "issues.acknowledge",
    "issues.snooze",
    "notifications.poll",
    "notifications.configure",
    "notifications.ack",
    "apps.list",
    "apps.revoke",
];
impl NativeService {
    fn advertised_methods(&self) -> Vec<&'static str> {
        let mut methods = METHODS.to_vec();
        methods.extend(self.execution_methods());
        if self.metadata_source.is_some() {
            methods.push("connections.probe");
        }
        if self.chatgpt_auth.is_some() {
            methods.extend([
                "auth.chatgpt.status",
                "auth.chatgpt.start",
                "auth.chatgpt.cancel",
                "auth.chatgpt.disconnect",
            ]);
        }
        methods
    }
    fn advertised_capabilities(&self) -> Vec<&'static str> {
        let mut capabilities = self.execution_capabilities();
        if self.chatgpt_auth.is_some() {
            capabilities.push("auth.chatgpt");
        }
        capabilities
    }
    pub(crate) fn dispatch(&self, client: &str, method: &str, p: &Value) -> Result<Value> {
        match method {
            "runtime.handshake" => {
                params::allow(p, &["protocolVersion", "clientName"])?;
                let version = params::text(p, "protocolVersion")?.unwrap_or("1.0");
                if version.split_once('.').is_none_or(|(major, minor)| {
                    major != "1" || minor.is_empty() || !minor.bytes().all(|b| b.is_ascii_digit())
                }) {
                    return Err(DomainError::new(
                        ErrorCode::ProtocolIncompatible,
                        "Protocol major version is not supported",
                    ));
                }
                params::text(p, "clientName")?;
                Ok(
                    json!({"protocolVersion":"1.0","runtimeVersion":env!("CARGO_PKG_VERSION"),"methods":self.advertised_methods(),"capabilities":self.advertised_capabilities()}),
                )
            }
            "costs.read" => {
                params::allow(p, &[])?;
                Ok(json!(self.cost_report(client)?))
            }
            "costs.setPrices" => {
                let input: originlm_contracts::SetPricesParams = params::decode(p.clone())?;
                Ok(json!(self.set_cost_prices(client, input.prices)?))
            }
            "runtime.health" => {
                params::allow(p, &[])?;
                let (revision, cursor, hydrated, storage) = self.cache.status();
                let autonomous = self
                    .refresh_providers
                    .read()
                    .map(|providers| !providers.is_empty())
                    .unwrap_or(false)
                    || self
                        .inventory_refresh
                        .load(std::sync::atomic::Ordering::SeqCst);
                Ok(
                    json!({"status": if storage.is_some() { "degraded" } else { "ok" },"mode":"live","executionAvailable":self.execution_state.source.is_some(),"metadataAvailable":self.metadata_source.is_some(),"revision":revision,"cursor":cursor,"hydrated":hydrated,"storageAvailable":storage.is_none(),"autonomousRefresh":autonomous}),
                )
            }
            "grants.request" => {
                params::allow(p, &["scopes", "connectionIds", "allowPaid"])?;
                // This is an approval request, never a permission mutation.
                Ok(json!({"status":"requires_host_approval"}))
            }
            "grants.list" => {
                params::allow(p, &[])?;
                let policy = self.policy(client)?;
                Ok(
                    json!({"allowPassiveDiscovery":policy.allow_passive_discovery,"connectionIds":policy.connection_ids,"revealIdentity":policy.reveal_identity,"execution":self.execution_state.grants.read().map_err(|_|denied())?.contains_key(client)}),
                )
            }
            "grants.revoke" => {
                params::allow(p, &[])?;
                let mut clients = self.clients.write().map_err(|_| denied())?;
                *clients.get_mut(client).ok_or_else(denied)? = ClientPolicy::denied();
                Ok(json!({"revoked":true}))
            }
            "apps.list" => {
                params::allow(p, &[])?;
                if !self.policy(client)?.admin {
                    return Err(denied());
                }
                let clients = self.clients.read().map_err(|_| denied())?;
                let identities = self.app_identities.read().map_err(|_| denied())?;
                let apps: Vec<Value> = clients
                    .iter()
                    .map(|(id, policy)| {
                        json!({
                            "id": id,
                            "app": identities.get(id).cloned().unwrap_or_else(|| id.clone()),
                            "allowPassiveDiscovery": policy.allow_passive_discovery,
                            "connectionIds": policy.connection_ids,
                            "revealIdentity": policy.reveal_identity,
                            "admin": policy.admin,
                        })
                    })
                    .collect();
                Ok(json!(apps))
            }
            "apps.revoke" => {
                params::allow(p, &["targetClientId"])?;
                if !self.policy(client)?.admin {
                    return Err(denied());
                }
                let target = params::text(p, "targetClientId")?.ok_or_else(params::invalid)?;
                if target == client {
                    return Err(DomainError::new(
                        ErrorCode::Conflict,
                        "Administrative client cannot revoke itself through this method",
                    ));
                }
                if !self
                    .clients
                    .read()
                    .map_err(|_| denied())?
                    .contains_key(target)
                {
                    return Err(DomainError::new(
                        ErrorCode::NotFound,
                        "Application is unavailable",
                    ));
                }
                // Revocation is live and application-scoped: the broker grant is
                // removed first, so the credential cannot reconnect, and every
                // live connection with the same application identity is swept.
                // The legacy owner credential keeps per-connection semantics.
                let app = self.app_identity(target);
                if app == "ipc-host" {
                    self.unregister_client(target);
                    Ok(json!({"revoked":true,"clientId":target,"app":app,"grantsRevoked":false}))
                } else {
                    let grants_revoked = self.revoke_app_grant(&app);
                    let connections = self.unregister_app(&app);
                    Ok(
                        json!({"revoked":true,"clientId":target,"app":app,"connections":connections,"grantsRevoked":grants_revoked}),
                    )
                }
            }
            "adapters.list" => {
                params::allow(p, &[])?;
                Ok(json!(self.adapters))
            }
            _ => self.read_data(client, method, p),
        }
    }
    fn read_data(&self, client: &str, method: &str, p: &Value) -> Result<Value> {
        let snapshot = self.projected(client)?;
        match method {
            "snapshot.get" => {
                params::allow(p, &["fields"])?;
                let data = serde_json::to_value(snapshot).map_err(|_| params::invalid())?;
                if let Some(fields) = p.get("fields") {
                    let fields = fields
                        .as_array()
                        .filter(|fields| fields.len() <= 32)
                        .ok_or_else(params::invalid)?;
                    let mut selected = serde_json::Map::new();
                    for field in fields {
                        let name = field.as_str().ok_or_else(params::invalid)?;
                        selected.insert(
                            name.to_owned(),
                            data.get(name).ok_or_else(params::invalid)?.clone(),
                        );
                    }
                    Ok(Value::Object(selected))
                } else {
                    Ok(data)
                }
            }
            "connections.list" => {
                params::allow(p, &[])?;
                Ok(json!(snapshot.connections))
            }
            "models.page" => crate::catalog::page(snapshot, client, p),
            "models.list" => {
                params::allow(p, &["routeId"])?;
                let route = params::text(p, "routeId")?;
                Ok(json!(
                    snapshot
                        .models
                        .into_iter()
                        .filter(|m| route.is_none_or(|id| id == m.route_id))
                        .collect::<Vec<_>>()
                ))
            }
            "usage.read" => {
                params::allow(p, &[])?;
                Ok(json!(snapshot.quota_pools))
            }
            "capabilities.read" => {
                params::allow(p, &["modelId"])?;
                let id = params::text(p, "modelId")?.ok_or_else(params::invalid)?;
                let model =
                    snapshot.models.iter().find(|m| m.id == id).ok_or_else(|| {
                        DomainError::new(ErrorCode::NotFound, "Model is unavailable")
                    })?;
                Ok(json!(model.capabilities))
            }
            "selection.resolve" => {
                let requirements: Requirements = params::decode(p.clone())?;
                Ok(json!(resolve(&snapshot, &requirements)))
            }
            "selection.validate" => {
                params::allow(p, &["selection", "allowPaid"])?;
                let selection: Selection =
                    params::decode(p.get("selection").ok_or_else(params::invalid)?.clone())?;
                validate_selection(&snapshot, &selection, params::boolean(p, "allowPaid")?)?;
                Ok(json!({"valid":true,"mode":snapshot.mode}))
            }
            "diagnostics.report" => {
                params::allow(p, &[])?;
                Ok(
                    json!({"mode":snapshot.mode,"connections":snapshot.connections.len(),"models":snapshot.models.len(),"quotaPools":snapshot.quota_pools.len(),"executionAvailable":self.execution_state.source.is_some(),"metadataAvailable":self.metadata_source.is_some(),"limitations":["Passive discovery alone does not verify authentication or plan access.","Adapter implementation is not a live account qualification."]}),
                )
            }
            _ => Err(unsupported()),
        }
    }
}
