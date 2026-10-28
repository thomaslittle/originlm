//! Host-provided MCP servers attached to a native session.
//!
//! A host runs a loopback MCP server and names the tools the agent may use.
//! Adapters attach it through each client's own MCP configuration, keep the
//! client's built-in tools disabled by default, and use [`HostTools`] to tell
//! an allowed host tool call from any other tool call. Explicit native-policy
//! sessions may also use their native client's built-in tools.
use originlm_contracts::{DomainError, ErrorCode, HostMcpServer};
use std::collections::{BTreeMap, BTreeSet};

/// The allowed `(server, tool)` pairs of one session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostTools {
    servers: BTreeMap<String, BTreeSet<String>>,
}
impl HostTools {
    pub fn new(servers: &[HostMcpServer]) -> Self {
        Self {
            servers: servers
                .iter()
                .map(|server| {
                    (
                        server.name.clone(),
                        server.allowed_tools.iter().cloned().collect(),
                    )
                })
                .collect(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
    pub fn server_names(&self) -> impl Iterator<Item = &str> {
        self.servers.keys().map(String::as_str)
    }
    pub fn allows(&self, server: &str, tool: &str) -> bool {
        self.servers
            .get(server)
            .is_some_and(|tools| tools.contains(tool))
    }
    /// Claude Code's qualified names, `mcp__<server>__<tool>`.
    pub fn qualified_names(&self) -> Vec<String> {
        self.servers
            .iter()
            .flat_map(|(server, tools)| {
                tools
                    .iter()
                    .map(move |tool| format!("mcp__{server}__{tool}"))
            })
            .collect()
    }
    /// Whether a qualified `mcp__<server>__<tool>` name is an allowed host tool.
    pub fn allows_qualified(&self, name: &str) -> bool {
        let Some(rest) = name.strip_prefix("mcp__") else {
            return false;
        };
        self.servers.iter().any(|(server, tools)| {
            rest.strip_prefix(server.as_str())
                .and_then(|tail| tail.strip_prefix("__"))
                .is_some_and(|tool| tools.contains(tool))
        })
    }
    /// Resolve a tool name reported by a client that does not say which MCP
    /// server it came from. Accepts `mcp__<server>__<tool>`, `<server>__<tool>`,
    /// `<server>/<tool>`, `<server>.<tool>`, `<server>:<tool>`, or a bare tool name
    /// that exactly one server allows. Returns the qualified name.
    pub fn resolve(&self, name: &str) -> Option<String> {
        let name = name.trim();
        if self.allows_qualified(name) {
            return Some(name.into());
        }
        for (server, tools) in &self.servers {
            for separator in ["__", "/", ".", ":"] {
                if let Some(tool) = name
                    .strip_prefix(server.as_str())
                    .and_then(|tail| tail.strip_prefix(separator))
                    && tools.contains(tool)
                {
                    return Some(format!("mcp__{server}__{tool}"));
                }
            }
        }
        let mut owners = self
            .servers
            .iter()
            .filter(|(_, tools)| tools.contains(name));
        match (owners.next(), owners.next()) {
            (Some((server, _)), None) => Some(format!("mcp__{server}__{name}")),
            _ => None,
        }
    }
}

/// Typed refusal for integrations that cannot attach host MCP servers.
pub fn unsupported() -> DomainError {
    DomainError::new(
        ErrorCode::CapabilityUnsupported,
        "Selected integration cannot attach host MCP servers",
    )
}

/// Typed failure for a tool call that is not an allowed host tool.
pub fn native_tool_refused() -> DomainError {
    DomainError::new(
        ErrorCode::IntegrationRestricted,
        "Native tool use is not enabled",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tools() -> HostTools {
        HostTools::new(&[
            HostMcpServer {
                name: "design".into(),
                url: "http://127.0.0.1:4000/mcp".into(),
                bearer_token: "t".repeat(32),
                allowed_tools: vec!["echo_color".into(), "set_fill".into()],
            },
            HostMcpServer {
                name: "design_extra".into(),
                url: "http://127.0.0.1:4001/mcp".into(),
                bearer_token: "t".repeat(32),
                allowed_tools: vec!["set_fill".into()],
            },
        ])
    }
    #[test]
    fn qualified_names_are_exact() {
        let tools = tools();
        assert!(tools.allows_qualified("mcp__design__echo_color"));
        assert!(tools.allows_qualified("mcp__design_extra__set_fill"));
        assert!(!tools.allows_qualified("mcp__design_extra__echo_color"));
        assert!(!tools.allows_qualified("mcp__design__bash"));
        assert!(!tools.allows_qualified("Bash"));
        assert!(!tools.allows_qualified("mcp__other__echo_color"));
        assert_eq!(
            tools.qualified_names(),
            [
                "mcp__design__echo_color",
                "mcp__design__set_fill",
                "mcp__design_extra__set_fill"
            ]
        );
    }
    #[test]
    fn resolve_accepts_server_forms_and_unambiguous_bare_names() {
        let tools = tools();
        assert_eq!(
            tools.resolve("design/echo_color").as_deref(),
            Some("mcp__design__echo_color")
        );
        assert_eq!(
            tools.resolve("echo_color").as_deref(),
            Some("mcp__design__echo_color")
        );
        // Two servers allow set_fill: a bare name is ambiguous.
        assert_eq!(tools.resolve("set_fill"), None);
        assert_eq!(tools.resolve("read_file"), None);
        assert_eq!(tools.resolve("Read"), None);
    }
}
