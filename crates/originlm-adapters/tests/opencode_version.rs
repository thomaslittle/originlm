//! Synthetic executables only: never launch an installed AI client.
#[cfg(unix)]
mod unix {
    use originlm_adapters::opencode::server;
    use originlm_contracts::ErrorCode;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn v2_is_rejected_before_launching_the_legacy_server() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("opencode");
        let marker = dir.path().join("legacy-server-launched");
        std::fs::write(&executable, format!(
            "#!/usr/bin/env python3\nimport sys,pathlib\nif sys.argv[1:] == ['--version']:\n print('opencode v2.0.18')\nelse:\n pathlib.Path({:?}).write_text('wrong protocol')\n sys.exit(77)\n",
            marker.to_str().unwrap()
        )).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = match server::start(&executable, dir.path()).await {
            Ok(_) => panic!("unsupported protocol was launched"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::ProtocolIncompatible);
        assert!(!marker.exists(), "must not run v2 with legacy flags");
    }
}
