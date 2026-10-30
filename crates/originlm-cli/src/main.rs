mod args;
mod inventory;
use inventory::source;
use originlm_service::InventorySource;
mod broker_cli;
mod endpoints;
mod host;
mod native_profiles;
mod status;
use args::{Action, Args};
use originlm_contracts::{DomainError, ErrorCode, Result};
use serde_json::json;
use std::{sync::Arc, time::Duration};

const HELP: &str = "OriginLM - native discovery and SDK helper
Usage:
  originlm serve --ipc LOCAL_ADDRESS [--allow-passive-discovery]
  originlm serve --ipc LOCAL_ADDRESS --independent (--token-file ABSOLUTE_PATH | --grants-file ABSOLUTE_PATH)
      [--admin-token-file ABSOLUTE_PATH] [--idle-timeout SECONDS] [--allow-passive-discovery]
      [--os-notifications (requires an os-notifications build)]
  originlm connect --ipc LOCAL_ADDRESS
  originlm scan --json [--home ABSOLUTE_DIRECTORY] [--executable-only-provider PROVIDER]
  originlm status [--json] [--ipc ADDRESS] [--token-file ABSOLUTE_FILE]
      Prints the running helper, its providers and connected apps.
      Exit: 0 running, 3 not running, 4 credential rejected.
  originlm serve --stdio [--allow-passive-discovery] [--allow-identity] [--allow-codex-metadata] [--home ABSOLUTE_DIRECTORY]
  originlm serve --http --port 4499 --assets ABSOLUTE_DIST_DIRECTORY
      [--allow-passive-discovery] [--allow-codex-metadata]
      [--allow-codex-execution --workspace ABSOLUTE_DIRECTORY]
      [--allow-writes] [--allow-native-policy] [--review-native-requests] [--allow-paid-options]
      [--warm-metadata] [--no-metadata-cache]
  Reusable provider grants: --allow-metadata PROVIDER --allow-execution PROVIDER
  --allow-usage cursor additionally reads Cursor's own dashboard allowances with the
  session token stored by Cursor; the token is never logged, serialized or exported.
  --executable-only-provider PROVIDER may repeat for known discovery providers; retains private executable candidates without publishing their profiles or routes.
  Execution providers: codex, claude_code, gemini_cli, antigravity, grok, ollama, lmstudio, openai_api, anthropic_api, gemini_api, xai_api, compatible_api, opencode, muse.
  --endpoints ABSOLUTE_JSON registers local or API endpoints without contacting them.
  --native-profiles ABSOLUTE_JSON explicitly registers Gemini CLI, Antigravity ACP, or Grok native profiles; the file contains paths/labels, never tokens.
  --chatgpt-app-name NAME opts in a separate managed Codex profile and SIWC auth RPCs; grant --allow-metadata codex_managed to use them.
  --billing-declarations ABSOLUTE_JSON declares billing for discovered connections whose provider leaves it unknown; recorded as user-supplied evidence, never verification.
  originlm --version

Metadata observations are kept in the broker-owned state directory (state.db,
no account identity, owner-only permissions) and the last ready snapshot is
reloaded after a restart as stale-while-revalidate data; --no-metadata-cache
turns that off. A parent-owned broker stops when its trusted control input
closes. An --independent broker owns its lifetime, publishes a reusable
owner-only token file for approved applications, and can stop after an idle
timeout. --grants-file instead mints a distinct reusable credential and stable
app identity for each manifest entry, so ordinary applications never share the
owner credential or each other's account scope; an administrator can revoke one
application and its grant live. It never silently replaces a live listener.
Serving never scans on startup unless the host passes --warm-metadata (HTTP only,
with --allow-passive-discovery and a metadata provider grant): the helper then runs
one passive scan and the already-permitted metadata checks in the background, so
results are ready when a browser pairs. Providers are checked concurrently; the
profiles of one provider are checked in order. --allow-identity reveals the signed-in account label (email) that the provider reported; without it the projection withholds identity. --allow-passive-discovery explicitly
authorizes metadata-only scans requested by the owning SDK. No credentials or inference.
--allow-codex-metadata separately permits explicit connections.probe requests
through the installed Codex client. This may read its login and contact its
account/catalog/usage services, but never starts threads or model generation.
--allow-codex-execution authorizes explicit session/turn requests, never generation
at startup. A --workspace is required; its public name is default. Writes, native-policy
tool access, native approval review, and paid options each require their own flag.
Native policy delegates tool and filesystem/network limits to the installed client;
it is not a Kit sandbox. HTTP is loopback-only
and requires explicit single-use browser pairing. No tokens are printed.
--home isolates all discovery roots and excludes the inherited PATH/overrides.";

