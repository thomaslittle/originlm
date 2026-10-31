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
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

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
    if let Action::Install(provider) = &args.action {
        return install_provider(provider, args.home.as_deref()).await;
    }
    if let Action::Auth(provider) = &args.action {
        return auth_provider(provider, args.home.as_deref()).await;
    }
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
async fn install_provider(provider: &str, home: Option<&Path>) -> Result<()> {
    if provider != "antigravity" {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Only the Antigravity ACP server has a managed installer",
        ));
    }
    let base = home
        .map(|home| home.join(".gemini/antigravity-acp"))
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .map(|home| home.join(".gemini/antigravity-acp"))
        })
        .ok_or_else(|| {
            DomainError::new(ErrorCode::LocalAccessDenied, "Home directory is unavailable")
        })?;
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    let release =
        originlm_adapters::native_clients::antigravity_install::release_for(platform, arch)
            .ok_or_else(|| {
                DomainError::new(
                    ErrorCode::CapabilityUnsupported,
                    "Antigravity is unavailable for this platform",
                )
            })?;
    let path =
        originlm_adapters::native_clients::antigravity_install::install(&base, release).await?;
    println!(
        "Antigravity ACP server {} installed at {}",
        release.version,
        path.display()
    );
    Ok(())
}

fn home_directory(home: Option<&Path>) -> Result<PathBuf> {
    home.map(Path::to_path_buf)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
        })
        .ok_or_else(|| {
            DomainError::new(ErrorCode::LocalAccessDenied, "Home directory is unavailable")
        })
}

fn gemini_executable(home: &Path) -> Option<PathBuf> {
    let tools = home.join(".t3/tools/gemini");
    if let Ok(entries) = std::fs::read_dir(&tools) {
        let mut candidates: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path().join("npm/bin/gemini"))
            .filter(|path| path.is_file())
            .collect();
        candidates.sort();
        if let Some(path) = candidates.pop() {
            return Some(path);
        }
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|root| root.join("gemini"))
            .find(|path| path.is_file())
    })
}

async fn auth_provider(provider: &str, home: Option<&Path>) -> Result<()> {
    let home = home_directory(home)?;
    let (native, executable, profile_home) = match provider {
        "antigravity" => {
            let profile_home = home.join(".gemini/antigravity-acp");
            let executable = profile_home.join(if cfg!(windows) {
                "agy_acp_server.exe"
            } else {
                "agy_acp_server.par"
            });
            (
                originlm_adapters::native_clients::config::NativeProvider::Antigravity,
                executable,
                profile_home,
            )
        }
        "gemini_cli" => {
            let executable = gemini_executable(&home).ok_or_else(|| {
                DomainError::new(ErrorCode::RuntimeMissing, "Gemini CLI is not installed")
            })?;
            (
                originlm_adapters::native_clients::config::NativeProvider::GeminiCli,
                executable,
                home.join(".gemini"),
            )
        }
        _ => {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "Unsupported provider for interactive auth",
            ));
        }
    };
    if !executable.is_file() {
        return Err(DomainError::new(
            ErrorCode::RuntimeMissing,
            "The provider runtime is not installed; run `originlm install antigravity` first",
        ));
    }
    let scratch = tempfile::Builder::new()
        .prefix("originlm-auth-")
        .tempdir()
        .map_err(|_| {
            DomainError::new(ErrorCode::ProviderUnavailable, "Auth workspace is unavailable")
        })?;
    let url_file = scratch.path().join("authorization-url");
    let browser = scratch.path().join(if cfg!(windows) {
        "browser.cmd"
    } else {
        "browser.sh"
    });
    let script = if cfg!(windows) {
        format!(
            "@echo off\r\necho %~1>\"{}\"\r\n",
            url_file.to_string_lossy()
        )
    } else {
        format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > \"{}\"\n",
            url_file.to_string_lossy()
        )
    };
    std::fs::write(&browser, script)
        .map_err(|_| DomainError::new(ErrorCode::LocalAccessDenied, "Auth helper is unavailable"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o700)).map_err(
            |_| DomainError::new(ErrorCode::LocalAccessDenied, "Auth helper is unavailable"),
        )?;
    }
    let profile = originlm_adapters::native_clients::config::NativeProfile {
        id: provider.to_owned(),
        label: provider.to_owned(),
        provider: native,
        executable,
        profile_home,
        auth_method: Some("oauth-personal".into()),
        billing: None,
    };
    let prepared = originlm_adapters::native_clients::launch::prepare_auth(
        &profile,
        scratch.path(),
        &browser,
    )?;
    let method = prepared
        .auth_method
        .clone()
        .unwrap_or_else(|| "oauth-personal".into());
    let spec = prepared.spec;
    let _scratch = prepared.scratch;
    let auth = originlm_adapters::acp::authenticate(spec, provider.to_owned(), "auth".into(), &method);
    tokio::pin!(auth);
    let mut printed = false;
    loop {
        tokio::select! {
            result = &mut auth => {
                result?;
                println!("AUTHENTICATED");
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if !printed
                    && let Ok(url) = std::fs::read_to_string(&url_file)
                    && !url.trim().is_empty()
                {
                    println!("AUTHORIZATION_URL={}", url.trim());
                    printed = true;
                }
            }
        }
    }
    Ok(())
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
