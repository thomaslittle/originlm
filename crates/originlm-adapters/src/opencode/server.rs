//! Kit-owned `opencode serve` for a discovered OpenCode install.
//!
//! Mirrors how T3 Code drives OpenCode: spawn `opencode serve` on a free
//! loopback port with a per-process `OPENCODE_SERVER_PASSWORD`, talk to it over
//! HTTP basic auth, and kill the whole process tree when the metadata probe or
//! session ends. Only the native binary is executed (never the npm `.cmd`
//! shim), external plugins are disabled with `--pure`, and sessions are
//! created with a deny-all permission ruleset (see `client::open`).
use crate::{
    http::{Authentication, Endpoint},
    process::{LaunchSpec, ManagedChild, capture},
};
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    hash::{BuildHasher, Hasher},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const STARTUP: Duration = Duration::from_secs(20);

fn missing() -> DomainError {
    DomainError::new(
        ErrorCode::RuntimeMissing,
        "A native OpenCode executable is unavailable",
    )
}

/// The native OpenCode binary behind a discovered command. The npm shim
/// (`opencode.cmd` / `opencode.ps1` / extensionless `opencode` on Windows)
/// is resolved to the packaged `opencode.exe` rather than run through a shell.
pub fn resolve_executable(candidate: &Path) -> Result<PathBuf> {
    if !candidate.is_absolute() || !candidate.is_file() {
        return Err(missing());
    }
    let name = candidate
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name == "opencode.exe" {
        return Ok(candidate.to_owned());
    }
    let wrapper =
        name == "opencode.cmd" || name == "opencode.ps1" || (cfg!(windows) && name == "opencode");
    if wrapper {
        let base = candidate.parent().ok_or_else(missing)?;
        for binary in [
            base.join("node_modules/opencode-ai/bin/opencode.exe"),
            base.join("../lib/node_modules/opencode-ai/bin/opencode.exe"),
        ] {
            if binary.is_file() {
                return Ok(binary);
            }
        }
        return Err(missing());
    }
    if !cfg!(windows) && name == "opencode" {
        return Ok(candidate.to_owned());
    }
    Err(missing())
}

fn password() -> String {
    // Per-process random keys seeded from OS entropy; 128 bits is plenty for a
    // loopback-only basic-auth secret that lives as long as one server.
    let mut out = String::with_capacity(32);
    for salt in 0u64..2 {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(salt);
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        hasher.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", hasher.finish()));
    }
    out
}

fn free_port() -> Result<u16> {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|_| {
            DomainError::new(
                ErrorCode::ProviderUnavailable,
                "No loopback port for OpenCode",
            )
        })
}

pub struct ManagedServer {
    child: ManagedChild,
    pub endpoint: Endpoint,
    /// The `/provider` catalog read that proved the server was ready.
    pub providers: Value,
}

