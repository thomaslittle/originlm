use originlm_adapters::codex::{managed_launch_spec, resolve_executable};
use std::path::Path;

fn write_host(dir: &Path) {
    let host = dir.join(if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    });
    std::fs::write(&host, b"host").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&host).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&host, permissions).unwrap();
    }
}

#[test]
fn managed_codex_launch_isolated_and_uses_responses_provider() {
    let temp = tempfile::tempdir().unwrap();
    let executable = temp
        .path()
        .join(if cfg!(windows) { "codex.exe" } else { "codex" });
    std::fs::write(&executable, b"native").unwrap();
    write_host(temp.path());
    let home = temp.path().join("managed-home");
    let spec =
        managed_launch_spec(&executable, &home, temp.path(), "SENTINEL_ACCESS_TOKEN").unwrap();
    assert_eq!(
        spec.env.get(std::ffi::OsStr::new("CODEX_HOME")).unwrap(),
        home.as_os_str()
    );
    assert_eq!(
        spec.env.get(std::ffi::OsStr::new("ACCESS_TOKEN")).unwrap(),
        "SENTINEL_ACCESS_TOKEN"
    );
    assert!(
        !spec
            .env
            .contains_key(std::ffi::OsStr::new("OPENAI_API_KEY"))
    );
    let args = spec
        .args
        .iter()
        .map(|a| a.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert!(args.windows(2).any(|w| w == ["--listen", "stdio://"]));
    for value in [
        "model_provider=\"openai_chatgpt_plan\"",
        "model_providers.openai_chatgpt_plan.base_url=\"https://api.openai.com/v1\"",
        "model_providers.openai_chatgpt_plan.env_key=\"ACCESS_TOKEN\"",
        "model_providers.openai_chatgpt_plan.wire_api=\"responses\"",
        "model_providers.openai_chatgpt_plan.requires_openai_auth=false",
        "model_providers.openai_chatgpt_plan.supports_websockets=false",
    ] {
        assert!(args.iter().any(|arg| arg == value), "missing {value}");
    }
}
#[test]
fn direct_native_executable_needs_its_adjacent_host() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .join(if cfg!(windows) { "codex.exe" } else { "codex" });
    std::fs::write(&path, b"native").unwrap();
    assert!(resolve_executable(&path).is_err());
    write_host(temp.path());
    assert_eq!(resolve_executable(&path).unwrap(), path);
}
#[cfg(not(windows))]
#[test]
fn shebang_launcher_does_not_need_the_adjacent_host() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("codex");
    std::fs::write(&path, b"#!/usr/bin/env bash\n").unwrap();
    assert_eq!(resolve_executable(&path).unwrap(), path);
}
#[test]
fn known_windows_npm_wrapper_resolves_to_packaged_native_binary() {
    let temp = tempfile::tempdir().unwrap();
    let wrapper = temp.path().join("codex.cmd");
    std::fs::write(&wrapper, "must not execute this shell wrapper").unwrap();
    let arch = if cfg!(target_arch = "aarch64") {
        ("codex-win32-arm64", "aarch64-pc-windows-msvc")
    } else {
        ("codex-win32-x64", "x86_64-pc-windows-msvc")
    };
    let bin = temp.path().join(format!(
        "node_modules/@openai/codex/node_modules/@openai/{}/vendor/{}/bin/codex.exe",
        arch.0, arch.1
    ));
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, []).unwrap();
    write_host(bin.parent().unwrap());
    assert_eq!(resolve_executable(&wrapper).unwrap(), bin);
}
#[test]
fn arbitrary_wrapper_and_absent_binary_are_not_executed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("arbitrary.cmd");
    std::fs::write(&path, "malicious").unwrap();
    assert!(resolve_executable(&path).is_err());
    assert!(resolve_executable(&temp.path().join("codex.cmd")).is_err());
    assert!(resolve_executable(std::path::Path::new("codex")).is_err());
}

#[cfg(windows)]
#[test]
fn extensionless_npm_shim_uses_the_native_package_not_a_shell() {
    let temp = tempfile::tempdir().unwrap();
    let shim = temp.path().join("codex");
    std::fs::write(&shim, "#!/bin/sh\nDO-NOT-EXECUTE").unwrap();
    let (package, target) = if cfg!(target_arch = "aarch64") {
        ("codex-win32-arm64", "aarch64-pc-windows-msvc")
    } else {
        ("codex-win32-x64", "x86_64-pc-windows-msvc")
    };
    let bin = temp.path().join(format!(
        "node_modules/@openai/codex/node_modules/@openai/{package}/vendor/{target}/bin/codex.exe"
    ));
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, []).unwrap();
    write_host(bin.parent().unwrap());
    assert_eq!(resolve_executable(&shim).unwrap(), bin);
}
