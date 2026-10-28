use originlm_contracts::*;
pub fn fixture() -> Snapshot {
    let mut s = Snapshot::empty(DataMode::Synthetic);
    let evidence = EvidenceSource::new(EvidenceKind::Synthetic, "synthetic", "core test");
    s.providers.push(Provider {
        id: "test".into(),
        name: "Test".into(),
        kind: "synthetic".into(),
    });
    for label in ["personal", "work"] {
        s.installations.push(Installation {
            id: format!("install-{label}"),
            provider_id: "test".into(),
            environment_id: "synthetic".into(),
            location_hint: "synthetic".into(),
            version: Observation::unknown("NOT_PROBED"),
        });
        s.profiles.push(Profile {
            id: format!("profile-{label}"),
            installation_id: format!("install-{label}"),
            label: label.into(),
        });
        s.accounts.push(Account {
            id: format!("account-{label}"),
            provider_id: "test".into(),
            display_label: Observation::reported(format!("{label}@example.test"), evidence.clone()),
            identity_source: Observation::reported(format!("private-{label}"), evidence.clone()),
            scopes: vec![],
        });
        s.connections.push(Connection {
            id: label.into(),
            provider_id: "test".into(),
            installation_id: format!("install-{label}"),
            profile_id: format!("profile-{label}"),
            display_name: label.into(),
            account_id: Some(format!("account-{label}")),
            auth: Observation::reported(AuthState::Authenticated, evidence.clone()),
            plan: Observation::unknown("NOT_REPORTED"),
            status: "ok".into(),
            diagnostics: vec![],
        });
        s.routes.push(AccessRoute {
            id: format!("route-{label}"),
            connection_id: label.into(),
            provider_id: "test".into(),
            display_name: label.into(),
            billing: Observation::reported(BillingMode::Subscription, evidence.clone()),
            capabilities: vec![],
            catalog_revision: "1".into(),
        });
        s.models.push(Model {
            id: format!("model-{label}"),
            route_id: format!("route-{label}"),
            native_id: "same-native-model".into(),
            name: "Same native model".into(),
            revision: "1".into(),
            is_default: true,
            capabilities: vec![Capability {
                id: "generation.text".into(),
                support: Support::Supported,
                origin: CapabilityOrigin::Model,
                evidence: Observation::reported("reported".into(), evidence.clone()),
                conditions: vec![],
            }],
            options: vec![],
            context_window: Observation::unknown("NOT_REPORTED"),
        });
    }
    s.quota_pools.push(QuotaPool {
        id: "shared".into(),
        account_id: None,
        connection_ids: vec!["personal".into(), "work".into()],
        route_ids: vec!["route-personal".into(), "route-work".into()],
        sharing: "verified".into(),
        windows: vec![],
    });
    s
}
pub fn selection() -> Selection {
    Selection {
        route_id: "route-personal".into(),
        model_id: "model-personal".into(),
        revision: "1".into(),
        options: Default::default(),
    }
}
