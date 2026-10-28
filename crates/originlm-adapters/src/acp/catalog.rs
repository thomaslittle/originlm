use super::{AcpCatalog, config, evidence, invalid, text};
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub fn parse_catalog(
    provider: &str,
    route: &str,
    init: &Value,
    session: &Value,
) -> Result<AcpCatalog> {
    if init["protocolVersion"].as_u64() != Some(1) {
        return Err(invalid());
    }
    let session_id = text(&session["sessionId"])?;
    let config_options = match session.get("configOptions") {
        None => vec![],
        Some(v) => config::parse(provider, v)?,
    };
    let select = config_options
        .iter()
        .find(|d| d.category.as_deref() == Some("model") || d.id == "model");
    let (entries, current) = if let Some(descriptor) = select {
        let OptionControl::Select { choices, .. } = &descriptor.control else {
            return Err(invalid());
        };
        (
            choices
                .iter()
                .map(|c| (c.value.clone(), c.label.clone()))
                .collect::<Vec<_>>(),
            descriptor.current_value.as_ref().map(text).transpose()?,
        )
    } else if session.get("configOptions").is_some() {
        (vec![], None)
    } else {
        let model_state = &session["models"];
        let values = match model_state.get("availableModels") {
            None => &[][..],
            Some(v) => v
                .as_array()
                .filter(|v| v.len() <= 2048)
                .ok_or_else(invalid)?
                .as_slice(),
        };
        let mut entries = vec![];
        for model in values {
            entries.push((text(&model["modelId"])?, text(&model["name"])?));
        }
        (
            entries,
            model_state.get("currentModelId").map(text).transpose()?,
        )
    };
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    let capabilities = &init["agentCapabilities"];
    for (native_id, name) in entries {
        if !seen.insert(native_id.clone()) {
            return Err(invalid());
        }
        let is_default = current.as_ref() == Some(&native_id);
        // Session options are specific to the current model. Re-query after changing models.
        let options = if is_default {
            config_options
                .iter()
                .filter(|d| Some(d.id.as_str()) != select.map(|d| d.id.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let signature=options.iter().map(|d|json!({"id":d.id,"control":d.control,"category":d.category,"permission":d.permission_sensitive,"billing":d.billing_sensitive})).collect::<Vec<_>>();
        let revision = stable_id(
            "acp-model",
            &[
                provider,
                route,
                &native_id,
                &serde_json::to_string(&signature).map_err(|_| invalid())?,
            ],
        );
        let mut caps = vec![Capability {
            id: "generation.text".into(),
            support: Support::Supported,
            origin: CapabilityOrigin::Model,
            evidence: Observation::reported(
                "ACP text prompt/response surface".into(),
                evidence(provider),
            ),
            conditions: vec![],
        }];
        for modality in ["image", "audio"] {
            let supported = capabilities["promptCapabilities"][modality]
                .as_bool()
                .unwrap_or(false);
            caps.push(Capability {
                id: format!("input.{modality}"),
                support: if supported {
                    Support::Supported
                } else {
                    Support::Unsupported
                },
                origin: CapabilityOrigin::Runtime,
                evidence: Observation::reported(
                    "ACP initialize promptCapabilities".into(),
                    evidence(provider),
                ),
                conditions: vec![],
            });
        }
        models.push(Model {
            id: route_model_key(route, &native_id),
            route_id: route.into(),
            native_id,
            name,
            revision,
            is_default,
            capabilities: caps,
            options,
            context_window: Observation::unknown("NOT_REPORTED_BY_ACP"),
        });
    }
    Ok(AcpCatalog {
        session_id,
        models,
        config_options,
        load_session: capabilities["loadSession"].as_bool().unwrap_or(false),
        image_prompt: capabilities["promptCapabilities"]["image"]
            .as_bool()
            .unwrap_or(false),
    })
}
