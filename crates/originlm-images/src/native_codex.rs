//! Image generation through the installed Codex client and its own login.
//! No OAuth extraction, API key, shell tool, or hosted billing route.
use crate::*;
use originlm_adapters::process::{LaunchSpec, capture_status};
use std::{collections::BTreeMap, ffi::OsString, time::Duration};

pub struct NativeCodexProvider {
    executable: PathBuf,
    home: PathBuf,
}
impl NativeCodexProvider {
    pub fn new(executable: PathBuf, home: PathBuf) -> Result<Self> {
        if !home.is_absolute() {
            return Err(invalid());
        }
        let executable = originlm_adapters::codex::resolve_executable(&executable)
            .map_err(|_| ImageError::new(ErrorKind::NoProvider))?;
        Ok(Self { executable, home })
    }
}
fn thread_id(output: &str) -> Result<String> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value["type"] == "thread.started")
        .and_then(|value| value["thread_id"].as_str().map(str::to_owned))
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        .ok_or_else(failed)
}
fn collect_image(home: &Path, id: &str) -> Result<Vec<u8>> {
    let root = home
        .join("generated_images")
        .canonicalize()
        .map_err(|_| failed())?;
    let directory = root.join(id).canonicalize().map_err(|_| failed())?;
    if !directory.starts_with(&root) {
        return Err(failed());
    }
    let mut images = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|_| failed())?.take(5) {
        let path = entry.map_err(|_| failed())?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("png") {
            continue;
        }
        let path = path.canonicalize().map_err(|_| failed())?;
        if !path.starts_with(&root) {
            return Err(failed());
        }
        let bytes = read_bounded(&path).map_err(|_| failed())?;
        decode(&bytes)?;
        images.push(bytes);
    }
    if images.len() != 1 {
        return Err(failed());
    }
    Ok(images.remove(0))
}
impl ImageProvider for NativeCodexProvider {
    fn provider(&self) -> &str {
        "codex_subscription"
    }
    fn model(&self) -> &str {
        "codex-native-image-generation"
    }
    fn generate<'a>(&'a self, request: ProviderRequest) -> Generation<'a> {
        Box::pin(async move {
            let mut output = Vec::new();
            for _ in 0..request.count {
                let workspace =
                    tempfile::tempdir().map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
                let mut args: Vec<OsString> = vec![
                    "exec".into(),
                    "--ephemeral".into(),
                    "--json".into(),
                    "--skip-git-repo-check".into(),
                    "--enable".into(),
                    "image_generation".into(),
                    "--sandbox".into(),
                    "workspace-write".into(),
                ];
                for feature in [
                    "shell_tool",
                    "unified_exec",
                    "apps",
                    "plugins",
                    "multi_agent",
                    "hooks",
                    "computer_use",
                    "browser_use",
                ] {
                    args.extend(["--disable".into(), feature.into()]);
                }
                args.extend(["-c".into(), "web_search=\"disabled\"".into()]);
                if let Some(reference) = &request.reference_png {
                    let path = workspace.path().join("reference.png");
                    std::fs::write(&path, reference)
                        .map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
                    args.extend(["--image".into(), path.into_os_string()]);
                }
                args.push(format!("Use the native image generation tool exactly once. Generate the requested image{} and save it using the native image tool. Do not use shell, code, MCP tools, or draw an approximation. If generation fails, report the actual failure and stop.\n{}",if request.reference_png.is_some(){" by editing the supplied reference"}else{""},request.prompt).into());
                let spec = LaunchSpec {
                    executable: self.executable.clone(),
                    args,
                    cwd: workspace.path().to_path_buf(),
                    env: BTreeMap::from([(
                        "CODEX_HOME".into(),
                        self.home.clone().into_os_string(),
                    )]),
                };
                let (success, events) = capture_status(spec, Duration::from_secs(60))
                    .await
                    .map_err(|_| failed())?;
                if !success {
                    return Err(failed());
                }
                let id = thread_id(&events)?;
                output.push(collect_image(&self.home, &id)?);
            }
            Ok(output)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_thread_binding_rejects_paths_and_missing_artifacts() {
        assert_eq!(
            thread_id("{\"type\":\"thread.started\",\"thread_id\":\"abc-123\"}").unwrap(),
            "abc-123"
        );
        assert!(thread_id("{\"type\":\"thread.started\",\"thread_id\":\"../other\"}").is_err());
        assert!(thread_id("{\"type\":\"item.completed\"}").is_err());
        let home = tempfile::tempdir().unwrap();
        assert!(collect_image(home.path(), "abc-123").is_err());
    }
}
