use crate::args::{Action, Args};
use originlm_core::ClientPolicy;
pub use originlm_kit::default_state_dir;
use originlm_service::ExecutionGrant;
use std::collections::BTreeMap;
pub fn validate_executable_only_sources(
    args: &Args,
    registry: &originlm_adapters::http_provider::config::Registry,
    native: &originlm_adapters::native_clients::config::NativeRegistry,
) -> originlm_contracts::Result<()> {
    if registry
        .entries
        .values()
        .any(|entry| args.executable_only_providers.contains(entry.kind.id()))
        || native.profiles.values().any(|profile| {
            args.executable_only_providers
                .contains(profile.provider.id())
        })
    {
        return Err(originlm_contracts::DomainError::new(
            originlm_contracts::ErrorCode::InvalidRequest,
            "Executable-only providers cannot also have explicit provider registrations",
        ));
    }
    Ok(())
}
pub fn policy(args: &Args) -> ClientPolicy {
    let mut policy = if args.allow_passive || matches!(args.action, Action::Scan) {
        ClientPolicy::passive()
    } else {
        ClientPolicy::denied()
    };
    policy.probe_provider_ids = args.metadata_providers.clone();
    policy.reveal_identity = args.allow_identity;
    policy
}
pub fn execution_grant(args: &Args) -> Option<ExecutionGrant> {
    if !args.allow_execution {
        return None;
    }
    Some(ExecutionGrant {
        provider_ids: args.execution_providers.clone(),
        workspaces: BTreeMap::from([("default".into(), args.workspace.clone()?)]),
        allow_paid: args.allow_paid,
        allow_writes: args.allow_writes,
        review_native: args.review_requests,
        allow_native_policy: args.allow_native_policy,
        ..Default::default()
    })
}