/// Host-initiated warm-up (opt-in). It uses only what the host already permitted for browsers:
/// one passive scan, then metadata checks. Different providers run concurrently; the profiles of
/// one provider run in order, because a status check can refresh that provider's own credentials
/// and two checks must never race that refresh. Failures are ignored: the browser can still check.
async fn warm_metadata(
    service: Arc<originlm_service::NativeService>,
    policy: originlm_core::ClientPolicy,
) {
    const HOST: &str = "host-warmup";
    if service.register_client(HOST, policy).is_err() {
        return;
    }
    let listed = match service
        .handle(HOST, "discovery.refresh", json!({"level": "passive"}))
        .await
    {
        Ok(_) => service.handle(HOST, "connections.list", json!({})).await,
        Err(error) => Err(error),
    };
    let mut by_provider: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    if let Some(connections) = listed.ok().as_ref().and_then(|value| value.as_array()) {
        for connection in connections {
            if let (Some(provider), Some(id)) =
                (connection["providerId"].as_str(), connection["id"].as_str())
            {
                by_provider
                    .entry(provider.to_owned())
                    .or_default()
                    .push(id.to_owned());
            }
        }
    }
    let mut checks = tokio::task::JoinSet::new();
    for ids in by_provider.into_values() {
        let service = service.clone();
        checks.spawn(async move {
            for id in ids {
                let _ = service
                    .handle(HOST, "connections.probe", json!({"connectionId": id}))
                    .await;
            }
        });
    }
    while checks.join_next().await.is_some() {}
    service.close_client(HOST);
}

