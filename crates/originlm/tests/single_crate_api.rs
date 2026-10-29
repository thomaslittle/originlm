//! Single-crate consumer fixture. This test imports only `originlm` plus std;
//! it proves synthetic inventory and scoped native configuration are usable
//! without depending on another OriginLM crate.

use originlm::{
    ClientPolicy, DataMode, InventorySource, LocalAi, Mode, NativeClientRegistry, NativeProfile,
    NativeProvider, ScanResult, Snapshot, StartOptions,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[test]
fn a_consumer_with_only_the_facade_can_build_synthetic_scope() {
    let inventory: InventorySource = Arc::new(|_cancel| {
        Ok(ScanResult {
            snapshot: Snapshot::empty(DataMode::Synthetic),
            locations: BTreeMap::new(),
            metadata_probes: 0,
        })
    });
    let mut native = NativeClientRegistry::default();
    native.profiles.insert(
        "fixture".into(),
        NativeProfile {
            id: "fixture".into(),
            label: "Fixture".into(),
            provider: NativeProvider::GeminiCli,
            executable: PathBuf::from("/fixture/bin/gemini"),
            profile_home: PathBuf::from("/fixture/home"),
            auth_method: None,
            billing: None,
        },
    );
    let mut ai = LocalAi::start_with(StartOptions {
        required_mode: Some(Mode::InProcess),
        inventory: Some(inventory),
        native_clients: native,
        policy: ClientPolicy::passive(),
        ..StartOptions::default()
    })
    .expect("the public facade must accept a synthetic scoped runtime");
    assert_eq!(ai.mode(), Mode::InProcess);
    let snapshot = ai.scan().expect("synthetic scan");
    assert_eq!(snapshot.mode, DataMode::Synthetic);
    assert!(
        snapshot
            .connections
            .iter()
            .any(|connection| connection.id == "fixture"),
        "host-declared native scope must reach the snapshot"
    );
    ai.close();
}
