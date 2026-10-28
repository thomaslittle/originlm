use originlm_adapters::native_clients::config::*;
use originlm_contracts::*;
use originlm_discovery::ScanResult;
use originlm_discovery::{HostPlatform, ScanEnvironment, Scanner};
use originlm_platform::NativeFilesystem;
use serde_json::json;
use std::collections::BTreeMap;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn document() -> serde_json::Value {
    let root = std::env::temp_dir();
    json!({"profiles":[{"id":"google","label":"My Google CLI","provider":"gemini_cli","executable":root.join("gemini"),"profileHome":root.join("google-home")},{"id":"antigravity","label":"Google ACP","provider":"antigravity","executable":root.join("agy_acp_server.exe"),"profileHome":root.join("gemini/antigravity-acp"),"authMethod":"oauth-personal"},{"id":"grok","label":"Grok","provider":"grok","executable":root.join("grok.exe"),"profileHome":root.join(".grok"),"billing":"subscription"}]})
}
fn write_codex_host(dir: &std::path::Path) {
    let host = dir.join(if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    });
    std::fs::write(&host, b"host").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&host).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&host, permissions).unwrap();
    }
}
fn empty() -> ScanResult {
    ScanResult {
        snapshot: Snapshot::empty(DataMode::Live),
        locations: BTreeMap::new(),
        metadata_probes: 0,
    }
}

#[test]
fn managed_codex_is_inert_isolated_and_stable() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = tmp.path().join("codex.exe");
    std::fs::write(&codex, b"fixture").unwrap();
    write_codex_host(tmp.path());
    let state = tmp.path().join("kit-state");
    let mut first = empty();
    first.locations.insert(
        "native-codex".into(),
        originlm_discovery::LocatedClient {
            executable_identity: None,
            provider_id: "codex".into(),
            profile_home: tmp.path().join("personal-codex"),
            executable: Some(codex.clone()),
        },
    );
    inject_managed_codex(&mut first, &state);
    let managed = first
        .snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "codex_managed")
        .unwrap();
    let located = &first.locations[&managed.id];
    assert_eq!(located.profile_home, state.join("chatgpt-codex-home"));
    assert_eq!(located.executable.as_ref(), Some(&codex));
    assert!(first.snapshot.accounts.is_empty());
    assert!(managed.auth.value().is_none());
    assert!(managed.plan.value().is_none());
    assert!(
        !state.exists(),
        "passive inventory must not open managed auth storage"
    );
    let projection = serde_json::to_string(&first.snapshot).unwrap();
    assert!(!projection.contains("ACCESS_TOKEN"));
    assert!(!projection.contains("refresh_token"));
    let mut second = empty();
    second.locations.insert(
        "native-codex".into(),
        first.locations["native-codex"].clone(),
    );
    inject_managed_codex(&mut second, &state);
    assert_eq!(
        managed.id,
        second
            .snapshot
            .connections
            .iter()
            .find(|c| c.provider_id == "codex_managed")
            .unwrap()
            .id
    );
}
#[test]
fn managed_codex_can_use_private_executable_only_inventory() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let executable = bin.join("codex.exe");
    std::fs::write(&executable, b"fixture").unwrap();
    write_codex_host(&bin);
    std::fs::create_dir(tmp.path().join(".codex")).unwrap();
    let mut env = ScanEnvironment::for_home(tmp.path().into(), HostPlatform::Windows);
    env.search_path.push(bin);
    let mut scan = Scanner::new(NativeFilesystem)
        .with_executable_only_providers(BTreeSet::from(["codex".into()]))
        .scan(&env, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert!(scan.snapshot.connections.is_empty());
    inject_managed_codex(&mut scan, &tmp.path().join("managed-state"));
    assert_eq!(scan.snapshot.connections.len(), 1);
    assert_eq!(scan.snapshot.connections[0].provider_id, "codex_managed");
    assert_eq!(
        scan.locations[&scan.snapshot.connections[0].id]
            .executable
            .as_ref(),
        Some(&executable)
    );
    assert!(!tmp.path().join("managed-state").exists());
}
#[test]
fn native_registration_is_inert_and_retains_distinct_google_clients() {
    let r = NativeRegistry::parse(&document().to_string(), "source").unwrap();
    let mut scan = empty();
    r.inject(&mut scan);
    assert_eq!(scan.snapshot.connections.len(), 3);
    assert!(scan.snapshot.accounts.is_empty());
    assert!(scan.snapshot.models.is_empty());
    assert!(
        scan.snapshot
            .connections
            .iter()
            .all(|c| c.auth.value().is_none() && c.plan.value().is_none())
    );
    assert_eq!(
        scan.snapshot
            .connections
            .iter()
            .filter(|c| c.provider_id == "gemini_cli")
            .count(),
        1
    );
    assert_eq!(
        scan.snapshot
            .connections
            .iter()
            .filter(|c| c.provider_id == "antigravity")
            .count(),
        1
    );
}
#[test]
fn credentials_arbitrary_args_and_remote_paths_are_not_configuration_fields() {
    for field in ["token", "apiKey", "args", "environment", "command"] {
        let mut d = document();
        d["profiles"][0][field] = json!("SECRET_SENTINEL");
        let e = NativeRegistry::parse(&d.to_string(), "source")
            .err()
            .expect("unknown fields denied");
        assert!(!e.to_string().contains("SECRET_SENTINEL"));
    }
    let mut d = document();
    d["profiles"][0]["executable"] = json!("relative");
    assert!(NativeRegistry::parse(&d.to_string(), "s").is_err());
}
#[test]
fn another_connection_cannot_claim_a_configured_profile() {
    let r = NativeRegistry::parse(&document().to_string(), "source").unwrap();
    let mut s = empty();
    r.inject(&mut s);
    let (c, l) = s
        .locations
        .iter()
        .find(|(_, l)| l.provider_id == "gemini_cli")
        .unwrap();
    let mut foreign = l.clone();
    foreign.provider_id = "grok".into();
    assert!(r.profile(c, &foreign).is_err());
}
#[test]
fn stable_ids_include_native_profile_scope_and_executable() {
    let d = document();
    let a = NativeRegistry::parse(&d.to_string(), "personal").unwrap();
    let b = NativeRegistry::parse(&d.to_string(), "work").unwrap();
    assert!(a.profiles.keys().all(|id| !b.profiles.contains_key(id)));
}
#[test]
fn native_billing_declaration_is_not_published_as_verified_access_during_inventory() {
    let r = NativeRegistry::parse(&document().to_string(), "s").unwrap();
    let mut s = empty();
    r.inject(&mut s);
    assert!(
        s.snapshot
            .routes
            .iter()
            .all(|r| r.billing.value().is_none())
    );
}
#[test]
fn malformed_profiles_and_unsupported_auth_switches_are_rejected() {
    let mut d = document();
    d["profiles"][0]["authMethod"] = json!("oauth-personal");
    assert!(NativeRegistry::parse(&d.to_string(), "s").is_err());
    let mut d = document();
    d["profiles"][1]["authMethod"] = json!("bad\nmethod");
    assert!(NativeRegistry::parse(&d.to_string(), "s").is_err());
    let mut d = document();
    d["profiles"][2]["billing"] = json!("local");
    assert!(NativeRegistry::parse(&d.to_string(), "s").is_err());
}