async fn execute(args: Args) -> Result<()> {
    if matches!(args.action, Action::Connect) {
        return broker_cli::connect(args.ipc.as_deref().unwrap()).await;
    }
    let registry = Arc::new(endpoints::load(args.endpoints.as_deref())?);
    let state_dir = host::default_state_dir(args.home.as_deref());
    let chatgpt = args
        .chatgpt_app_name
        .clone()
        .map(|name| {
            let root = state_dir.as_ref().ok_or_else(|| {
                DomainError::new(
                    ErrorCode::LocalAccessDenied,
                    "Managed ChatGPT state directory is unavailable",
                )
            })?;
            let manager =
                originlm_chatgpt_auth::AuthManager::new(root.join("chatgpt-auth"), name.clone())
                    .map_err(|_| {
                        DomainError::new(
                            ErrorCode::ProviderUnavailable,
                            "Managed ChatGPT state is unavailable",
                        )
                    })?;
            Ok::<_, DomainError>((Arc::new(manager), name))
        })
        .transpose()?;
    let mut native = native_profiles::load(args.native_profiles.as_deref())?;
    host::validate_executable_only_sources(&args, &registry, &native)?;
    native.browser_blocker = native_profiles::packaged_browser_blocker();
    let native = Arc::new(native);
    let declarations = Arc::new(native_profiles::load_declarations(
        args.billing_declarations.as_deref(),
    )?);
    let original = source(args.home.clone(), args.executable_only_providers.clone());
    let scan_registry = registry.clone();
    let scan_native = native.clone();
    let managed_root = chatgpt.as_ref().and(state_dir.as_ref()).cloned();
    let inventory: InventorySource = Arc::new(move |cancel| {
        let mut result = original(cancel)?;
        scan_registry.inject(&mut result);
        scan_native.inject(&mut result);
        if let Some(root) = &managed_root {
            originlm_adapters::native_clients::config::inject_managed_codex(&mut result, root);
        }
        Ok(result)
    });
    let service = originlm_kit::build_service_with_cursor_usage(
        inventory,
        args.allow_execution,
        registry,
        native,
        declarations,
        state_dir,
        chatgpt,
        args.usage_providers.contains("cursor"),
    );
    #[cfg(feature = "os-notifications")]
    if args.os_notifications {
        service.install_os_notifications();
    }
    service.enable_autonomous_refresh(args.metadata_providers.clone());
    service.enable_inventory_refresh(args.allow_passive);
    if matches!(args.action, Action::Serve)
        && !args.no_metadata_cache
        && let Some(directory) = host::default_state_dir(args.home.as_deref())
        && let Err(error) = service.enable_state_store(directory.join("state.db"))
    {
        eprintln!("storage: {error}");
    }
    if matches!(args.action, Action::Serve) {
        service.start_background_tasks();
    }
    let policy = host::policy(&args);
    let grant = host::execution_grant(&args);
    match args.action {
        Action::Scan => {
            service.register_client("stdio-owner", policy)?;
            service
                .handle(
                    "stdio-owner",
                    "discovery.refresh",
                    json!({"level":"passive"}),
                )
                .await?;
            println!(
                "{}",
                service
                    .handle("stdio-owner", "snapshot.get", json!({}))
                    .await?
            );
            Ok(())
        }
        Action::Serve if args.ipc.is_some() => {
            let lifetime = if args.independent {
                broker_cli::Lifetime::Independent {
                    token_file: args.token_file.clone(),
                    admin_token_file: args.admin_token_file.clone(),
                    grants_file: args.grants_file.clone(),
                    idle_timeout: args.idle_timeout_seconds.map(Duration::from_secs),
                }
            } else {
                broker_cli::Lifetime::Parent
            };
            broker_cli::serve(
                service,
                args.ipc.as_deref().unwrap(),
                policy,
                grant,
                lifetime,
            )
            .await
        }
        Action::Serve if args.http => {
            if args.warm_metadata {
                tokio::spawn(warm_metadata(service.clone(), policy.clone()));
            }
            let server = originlm_transports::http::start(
                service,
                originlm_transports::http::HttpOptions {
                    port: args.port,
                    policy,
                    execution: grant,
                    assets: args.assets,
                },
            )
            .await?;
            println!("OriginLM: {}", server.url);
            println!(
                "Pairing code (single use, expires in five minutes): {}",
                server.pairing_code
            );
            tokio::signal::ctrl_c().await.map_err(|_| {
                DomainError::new(
                    ErrorCode::ProviderUnavailable,
                    "Shutdown signal unavailable",
                )
            })?;
            server.close().await;
            Ok(())
        }
        Action::Serve => {
            service.register_client("stdio-owner", policy)?;
            if let Some(grant) = grant {
                service.register_execution("stdio-owner", grant)?;
            }
            originlm_transports::serve_io(
                tokio::io::stdin(),
                tokio::io::stdout(),
                service,
                "stdio-owner".into(),
                Default::default(),
            )
            .await
        }
        _ => Ok(()),
    }
}
fn main() -> std::process::ExitCode {
    let result = (|| -> originlm_contracts::Result<u8> {
        let args = Args::parse(std::env::args_os().skip(1))?;
        match args.action {
            Action::Help => {
                println!("{HELP}");
                return Ok(0);
            }
            Action::Version => {
                println!("originlm {}", env!("CARGO_PKG_VERSION"));
                return Ok(0);
            }
            Action::Status => {
                return status::run(args.ipc.clone(), args.token_file.clone(), args.json);
            }
            _ => {}
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(4)
            .enable_all()
            .build()
            .map_err(|_| DomainError::new(ErrorCode::ProviderUnavailable, "Runtime unavailable"))?;
        let result = runtime.block_on(execute(args));
        // Tokio stdin uses a blocking thread; an idle pipe must not hold shutdown forever.
        runtime.shutdown_timeout(Duration::from_millis(100));
        result.map(|_| 0u8)
    })();
    match result {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}
