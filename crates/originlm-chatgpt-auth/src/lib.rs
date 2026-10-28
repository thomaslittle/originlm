//! Host-owned Sign in with ChatGPT for an open-source native client.
//! Nothing reads credentials or contacts the network until a host calls an auth method.
mod oauth;
mod store;

pub use oauth::{AuthManager, AuthStart, AuthState, AuthStatus, Credentials};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{code}: {message}")]
pub struct AuthError {
    pub code: &'static str,
    pub message: &'static str,
}

pub type Result<T> = std::result::Result<T, AuthError>;

fn error(code: &'static str, message: &'static str) -> AuthError {
    AuthError { code, message }
}

fn has_plan_scope(scope: &str) -> bool {
    scope
        .split_ascii_whitespace()
        .any(|s| s == "chatgpt.tokens.use.direct")
}

#[cfg(test)]
mod tests {
    #[test]
    fn scope_gate_requires_exact_plan_permission() {
        assert!(crate::has_plan_scope("openid chatgpt.tokens.use.direct"));
        assert!(!crate::has_plan_scope("openid chatgpt.tokens.use.directly"));
    }
    #[tokio::test]
    async fn constructor_does_not_read_credential_storage() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("chatgpt-auth.json"), b"not-json").unwrap();
        let manager =
            crate::AuthManager::new(root.path().to_path_buf(), "example-app".into()).unwrap();
        assert_eq!(
            manager.status().await.unwrap_err().code,
            "STORAGE_UNAVAILABLE"
        );
    }
}
