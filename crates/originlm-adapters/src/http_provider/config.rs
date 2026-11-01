use super::HttpKind;
use crate::http::{Authentication, Endpoint};
use originlm_contracts::*;
use originlm_discovery::{LocatedClient, ScanResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndpointConfig {
    pub id: String,
    pub label: String,
    pub kind: HttpKind,
    pub base_url: String,
    #[serde(default)]
    pub key_env: Option<String>,
    #[serde(default)]
    pub text_models: Vec<String>,
    #[serde(default)]
    pub provider_billing: BTreeMap<String, BillingMode>,
}
#[derive(Clone, Default)]
pub struct Registry {
    pub entries: BTreeMap<String, EndpointConfig>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    endpoints: Vec<EndpointConfig>,
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Invalid endpoint configuration; inline credentials are not accepted",
    )
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
impl Registry {
    pub fn parse(input: &str, scope: &str) -> Result<Self> {
        if input.len() > 65536 || scope.is_empty() {
            return Err(invalid());
        }
        let document: Document = serde_json::from_str(input).map_err(|_| invalid())?;
        if document.endpoints.len() > 64 {
            return Err(invalid());
        }
        let mut entries = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for config in document.endpoints {
            if !identifier(&config.id)
                || !seen.insert(config.id.clone())
                || config.label.is_empty()
                || config.label.len() > 256
                || config.label.chars().any(char::is_control)
                || config.text_models.len() > 256
                || config
                    .text_models
                    .iter()
                    .any(|s| s.is_empty() || s.len() > 512 || s.chars().any(char::is_control))
                || config
                    .key_env
                    .as_ref()
                    .is_some_and(|s| !identifier(s) || s.as_bytes()[0].is_ascii_digit())
            {
                return Err(invalid());
            }
            if config.provider_billing.len() > 128
                || config.provider_billing.keys().any(|id| !identifier(id))
            {
                return Err(invalid());
            }
            Endpoint::new(
                &config.base_url,
                Authentication::None,
                config.kind.local() || config.kind == HttpKind::Opencode,
            )?;
            let id = stable_id(
                "connection",
                &[scope, &config.id, config.kind.id(), &config.base_url],
            );
            entries.insert(id, config);
        }
        Ok(Self { entries })
    }
    pub fn for_connection(&self, connection: &str, provider: &str) -> Option<EndpointConfig> {
        self.entries.get(connection).cloned().or_else(|| {
            let (kind, url) = match provider {
                "ollama" => (HttpKind::Ollama, "http://127.0.0.1:11434/"),
                "lmstudio" => (HttpKind::Lmstudio, "http://127.0.0.1:1234/"),
                // A discovered OpenCode install runs a kit-owned, password-protected
                // `opencode serve` (see `opencode::server`); only an explicit
                // endpoint entry points the kit at a user-run server.
                _ => return None,
            };
            Some(EndpointConfig {
                id: connection.into(),
                label: provider.into(),
                kind,
                base_url: url.into(),
                key_env: None,
                text_models: vec![],
                provider_billing: BTreeMap::new(),
            })
        })
    }
    pub fn inject(&self, scan: &mut ScanResult) {
        for (id, config) in &self.entries {
            let provider = config.kind.id();
            let installation = stable_id("installation", &[id]);
            let profile = stable_id("profile", &[id]);
            if !scan.snapshot.providers.iter().any(|p| p.id == provider) {
                scan.snapshot.providers.push(Provider {
                    id: provider.into(),
                    name: provider.into(),
                    kind: "registered_endpoint".into(),
                });
            }
            scan.snapshot.installations.push(Installation {
                id: installation.clone(),
                provider_id: provider.into(),
                environment_id: "explicit-http-registration".into(),
                location_hint: "Registered endpoint; not contacted".into(),
                version: Observation::unknown("NOT_PROBED"),
            });
            scan.snapshot.profiles.push(Profile {
                id: profile.clone(),
                installation_id: installation.clone(),
                label: config.label.clone(),
            });
            scan.snapshot.connections.push(Connection {
                id: id.clone(),
                provider_id: provider.into(),
                installation_id: installation,
                profile_id: profile,
                display_name: config.label.clone(),
                account_id: None,
                auth: Observation::unknown("NOT_PROBED"),
                plan: Observation::unknown("NOT_REPORTED"),
                status: "configured".into(),
                diagnostics: vec!["EXPLICIT_ENDPOINT_REGISTRATION".into()],
            });
            scan.snapshot.routes.push(AccessRoute {
                id: stable_id("route", &[id]),
                connection_id: id.clone(),
                provider_id: provider.into(),
                display_name: config.label.clone(),
                billing: Observation::unknown("NOT_PROBED"),
                capabilities: vec![],
                catalog_revision: "unprobed".into(),
            });
            scan.locations.insert(
                id.clone(),
                LocatedClient {
                    executable_identity: None,
                    provider_id: provider.into(),
                    profile_home: PathBuf::new(),
                    executable: None,
                    scope: None,
                },
            );
        }
        scan.snapshot.revision = stable_id(
            "inventory",
            &[
                &scan.snapshot.revision,
                &self.entries.keys().cloned().collect::<Vec<_>>().join("|"),
            ],
        );
    }
}
impl EndpointConfig {
    /// Callers explicitly supply a credential after authorizing the selected endpoint.
    /// This module never reads another application's credentials or the process environment.
    pub fn endpoint_with_credential(&self, credential: Option<String>) -> Result<Endpoint> {
        let auth = match credential {
            Some(value) => match self.kind {
                HttpKind::Opencode => Authentication::Basic("opencode".into(), value),
                HttpKind::Anthropic => Authentication::Header("x-api-key".into(), value),
                HttpKind::Gemini => Authentication::Header("x-goog-api-key".into(), value),
                _ => Authentication::Bearer(value),
            },
            None if self.key_env.is_none()
                && (self.kind.local()
                    || matches!(self.kind, HttpKind::Compatible | HttpKind::Opencode)) =>
            {
                Authentication::None
            }
            None => {
                return Err(DomainError::new(
                    ErrorCode::AuthRequired,
                    "Explicit endpoint credential required",
                ));
            }
        };
        Endpoint::new(
            &self.base_url,
            auth,
            self.kind.local() || self.kind == HttpKind::Opencode,
        )
    }
    pub fn endpoint(&self) -> Result<Endpoint> {
        self.endpoint_with_credential(None)
    }
}
