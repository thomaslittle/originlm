//! Live probe against a running OpenCode 2 background service.
//! Run with: cargo test -p originlm-adapters --test opencode_v2_live_probe -- --ignored
use originlm_adapters::opencode::v2_service;

#[tokio::test]
#[ignore = "requires a running OpenCode 2 background service"]
async fn probes_the_local_service() {
    let metadata = v2_service::probe("connection_test", "route_test", "2.2.0")
        .await
        .expect("running service");
    assert!(!metadata.models.is_empty());
    assert!(matches!(
        metadata.auth,
        originlm_contracts::Observation::Known {
            value: originlm_contracts::AuthState::Authenticated,
            ..
        }
    ));
    assert!(!metadata.quota_pools.is_empty());
    println!(
        "models={} providers={} pools={} version={:?}",
        metadata.models.len(),
        metadata.providers.len(),
        metadata.quota_pools.len(),
        metadata.version
    );
    for pool in &metadata.quota_pools {
        for window in &pool.windows {
            println!(
                "{} · {} · {:?}",
                window.label,
                match &window.reading {
                    originlm_contracts::Observation::Known { value, .. } =>
                        format!("used {} {}", value.used.as_str(), value.unit),
                    _ => "unknown".into(),
                },
                window.resets_at.as_ref().map(|t| t.as_str())
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires a running OpenCode 2 background service"]
async fn probes_through_the_installed_path() {
    let home = std::env::var("HOME").unwrap();
    let path = std::path::PathBuf::from(home).join(".opencode/bin/opencode");
    match originlm_adapters::opencode::server::probe(&path, "connection_test", "route_test").await {
        Ok(metadata) => println!(
            "ok models={} pools={}",
            metadata.models.len(),
            metadata.quota_pools.len()
        ),
        Err(error) => panic!("probe failed: {:?} {}", error.code, error.message),
    }
}
