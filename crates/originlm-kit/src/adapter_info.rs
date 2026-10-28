//! Public static implementation coverage. No installation or account is implied.
use originlm_contracts::AdapterDescriptor;
pub fn descriptors() -> Vec<AdapterDescriptor> {
    let mut items = Vec::new();
    items.push(AdapterDescriptor {
        id: "codex_managed".into(),
        name: "ChatGPT plan (Codex)".into(),
        integration: "native_client".into(),
        passive_discovery: true,
        metadata: true,
        text_sessions: true,
        native_tool_approval: true,
        cancellation: true,
        persistent_resumption: true,
        qualification: "Codex app-server 0.157+ with host-managed SIWC token".into(),
        limitations: vec![
            "Model catalog does not prove model entitlement".into(),
            "Account quota is not reported by this route".into(),
        ],
    });
    for def in originlm_discovery::definitions() {
        let metadata = matches!(
            def.id,
            "codex"
                | "claude_code"
                | "grok"
                | "gemini_cli"
                | "antigravity"
                | "cursor"
                | "ollama"
                | "lmstudio"
                | "opencode"
                | "muse"
        );
        let execution = matches!(
            def.id,
            "codex"
                | "claude_code"
                | "gemini_cli"
                | "antigravity"
                | "grok"
                | "ollama"
                | "lmstudio"
                | "opencode"
                | "muse"
        );
        items.push(AdapterDescriptor {
            id: def.id.into(),
            name: def.name.into(),
            integration: if matches!(def.id, "ollama" | "lmstudio" | "opencode") {
                "local_http"
            } else {
                "native_client"
            }
            .into(),
            passive_discovery: true,
            metadata,
            text_sessions: execution,
            native_tool_approval: matches!(
                def.id,
                "codex" | "gemini_cli" | "antigravity" | "grok" | "muse"
            ),
            cancellation: execution,
            persistent_resumption: matches!(def.id, "gemini_cli" | "antigravity" | "grok" | "muse"),
            qualification: "See per-operation qualification reports; no account capability implied"
                .into(),
            limitations: if metadata {
                vec!["Plan and account quota depend on what the native interface reports".into()]
            } else {
                vec![
                    "Passive detection only; native authenticated integration not yet qualified"
                        .into(),
                ]
            },
        });
    }
    for id in [
        "openai_api",
        "anthropic_api",
        "gemini_api",
        "xai_api",
        "compatible_api",
    ] {
        items.push(AdapterDescriptor {
            id: id.into(),
            name: id.into(),
            integration: "explicit_api".into(),
            passive_discovery: false,
            metadata: true,
            text_sessions: true,
            native_tool_approval: false,
            cancellation: true,
            persistent_resumption: false,
            qualification: "Local protocol fixtures; remote account checks are opt-in".into(),
            limitations: vec![
                "Text-only routes; no automatic subscription billing transfer".into(),
                "Per-account plans and allowances are not reported by ordinary model endpoints"
                    .into(),
            ],
        });
    }
    items
}

#[cfg(test)]
mod tests {
    #[test]
    fn muse_is_a_native_client_with_mediated_approvals_and_resumption() {
        let muse = super::descriptors()
            .into_iter()
            .find(|d| d.id == "muse")
            .unwrap();
        assert_eq!(muse.name, "Muse Code");
        assert_eq!(muse.integration, "native_client");
        assert!(muse.passive_discovery && muse.metadata && muse.text_sessions);
        assert!(muse.native_tool_approval && muse.cancellation && muse.persistent_resumption);
    }

    #[test]
    fn fx_stays_passive_inventory() {
        let fx = super::descriptors()
            .into_iter()
            .find(|d| d.id == "fx")
            .unwrap();
        assert_eq!(fx.name, "fx");
        assert!(fx.passive_discovery);
        assert!(!fx.metadata && !fx.text_sessions && !fx.cancellation);
    }
}
