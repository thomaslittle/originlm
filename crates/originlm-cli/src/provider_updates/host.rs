//! Everything provider maintenance asks of the machine, behind one trait so the
//! ownership rules and the runner are tested without real installers.
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    pub success: bool,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

/// `Sync` so independent installs are checked concurrently.
pub trait Host: Sync {
    fn platform(&self) -> Platform;
    fn env(&self, key: &str) -> Option<String>;
    /// Resolve a bare command name through `PATH` (and `PATHEXT` on Windows).
    fn which(&self, command: &str) -> Option<PathBuf>;
    /// The path with every symlink followed, or the path itself when that fails.
    fn real_path(&self, path: &Path) -> PathBuf;
    fn is_file(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    /// Read a small text file; `None` when missing, unreadable or over `max` bytes.
    fn read_small(&self, path: &Path, max: u64) -> Option<String>;
    /// Run a program with stdin at EOF. `None` only when it could not start.
    fn run(
        &self,
        program: &Path,
        args: &[String],
        env: &BTreeMap<String, String>,
        timeout: Duration,
    ) -> Option<CommandOutput>;
    /// GET a small text resource. `None` on any failure; never retried elsewhere.
    fn http_get(&self, url: &str) -> Option<String>;
}

pub struct SystemHost;

/// Remove the `\\?\` prefix `canonicalize` adds on Windows so paths stay comparable.
fn plain(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

const OUTPUT_LIMIT: usize = 512 * 1024;

/// Read a pipe on a thread into a shared tail buffer.
fn drain(
    mut reader: impl Read + Send + 'static,
) -> (Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<()>) {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let sink = kept.clone();
    let handle = std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 {
                break;
            }
            let Ok(mut kept) = sink.lock() else { break };
            kept.extend_from_slice(&chunk[..count]);
            // Keep the tail: the last error line is what a failed update reports.
            if kept.len() > OUTPUT_LIMIT {
                let excess = kept.len() - OUTPUT_LIMIT;
                kept.drain(..excess);
            }
        }
    });
    (kept, handle)
}

/// A detached grandchild can hold a pipe open after the process exits; wait a
/// bounded time for the reader, then take what was read.
fn collect(pipe: (Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<()>)) -> String {
    let (kept, handle) = pipe;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let bytes = kept.lock().map(|kept| kept.clone()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Kill the process and everything it started: an installer's own children
/// would otherwise keep writing after the timeout.
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        // The child leads its own process group (see `process_group(0)`).
        let _ = Command::new("kill")
            .args(["-s", "KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

impl Host for SystemHost {
    fn platform(&self) -> Platform {
        Platform::current()
    }

    fn env(&self, key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    fn which(&self, command: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        let extensions: Vec<String> = if cfg!(windows) {
            std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                .split(';')
                .filter(|ext| !ext.is_empty())
                .map(str::to_ascii_lowercase)
                .collect()
        } else {
            vec![String::new()]
        };
        for directory in std::env::split_paths(&path) {
            if !directory.is_absolute() {
                continue;
            }
            for extension in &extensions {
                let candidate = directory.join(format!("{command}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn real_path(&self, path: &Path) -> PathBuf {
        std::fs::canonicalize(path)
            .map(plain)
            .unwrap_or_else(|_| path.to_path_buf())
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn read_small(&self, path: &Path, max: u64) -> Option<String> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() || metadata.len() > max {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        String::from_utf8(bytes).ok()
    }

    fn run(
        &self,
        program: &Path,
        args: &[String],
        env: &BTreeMap<String, String>,
        timeout: Duration,
    ) -> Option<CommandOutput> {
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(env)
            // Nothing answers an update or probe, so a prompt must see EOF, not wait.
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().ok()?;
        let stdout = drain(child.stdout.take()?);
        let stderr = drain(child.stderr.take()?);
        let started = Instant::now();
        let (success, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status.success(), false),
                Ok(None) if started.elapsed() >= timeout => {
                    kill_tree(&mut child);
                    break (false, true);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break (false, false),
            }
        };
        Some(CommandOutput {
            success,
            timed_out,
            stdout: collect(stdout),
            stderr: collect(stderr),
        })
    }

    fn http_get(&self, url: &str) -> Option<String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        runtime.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(6))
                .user_agent(concat!("originlm/", env!("CARGO_PKG_VERSION")))
                .build()
                .ok()?;
            let response = client.get(url).send().await.ok()?;
            if !response.status().is_success() {
                return None;
            }
            let text = response.text().await.ok()?;
            (text.len() <= 64 * 1024).then_some(text)
        })
    }
}
