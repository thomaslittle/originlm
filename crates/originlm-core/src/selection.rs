use crate::{Candidate, Requirements};
use originlm_contracts::*;
use std::collections::BTreeMap;
fn error(code: ErrorCode, message: &str) -> DomainError {
    DomainError::new(code, message)
}
fn route<'a>(snapshot: &'a Snapshot, model: &Model) -> Result<(&'a AccessRoute, &'a Connection)> {
    let route = snapshot
        .routes
        .iter()
        .find(|r| r.id == model.route_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Model route is unavailable"))?;
    let connection = snapshot
        .connections
        .iter()
        .find(|c| c.id == route.connection_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Connection is unavailable"))?;
    Ok((route, connection))
}
/// Accountless access is specific to supported local runtimes, not a billing bypass.
/// Callers must separately validate freshness, grants, and route capabilities.
pub fn is_accountless_local_route(route: &AccessRoute, connection: &Connection) -> bool {
    matches!(connection.provider_id.as_str(), "ollama" | "lmstudio")
        && route.provider_id == connection.provider_id
        && route.connection_id == connection.id
        && route.billing.value() == Some(&BillingMode::Local)
        && matches!(&connection.auth, Observation::Unknown { reason, .. } if reason == "LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH")
}
fn access(route: &AccessRoute, connection: &Connection, paid: bool) -> Result<()> {
    // Like native hosts (T3 Code), the last observed sign-in stands until the
    // client reports otherwise: stale "authenticated" evidence is accepted and
    // the client itself refuses if the login lapsed. Signed-out, expired or
    // never-checked connections still block.
    if connection.auth.value() != Some(&AuthState::Authenticated)
        && !is_accountless_local_route(route, connection)
    {
        return Err(error(
            ErrorCode::AuthRequired,
            "Current authentication is not verified",
        ));
    }
    if route.billing.value() == Some(&BillingMode::Subscription)
        && route
            .capabilities
            .iter()
            .any(|c| c.id == "usage.included_allowed" && c.support == Support::Unsupported)
    {
        return Err(error(
            ErrorCode::RateLimited,
            "Provider denied included usage; refresh allowance evidence before retrying",
        ));
    }
    match route.billing.fresh_value() {
        Some(BillingMode::Subscription | BillingMode::Local) => Ok(()),
        Some(BillingMode::ApiMetered | BillingMode::PrepaidCredits | BillingMode::Mixed)
            if paid =>
        {
            Ok(())
        }
        Some(BillingMode::ApiMetered | BillingMode::PrepaidCredits | BillingMode::Mixed) => {
            Err(error(
                ErrorCode::BillingConsentRequired,
                "This route needs separate billing authorization",
            ))
        }
        _ => Err(error(
            ErrorCode::BillingConsentRequired,
            "Billing is unknown; explicit verification is required",
        )),
    }
}
fn capability(model: &Model, route: &AccessRoute, id: &str) -> Result<()> {
    let found: Vec<_> = model
        .capabilities
        .iter()
        .chain(&route.capabilities)
        .filter(|c| c.id == id)
        .collect();
    if found.is_empty() {
        return Err(error(
            ErrorCode::CapabilityUnknown,
            "Required capability was not reported",
        ));
    }
    for cap in found {
        if cap.support == Support::Unsupported {
            return Err(error(
                ErrorCode::CapabilityUnsupported,
                "Required capability is unsupported",
            ));
        }
        if cap.support != Support::Supported
            || cap.evidence.fresh_value().is_none()
            || !cap.conditions.is_empty()
        {
            return Err(error(
                ErrorCode::CapabilityUnknown,
                "Required capability is not unconditionally verified",
            ));
        }
    }
    Ok(())
}
fn code_string(code: &ErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "INVALID_REQUEST".into())
}
pub fn resolve(snapshot: &Snapshot, requirements: &Requirements) -> Vec<Candidate> {
    let query = requirements
        .query
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(2048)
        .collect::<String>()
        .to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    let mut candidates = Vec::new();
    for model in &snapshot.models {
        let Ok((route, connection)) = route(snapshot, model) else {
            continue;
        };
        let search = format!(
            "{} {} {} {} {}",
            model.name,
            model.native_id,
            route.provider_id,
            route.display_name,
            connection.display_name
        )
        .to_lowercase();
        if !terms.iter().all(|term| search.contains(term)) {
            continue;
        }
        let mut reasons = Vec::new();
        if let Err(e) = access(route, connection, requirements.allow_paid_fallback) {
            reasons.push(code_string(&e.code));
        }
        if requirements.required_capabilities.len() > 64 {
            reasons.push("INVALID_REQUEST".into());
        }
        for required in requirements.required_capabilities.iter().take(64) {
            if let Err(e) = capability(model, route, required) {
                reasons.push(format!("{}:{required}", code_string(&e.code)));
            }
        }
        let eligible = reasons.is_empty();
        if requirements.eligible_only && !eligible {
            continue;
        }
        candidates.push(Candidate {
            selection: Selection {
                route_id: route.id.clone(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: BTreeMap::new(),
            },
            connection_id: connection.id.clone(),
            eligible,
            reasons,
        });
    }
    if requirements.billing_preference.as_deref() == Some("prefer-included") {
        candidates.sort_by_key(|c| {
            let included = snapshot
                .routes
                .iter()
                .find(|r| r.id == c.selection.route_id)
                .is_some_and(|r| {
                    matches!(
                        r.billing.fresh_value(),
                        Some(BillingMode::Subscription | BillingMode::Local)
                    )
                });
            (!c.eligible, !included)
        });
    }
    candidates
}
pub fn validate_selection<'a>(
    snapshot: &'a Snapshot,
    selection: &Selection,
    paid: bool,
) -> Result<&'a Model> {
    let model = snapshot
        .models
        .iter()
        .find(|m| m.id == selection.model_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Selected model is no longer present"))?;
    if selection.route_id != model.route_id {
        return Err(error(
            ErrorCode::SessionRouteMismatch,
            "Selected route and model belong to different connections",
        ));
    }
    if selection.revision != model.revision {
        return Err(error(
            ErrorCode::SelectionStale,
            "Model configuration changed; review the selection",
        ));
    }
    let (route, connection) = route(snapshot, model)?;
    access(route, connection, paid)?;
    capability(model, route, "generation.text")?;
    for (id, value) in &selection.options {
        let descriptor = model.options.iter().find(|d| d.id == *id).ok_or_else(|| {
            error(
                ErrorCode::OptionUnsupported,
                "Option is not advertised for this model",
            )
        })?;
        descriptor.validate(value).map_err(|_| {
            error(
                ErrorCode::OptionUnsupported,
                "Option value is not supported by the selected model",
            )
        })?;
        if descriptor.billing_sensitive && !paid {
            return Err(error(
                ErrorCode::BillingConsentRequired,
                "This option needs separate billing authorization",
            ));
        }
        if descriptor.permission_sensitive {
            return Err(error(
                ErrorCode::ApprovalRequired,
                "Permission-sensitive options require a supported approval path",
            ));
        }
    }
    Ok(model)
}
