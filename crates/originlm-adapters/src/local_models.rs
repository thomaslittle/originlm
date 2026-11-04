//! On-disk model inventory for local runtimes that work without a server.
//!
//! This is an explicit read path used by `originlm models`. It reports exactly
//! what the provider's own manifest/file layout contains: native ids, names,
//! byte sizes, and a coarse format label. It does not guess quantization,
//! context window, vision, or text capability from a filename, and it never
//! loads a model or starts a server.

use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const MAX_MODELS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub provider_id: String,
    pub native_id: String,
    pub name: String,
    pub size_bytes: Option<u64>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInventory {
    pub provider_id: String,
    pub roots: Vec<String>,
    pub models: Vec<LocalModel>,
    pub files_skipped: u32,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
}

/// Ollama's managed library, honoring `OLLAMA_MODELS` when supplied by the
/// caller. The default is `~/.ollama/models`.
pub fn read_ollama_models(
    home: &Path,
    models_override: Option<&Path>,
    limit: usize,
) -> LocalModelInventory {
    let root = models_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join(".ollama/models"));
    let mut inventory = LocalModelInventory {
        provider_id: "ollama".into(),
        roots: vec![root.display().to_string()],
        models: Vec::new(),
        files_skipped: 0,
        truncated: false,
        diagnostics: Vec::new(),
    };
    let manifests = root.join("manifests");
    if !manifests.is_dir() {
        inventory
            .diagnostics
            .push("OLLAMA_MANIFEST_ROOT_MISSING".into());
        return inventory;
    }
    let mut seen = BTreeMap::new();
    let mut visited = 0;
    visit(
        &manifests,
        0,
        limit.min(MAX_MODELS),
        &mut inventory,
        &mut visited,
        &mut |path| {
            let relative = match path.strip_prefix(&manifests) {
                Ok(relative) => relative,
                Err(_) => return,
            };
            let parts: Vec<String> = relative
                .components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect();
            if parts.len() < 2 {
                return;
            }
            let tag = parts[parts.len() - 1].clone();
            let model = parts[parts.len() - 2].clone();
            let namespace = parts.get(parts.len().saturating_sub(3)).cloned();
            let native_id = match namespace.as_deref() {
                Some("library") | None => format!("{model}:{tag}"),
                Some(namespace) => format!("{namespace}/{model}:{tag}"),
            };
            let size = fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|value| {
                    value["layers"].as_array().map(|layers| {
                        layers
                            .iter()
                            .filter_map(|layer| layer["size"].as_u64())
                            .sum::<u64>()
                    })
                });
            seen.insert(
                native_id.clone(),
                LocalModel {
                    provider_id: "ollama".into(),
                    native_id: native_id.clone(),
                    name: native_id,
                    size_bytes: size,
                    format: Some("ollama-manifest".into()),
                },
            );
        },
    );
    inventory.models = seen.into_values().collect();
    inventory
}

/// LM Studio's managed library. Both the documented `~/.lmstudio/models` and
/// the older `~/.cache/lm-studio/models` location are searched.
pub fn read_lmstudio_models(home: &Path, limit: usize) -> LocalModelInventory {
    let roots: Vec<PathBuf> = [
        home.join(".lmstudio/models"),
        home.join(".cache/lm-studio/models"),
    ]
    .into_iter()
    .filter(|root| root.is_dir())
    .collect();
    let mut inventory = LocalModelInventory {
        provider_id: "lmstudio".into(),
        roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        models: Vec::new(),
        files_skipped: 0,
        truncated: false,
        diagnostics: Vec::new(),
    };
    if roots.is_empty() {
        inventory
            .diagnostics
            .push("LMSTUDIO_MODEL_ROOT_MISSING".into());
        return inventory;
    }
    let mut seen = BTreeMap::new();
    for root in &roots {
        let mut visited = 0;
        visit(
            root,
            0,
            limit.min(MAX_MODELS),
            &mut inventory,
            &mut visited,
            &mut |path| {
                if !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
                {
                    return;
                }
                let relative = match path.strip_prefix(root) {
                    Ok(relative) => relative,
                    Err(_) => return,
                };
                let native_id = relative
                    .with_extension("")
                    .components()
                    .filter_map(|component| match component {
                        std::path::Component::Normal(part) => {
                            Some(part.to_string_lossy().into_owned())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("/");
                if native_id.is_empty() {
                    return;
                }
                let name = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_else(|| native_id.clone());
                let size = fs::metadata(path).ok().map(|metadata| metadata.len());
                seen.insert(
                    native_id.clone(),
                    LocalModel {
                        provider_id: "lmstudio".into(),
                        native_id,
                        name,
                        size_bytes: size,
                        format: Some("gguf".into()),
                    },
                );
            },
        );
    }
    inventory.models = seen.into_values().collect();
    inventory
}

fn visit(
    root: &Path,
    depth: usize,
    limit: usize,
    inventory: &mut LocalModelInventory,
    visited: &mut usize,
    callback: &mut dyn FnMut(&Path),
) {
    if depth > 8 || *visited >= limit {
        inventory.truncated = true;
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        inventory
            .diagnostics
            .push("MODEL_DIRECTORY_UNREADABLE".into());
        return;
    };
    for entry in entries.flatten() {
        if *visited >= limit {
            inventory.truncated = true;
            return;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            inventory
                .diagnostics
                .push("MODEL_ENTRY_SYMLINK_SKIPPED".into());
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            visit(&path, depth + 1, limit, inventory, visited, callback);
        } else if file_type.is_file() {
            *visited = visited.saturating_add(1);
            callback(&path);
        } else {
            inventory.files_skipped = inventory.files_skipped.saturating_add(1);
        }
    }
    inventory.diagnostics.sort();
    inventory.diagnostics.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(path).unwrap();
        file.write_all(bytes).unwrap();
    }

    #[test]
    fn ollama_manifests_become_native_names_and_sizes() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        write(
            &home.join(".ollama/models/manifests/registry.ollama.ai/library/llama3.2/latest"),
            br#"{"layers":[{"size":100},{"size":25}]}"#,
        );
        let inventory = read_ollama_models(home, None, 100);
        assert_eq!(inventory.models.len(), 1);
        assert_eq!(inventory.models[0].native_id, "llama3.2:latest");
        assert_eq!(inventory.models[0].size_bytes, Some(125));
    }

    #[test]
    fn lmstudio_gguf_files_keep_their_publisher_relative_id() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        write(
            &home.join(".lmstudio/models/infra-ai/ocelot-v1/ocelot-q4_0.gguf"),
            b"gguf",
        );
        let inventory = read_lmstudio_models(home, 100);
        assert_eq!(inventory.models.len(), 1);
        assert_eq!(
            inventory.models[0].native_id,
            "infra-ai/ocelot-v1/ocelot-q4_0"
        );
        assert_eq!(inventory.models[0].format.as_deref(), Some("gguf"));
    }

    #[test]
    fn missing_roots_are_explained_without_fabricating_models() {
        let root = tempfile::tempdir().unwrap();
        for inventory in [
            read_ollama_models(root.path(), None, 100),
            read_lmstudio_models(root.path(), 100),
        ] {
            assert!(inventory.models.is_empty());
            assert!(!inventory.diagnostics.is_empty());
        }
    }

    #[test]
    fn malformed_ollama_manifests_do_not_invent_a_size() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        write(
            &home.join(".ollama/models/manifests/registry.ollama.ai/library/bad/latest"),
            b"not json",
        );
        let inventory = read_ollama_models(home, None, 100);
        assert_eq!(inventory.models.len(), 1);
        assert_eq!(inventory.models[0].size_bytes, None);
    }
}
