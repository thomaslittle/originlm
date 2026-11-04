use originlm_discovery::{HostPlatform, ScanEnvironment, Scanner};
use originlm_platform::NativeFilesystem;
use originlm_service::InventorySource;
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
pub fn source(
    home: Option<PathBuf>,
    executable_only_providers: BTreeSet<String>,
) -> InventorySource {
    Arc::new(move |cancel| scan(&home, executable_only_providers.clone(), cancel))
}

/// One passive scan. Used by the CLI's `scan` action so the opt-in location
/// detail comes from the same `ScanResult` the service publishes.
pub fn scan(
    home: &Option<PathBuf>,
    executable_only_providers: BTreeSet<String>,
    cancel: &std::sync::atomic::AtomicBool,
) -> originlm_contracts::Result<originlm_discovery::ScanResult> {
    let environment = if let Some(home) = home {
        let platform = if cfg!(windows) {
            HostPlatform::Windows
        } else if cfg!(target_os = "macos") {
            HostPlatform::Macos
        } else {
            HostPlatform::Linux
        };
        ScanEnvironment::for_home(home.clone(), platform)
    } else {
        ScanEnvironment::from_process()?
    };
    Scanner::new(NativeFilesystem)
        .with_executable_only_providers(executable_only_providers)
        .scan(&environment, cancel, |_| {})
}
