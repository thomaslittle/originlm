use crate::{NativeService, denied, params};
use originlm_contracts::*;
use std::{collections::BTreeSet, path::PathBuf, sync::RwLock};

const MAX_PRICE_BYTES: usize = 131072;

pub(crate) struct Prices {
    values: Vec<ModelPrice>,
    path: Option<PathBuf>,
    error: Option<String>,
}
impl Default for Prices {
    fn default() -> Self {
        Self {
            values: serde_json::from_str(include_str!("../assets/default-prices.json"))
                .expect("bundled price table"),
            path: None,
            error: None,
        }
    }
}
fn validate(prices: &[ModelPrice]) -> Result<()> {
    if prices.len() > 128 {
        return Err(params::invalid());
    }
    let mut seen = BTreeSet::new();
    for price in prices {
        if price.provider_id.is_empty()
            || price.provider_id.len() > 64
            || price.native_model_id.is_empty()
            || price.native_model_id.len() > 256
            || price.source.is_empty()
            || price.source.len() > 1024
            || chrono::NaiveDate::parse_from_str(&price.reviewed_at, "%Y-%m-%d").is_err()
            || !seen.insert((&price.provider_id, &price.native_model_id))
            || price.input_usd_per_million.as_str().len() > 24
            || price.output_usd_per_million.as_str().len() > 24
        {
            return Err(params::invalid());
        }
    }
    Ok(())
}
impl NativeService {
    /// Host-owned price configuration. No network requests or provider secrets.
    pub fn with_cost_prices(mut self, directory: PathBuf) -> Self {
        let path = directory.join("cost-prices.json");
        let mut prices = Prices {
            path: Some(path.clone()),
            ..Prices::default()
        };
        let load = || -> std::result::Result<Option<Vec<ModelPrice>>, ()> {
            let metadata = match path.symlink_metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_) => return Err(()),
            };
            if !metadata.is_file() || metadata.len() > MAX_PRICE_BYTES as u64 {
                return Err(());
            }
            let bytes =
                originlm_platform::read_private_file(&path, MAX_PRICE_BYTES).map_err(|_| ())?;
            let values: Vec<ModelPrice> = serde_json::from_slice(&bytes).map_err(|_| ())?;
            validate(&values).map_err(|_| ())?;
            Ok(Some(values))
        };
        match load() {
            Ok(Some(values)) => prices.values = values,
            Ok(None) => {}
            Err(()) => {
                prices.values.clear();
                prices.error = Some(
                    "Saved prices are unreadable or invalid. Save a reviewed table in Settings."
                        .into(),
                );
            }
        }
        self.cost_prices = RwLock::new(prices);
        self
    }
    pub(crate) fn set_cost_prices(
        &self,
        client: &str,
        prices: Vec<ModelPrice>,
    ) -> Result<Vec<ModelPrice>> {
        if !self.policy(client)?.admin {
            return Err(denied());
        }
        validate(&prices)?;
        let serialized = serde_json::to_vec_pretty(&prices).map_err(|_| params::invalid())?;
        if serialized.len() > MAX_PRICE_BYTES {
            return Err(DomainError::new(
                ErrorCode::FrameTooLarge,
                "Price table exceeds 128 KiB; shorten sources or remove rows",
            ));
        }
        let mut current = self.cost_prices.write().map_err(|_| denied())?;
        if let Some(path) = &current.path {
            let write = || -> std::io::Result<()> {
                use std::io::Write;
                let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
                std::fs::create_dir_all(parent)?;
                let mut nonce = [0u8; 8];
                getrandom::fill(&mut nonce)
                    .map_err(|_| std::io::Error::other("OS randomness unavailable"))?;
                let temporary =
                    parent.join(format!(".cost-prices-{:x}.tmp", u64::from_le_bytes(nonce)));
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let result = (|| {
                    let mut file = options.open(&temporary)?;
                    file.write_all(&serialized)?;
                    file.sync_all()?;
                    drop(file);
                    std::fs::rename(&temporary, path)
                })();
                if result.is_err() {
                    let _ = std::fs::remove_file(temporary);
                }
                result
            };
            write().map_err(|_| {
                DomainError::new(
                    ErrorCode::ProviderUnavailable,
                    "Could not save local prices; check the runtime state directory",
                )
            })?;
        }
        current.values = prices.clone();
        current.error = None;
        Ok(prices)
    }
    pub(crate) fn cost_report(&self, client: &str) -> Result<CostReport> {
        let policy = self.policy(client)?;
        let snapshot = self.projected(client)?;
        let prices = self.cost_prices.read().map_err(|_| denied())?;
        let records =
            self.execution_state
                .runtime
                .cost_usage(if policy.admin { None } else { Some(client) })?
                .into_iter()
                .filter_map(|usage| {
                    let model = snapshot.models.iter().find(|model| {
                        model.id == usage.model_id && model.route_id == usage.route_id
                    })?;
                    let route = snapshot.routes.iter().find(|route| {
                        route.id == usage.route_id && route.connection_id == usage.connection_id
                    })?;
                    let connection = snapshot
                        .connections
                        .iter()
                        .find(|connection| connection.id == route.connection_id)?;
                    let price = prices
                        .values
                        .iter()
                        .find(|price| {
                            price.provider_id == connection.provider_id
                                && price.native_model_id == model.native_id
                        })
                        .cloned();
                    let (amount, basis) = if let Some(reported) = &usage.reported_usd {
                        (Some(reported.clone()), "provider_reported")
                    } else if let (Some(input), Some(output), Some(price)) =
                        (&usage.input_tokens, &usage.output_tokens, &price)
                    {
                        (estimate_cost(input, output, price), "estimated")
                    } else {
                        (None, "unknown")
                    };
                    Some(CostRecord {
                        provider_id: connection.provider_id.clone(),
                        native_model_id: model.native_id.clone(),
                        basis: if amount.is_some() { basis } else { "unknown" }.into(),
                        amount_usd: amount,
                        price,
                        usage,
                    })
                })
                .collect();
        Ok(CostReport { records, prices: prices.values.clone(), scope: "Observed turns in this runtime lifetime; not account-wide spend or a subscription invoice".into(), can_edit_prices: policy.admin, prices_persisted: prices.path.is_some(), storage_error: prices.error.clone() })
    }
}
