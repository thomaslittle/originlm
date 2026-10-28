use super::{HttpKind, parse_catalog};
use crate::http::Endpoint;
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Provider catalog pagination is invalid",
    )
}
pub async fn probe(
    kind: HttpKind,
    endpoint: Endpoint,
    connection: String,
    route: String,
    text_models: Vec<String>,
) -> Result<ConnectionMetadata> {
    let endpoint = if kind == HttpKind::Anthropic {
        endpoint.anthropic_version()
    } else {
        endpoint
    };
    let path = match kind {
        HttpKind::Ollama => "api/tags",
        HttpKind::Lmstudio => "api/v1/models",
        _ => "models",
    };
    let mut raw = endpoint.json(path, None).await?;
    let mut details = BTreeMap::new();
    let mut truncated = false;
    if kind == HttpKind::Ollama {
        let list = raw["models"].as_array().ok_or_else(invalid)?;
        // Bounded metadata enrichment; no model loading or generation endpoint is called.
        for model in list.iter().take(32) {
            let name = model["name"].as_str().ok_or_else(invalid)?;
            match endpoint.json("api/show", Some(json!({"model":name}))).await {
                Ok(detail) => {
                    details.insert(name.to_owned(), detail);
                }
                Err(_) => truncated = true,
            }
        }
        truncated |= list.len() > 32;
    } else if matches!(kind, HttpKind::Anthropic | HttpKind::Gemini) {
        let key = if kind == HttpKind::Gemini {
            "models"
        } else {
            "data"
        };
        let mut all = raw[key].as_array().ok_or_else(invalid)?.clone();
        let mut seen = BTreeSet::new();
        for page in 0..16 {
            let next = if kind == HttpKind::Gemini {
                raw["nextPageToken"].as_str()
            } else if raw["has_more"] == true {
                raw["last_id"].as_str()
            } else {
                None
            };
            let Some(next) = next.filter(|s| !s.is_empty()) else {
                break;
            };
            if !seen.insert(next.to_owned()) {
                return Err(invalid());
            }
            if page == 15 || all.len() >= 4096 {
                truncated = true;
                break;
            }
            let key = if kind == HttpKind::Gemini {
                "pageToken"
            } else {
                "after_id"
            };
            raw = endpoint.json_query(path, &[(key, next)], None).await?;
            all.extend(
                raw[if kind == HttpKind::Gemini {
                    "models"
                } else {
                    "data"
                }]
                .as_array()
                .ok_or_else(invalid)?
                .clone(),
            );
        }
        raw[key] = Value::Array(all);
    }
    let mut result = parse_catalog(kind, &connection, &route, raw, &details, &text_models)?;
    if truncated {
        result.diagnostics.push("CATALOG_ENRICHMENT_PARTIAL".into());
    }
    Ok(result)
}
