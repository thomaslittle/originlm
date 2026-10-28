use super::{evidence, wire::Wire};
use crate::process::LaunchSpec;
use originlm_contracts::*;
/// Facts published by provider-specific status commands, never inferred from installation.
pub struct AccountEvidence {
    pub auth: Observation<AuthState>,
    pub billing: Observation<BillingMode>,
    pub plan: Observation<Plan>,
}
impl Default for AccountEvidence {
    fn default() -> Self {
        Self {
            auth: Observation::unknown("ACP_HAS_NO_STANDARD_ACCOUNT_STATUS"),
            billing: Observation::unknown("BILLING_NOT_REPORTED"),
            plan: Observation::unknown("PLAN_NOT_REPORTED"),
        }
    }
}
pub async fn probe(
    spec: LaunchSpec,
    provider: &str,
    connection: &str,
    route: &str,
    access: AccountEvidence,
) -> Result<ConnectionMetadata> {
    probe_with_auth(spec, provider, connection, route, access, None).await
}
pub async fn probe_with_auth(
    spec: LaunchSpec,
    provider: &str,
    connection: &str,
    route: &str,
    access: AccountEvidence,
    auth: Option<&str>,
) -> Result<ConnectionMetadata> {
    let mut wire = Wire::connect_with_auth(spec, provider.into(), route.into(), auth, &[]).await?;
    let result = (|| {
        let catalog = wire.catalog()?;
        let version = wire.init["agentInfo"]["version"]
            .as_str()
            .or_else(|| {
                if provider == "grok" {
                    wire.init["_meta"]["agentVersion"].as_str()
                } else {
                    None
                }
            })
            .filter(|s| s.len() <= 256 && !s.chars().any(char::is_control));
        Ok(ConnectionMetadata {
            connection_id: connection.into(),
            route_id: route.into(),
            version: version
                .map(|s| Observation::reported(s.to_owned(), evidence(provider)))
                .unwrap_or_else(|| Observation::unknown("VERSION_NOT_REPORTED")),
            auth: access.auth,
            billing: access.billing,
            plan: access.plan,
            account: None,
            models: catalog.models,
            additional_routes: vec![],
            providers: vec![],
            quota_pools: vec![],
            included_usage_allowed: Observation::unknown("QUOTA_NOT_REPORTED_BY_ACP"),
            diagnostics: vec![
                "NATIVE_SESSION_METADATA_NO_INFERENCE".into(),
                "ACCOUNT_QUOTA_NOT_REPORTED_BY_ACP".into(),
            ],
        })
    })();
    wire.close().await;
    result
}