/// Start `opencode serve` in `cwd` and wait until its authenticated
/// `/provider` endpoint answers.
pub async fn start(located_executable: &Path, cwd: &Path) -> Result<ManagedServer> {
    let executable = resolve_executable(located_executable)?;
    // OpenCode 2 uses a different launch and HTTP/event protocol. Do not
    // start it with legacy flags or infer a generation from a 200 HTML page.
    let version = capture(
        LaunchSpec {
            executable: executable.clone(),
            args: vec!["--version".into()],
            cwd: cwd.into(),
            env: BTreeMap::new(),
        },
        Duration::from_secs(4),
    )
    .await?;
    super::version::require_legacy(&version)?;
    let port = free_port()?;
    let secret = password();
    let mut child = ManagedChild::spawn(LaunchSpec {
        executable,
        args: vec![
            "serve".into(),
            "--pure".into(),
            "--hostname=127.0.0.1".into(),
            format!("--port={port}").into(),
        ],
        cwd: cwd.into(),
        env: BTreeMap::from([("OPENCODE_SERVER_PASSWORD".into(), secret.clone().into())]),
    })?;
    let endpoint = Endpoint::new(
        &format!("http://127.0.0.1:{port}/"),
        Authentication::Basic("opencode".into(), secret),
        true,
    )?;
    let deadline = Instant::now() + STARTUP;
    loop {
        if child.exited() {
            return Err(DomainError::new(
                ErrorCode::ProviderUnavailable,
                "OpenCode server exited during startup",
            ));
        }
        if let Ok(Ok(providers)) = tokio::time::timeout(
            Duration::from_secs(5),
            endpoint.json_query_limited("provider", &[], None, 16 * 1024 * 1024),
        )
        .await
        {
            return Ok(ManagedServer {
                child,
                endpoint,
                providers,
            });
        }
        if Instant::now() >= deadline {
            child.shutdown().await;
            return Err(DomainError::new(
                ErrorCode::Timeout,
                "OpenCode server did not become ready",
            ));
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// Live metadata: a short-lived server answers `/provider`, then dies.
/// Live metadata: OpenCode 2 answers from its running local service; OpenCode
/// 1.x gets a short-lived kit-owned server so `/provider` can be read.
pub async fn probe(
    located_executable: &Path,
    connection: &str,
    route: &str,
) -> Result<ConnectionMetadata> {
    let executable = resolve_executable(located_executable)?;
    let cwd = std::env::current_dir().map_err(|_| {
        DomainError::new(
            ErrorCode::ProviderUnavailable,
            "OpenCode working directory is unavailable",
        )
    })?;
    let version = capture(
        LaunchSpec {
            executable,
            args: vec!["--version".into()],
            cwd,
            env: BTreeMap::new(),
        },
        Duration::from_secs(4),
    )
    .await?;
    if super::version::installed_major(&version) == Some(2) {
        return super::v2_service::probe(connection, route, &version).await;
    }
    let dir = tempfile::Builder::new()
        .prefix("originlm-opencode-probe-")
        .tempdir()
        .map_err(|_| {
            DomainError::new(
                ErrorCode::ProviderUnavailable,
                "Isolated workspace unavailable",
            )
        })?;
    let mut server = start(located_executable, dir.path()).await?;
    let result = super::catalog(connection, route, &server.providers, &BTreeMap::new());
    server.child.shutdown().await;
    result
}

/// A text session on a kit-owned server; closing the session stops the server.
pub async fn open(
    located_executable: &Path,
    model: Model,
    provider: String,
    workspace: PathBuf,
) -> Result<Box<dyn Agent>> {
    open_session(located_executable, model, provider, workspace, false, None).await
}
/// Like [`open`]; `persistent` keeps the native session in OpenCode's storage
/// on suspend, and `resume` reattaches such a session on a new server.
pub async fn open_session(
    located_executable: &Path,
    model: Model,
    provider: String,
    workspace: PathBuf,
    persistent: bool,
    resume: Option<String>,
) -> Result<Box<dyn Agent>> {
    let mut server = start(located_executable, &workspace).await?;
    match super::client::open_session(
        server.endpoint.clone(),
        model,
        provider,
        workspace,
        persistent,
        resume,
    )
    .await
    {
        Ok(inner) => Ok(Box::new(ServerAgent {
            inner,
            child: server.child,
        })),
        Err(error) => {
            server.child.shutdown().await;
            Err(error)
        }
    }
}

struct ServerAgent {
    inner: Box<dyn Agent>,
    child: ManagedChild,
}
impl Agent for ServerAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        context: TurnContext,
    ) -> BoxFuture<'a, ()> {
        self.inner.turn(prompt, options, context)
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let result = self.inner.close().await;
            self.child.shutdown().await;
            result
        })
    }
    fn native_resumption(&self) -> bool {
        self.inner.native_resumption()
    }
    fn image_input(&self) -> bool {
        self.inner.image_input()
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            let id = self.inner.suspend().await?;
            self.child.shutdown().await;
            Ok(id)
        })
    }
}
