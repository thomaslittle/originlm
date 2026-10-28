//! `originlm status`: the same facts the dashboard badge shows.
//!
//! Exit codes: 0 running and granted, 3 no broker is listening, 4 the broker
//! rejected this application's credential. Other failures are 1.
use originlm_contracts::ErrorCode;
use originlm_control::BrokerClient;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

fn home() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn default_address() -> String {
    if let Some(value) = std::env::var_os("LAI_BROKER_ADDRESS")
        && let Some(value) = value.to_str().filter(|value| !value.is_empty())
    {
        return value.to_string();
    }
    if cfg!(windows) {
        return "originlm-user".into();
    }
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
        && runtime.is_absolute()
    {
        return runtime.join("originlm/user.sock").to_string_lossy().into();
    }
    home()
        .map(|home| home.join(".originlm/user.sock"))
        .unwrap_or_else(|| PathBuf::from("/tmp/originlm-user.sock"))
        .to_string_lossy()
        .into()
}

fn default_token_file() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("LAI_BROKER_TOKEN_FILE") {
        return Some(PathBuf::from(value));
    }
    home().map(|home| home.join(".originlm/broker-token.json"))
}

fn listening(address: &str) -> bool {
    if !cfg!(windows)
        && std::path::Path::new(address).is_absolute()
        && !std::path::Path::new(address).exists()
    {
        return false;
    }
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    let connected = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(2),
            originlm_transports::local_ipc::connect(address),
        )
        .await
    });
    drop(runtime);
    connected.map(|result| result.is_ok()).unwrap_or(false)
}

pub fn run(
    address: Option<String>,
    token_file: Option<PathBuf>,
    json: bool,
) -> originlm_contracts::Result<u8> {
    let address = address.unwrap_or_else(default_address);
    let token_file = token_file.or_else(default_token_file);
    let Some(token_file) = token_file.filter(|path| path.is_file()) else {
        if json {
            println!(
                "{}",
                json!({"running": false, "reason": "NO_CREDENTIAL", "address": address})
            );
        } else {
            println!("OriginLM helper: not configured (no owner-only token descriptor)");
            println!("  expected: ~/.originlm/broker-token.json");
        }
        return Ok(3);
    };
    if !listening(&address) {
        if json {
            println!(
                "{}",
                json!({"running": false, "reason": "NOT_RUNNING", "address": address})
            );
        } else {
            println!("OriginLM helper: not running");
            println!("  address: {address}");
        }
        return Ok(3);
    }
    let mut client = match BrokerClient::from_token_file(&token_file, Some(&address)) {
        Ok(client) => client,
        Err(error) => {
            let denied = error.starts_with("LOCAL_ACCESS_DENIED:")
                || error.starts_with("PERMISSION_DENIED:");
            let reason = if denied {
                "LOCAL_ACCESS_DENIED"
            } else if error.starts_with("PROTOCOL_INCOMPATIBLE:") {
                "PROTOCOL_INCOMPATIBLE"
            } else {
                "CONNECTION_FAILED"
            };
            if json {
                println!(
                    "{}",
                    json!({"running": true, "authorized": if denied { Some(false) } else { None }, "reason": reason, "address": address})
                );
            } else if denied {
                eprintln!("OriginLM helper: running, but this credential was rejected");
            } else {
                eprintln!(
                    "OriginLM helper: reachable, but the authenticated protocol connection failed ({reason}). Check the descriptor and runtime version."
                );
            }
            return Ok(if denied { 4 } else { 1 });
        }
    };
    let snapshot = client.snapshot().map_err(|_| {
        originlm_contracts::DomainError::new(ErrorCode::ProviderUnavailable, "snapshot")
    })?;
    let health = client
        .call("runtime.health", json!({}))
        .unwrap_or(Value::Null);
    let apps = client.apps().ok();
    let providers: Vec<String> = snapshot
        .providers
        .iter()
        .map(|provider| provider.name.clone())
        .collect();
    if json {
        println!(
            "{}",
            json!({
                "running": true,
                "authorized": true,
                "mode": "shared",
                "address": address,
                "clientVersion": env!("CARGO_PKG_VERSION"),
                "protocolVersion": snapshot.protocol_version,
                "revision": snapshot.revision,
                "observedAt": snapshot.observed_at,
                "providers": providers,
                "models": snapshot.models.len(),
                "connectedApps": apps.as_ref().map(|apps| apps.len()),
                "health": health,
            })
        );
    } else {
        println!(
            "OriginLM helper: running (protocol {}, revision {})",
            snapshot.protocol_version, snapshot.revision
        );
        println!("  status client version: {}", env!("CARGO_PKG_VERSION"));
        println!(
            "  providers: {}",
            if providers.is_empty() {
                "none observed".to_string()
            } else {
                providers.join(", ")
            }
        );
        println!("  models: {}", snapshot.models.len());
        match apps {
            Some(apps) => println!("  connected apps: {}", apps.len()),
            None => println!("  connected apps: requires the administrator credential"),
        }
    }
    Ok(0)
}
