use originlm_discovery::{HostPlatform, ScanEnvironment, Scanner};
use originlm_platform::NativeFilesystem;
use originlm_service::InventorySource;
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
pub fn source(
    home: Option<PathBuf>,
    executable_only_providers: BTreeSet<String>,
) -> InventorySource {
    Arc::new(move |cancel| {
        let environment = if let Some(home) = &home {
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
            .with_executable_only_providers(executable_only_providers.clone())
            .scan(&environment, cancel, |_| {})
    })
}
