//! Installed native host. Registration configuration is owned by the installing application.
mod args;
mod endpoints;
mod host;
mod inventory;
mod native_profiles;
use originlm_contracts::*;
use serde::Deserialize;
use std::{io::Read, time::Duration};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    allowed_origins: Vec<String>,
    #[serde(default)]
    allowed_extensions: Vec<String>,
    args: Vec<String>,
}
fn denied() -> DomainError {
    DomainError::new(
        ErrorCode::LocalAccessDenied,
        "Native host registration is unavailable or does not allow this extension",
    )
}
fn run() -> Result<()> {
    let mut input = std::env::args().skip(1);
    let origin = input.next().ok_or_else(denied)?;
    if !originlm_transports::native_messaging::valid_caller(&origin)
        || input.any(|v| {
            !v.strip_prefix("--parent-window=")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
    {
        return Err(denied());
    }
    let config = std::env::current_exe()
        .map_err(|_| denied())?
        .with_extension("json");
    let f = std::fs::File::open(config).map_err(|_| denied())?;
    let mut bytes = vec![];
    f.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| denied())?;
    if bytes.len() > 65536 {
        return Err(denied());
    }
    let config: Registration = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    if !(config.allowed_origins.iter().any(|o| o == &origin)
        || config.allowed_extensions.iter().any(|e| e == &origin))
        || config.allowed_origins.len() + config.allowed_extensions.len() > 32
        || config.args.len() > 128
    {
        return Err(denied());
    }
    let args = args::Args::parse(
        ["serve".to_owned(), "--stdio".into()]
            .into_iter()
            .chain(config.args)
            .map(std::ffi::OsString::from),
    )?;
    if args.http || args.ipc.is_some() {
        return Err(denied());
    }
    let registry = std::sync::Arc::new(endpoints::load(args.endpoints.as_deref())?);
    let state_dir = host::default_state_dir(args.home.as_deref());
    let chatgpt = args
        .chatgpt_app_name
        .clone()
        .map(|name| {
            let root = state_dir.as_ref().ok_or_else(denied)?;
            let manager =
                originlm_chatgpt_auth::AuthManager::new(root.join("chatgpt-auth"), name.clone())
                    .map_err(|_| denied())?;
            Ok::<_, DomainError>((std::sync::Arc::new(manager), name))
        })
        .transpose()?;
    let mut native = native_profiles::load(args.native_profiles.as_deref())?;
    host::validate_executable_only_sources(&args, &registry, &native)?;
    native.browser_blocker = native_profiles::packaged_browser_blocker();
    let native = std::sync::Arc::new(native);
    let declarations = std::sync::Arc::new(native_profiles::load_declarations(
        args.billing_declarations.as_deref(),
    )?);
    let base = inventory::source(args.home.clone(), args.executable_only_providers.clone());
    let entries = registry.clone();
    let native_entries = native.clone();
    let managed_root = chatgpt.as_ref().and(state_dir.as_ref()).cloned();
    let service = originlm_kit::build_service_with_cursor_usage(
        std::sync::Arc::new(move |cancel| {
            let mut scan = base(cancel)?;
            entries.inject(&mut scan);
            native_entries.inject(&mut scan);
            if let Some(root) = &managed_root {
                originlm_adapters::native_clients::config::inject_managed_codex(&mut scan, root);
            }
            Ok(scan)
        }),
        args.allow_execution,
        registry,
        native,
        declarations,
        state_dir,
        chatgpt,
        args.usage_providers.contains("cursor"),
    );
    let client = stable_id("extension", &[&origin]);
    service.register_client(&client, host::policy(&args))?;
    if let Some(grant) = host::execution_grant(&args) {
        service.register_execution(&client, grant)?;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| denied())?;
    let allowed: Vec<String> = config
        .allowed_origins
        .iter()
        .chain(config.allowed_extensions.iter())
        .cloned()
        .collect();
    let result = runtime.block_on(originlm_transports::native_messaging::serve_native(
        tokio::io::stdin(),
        tokio::io::stdout(),
        service,
        client,
        &origin,
        &allowed,
    ));
    runtime.shutdown_timeout(Duration::from_millis(100));
    result
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::from(2)
        }
    }
}
