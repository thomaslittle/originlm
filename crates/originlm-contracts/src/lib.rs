//! Canonical, serializable contracts. Importing this crate performs no I/O.
mod adapters;
pub use adapters::*;
mod api;
pub use api::*;
mod artifacts;
mod catalog;
pub use catalog::*;
mod errors;
pub use artifacts::*;
mod costs;
mod detection;
mod events;
mod evidence;
mod health;
mod identity;
mod intelligence;
mod local_models;
mod metadata;
mod options;
mod provider_updates;
mod usage;
pub use costs::*;
pub use detection::*;
pub use errors::*;
pub use events::*;
pub use evidence::*;
pub use health::*;
pub use identity::*;
pub use intelligence::*;
pub use local_models::*;
pub use metadata::*;
pub use options::*;
pub use provider_updates::*;
use schemars::JsonSchema;
use serde_json::Value;
pub use usage::*;

#[derive(JsonSchema)]
#[allow(dead_code)]
pub struct PublicContracts {
    snapshot: Snapshot,
    connection: Connection,
    route: AccessRoute,
    model: Model,
    intelligence: IntelligenceCatalog,
    intelligence_model: IntelligenceModel,
    option: OptionDescriptor,
    pool: QuotaPool,
    session: Session,
    event: Event,
    error: DomainError,
    method_inputs: MethodInputs,
    method_outputs: MethodOutputs,
    metadata: ConnectionMetadata,
    issue: Issue,
    health: HealthSummary,
    inventory_change: InventoryChange,
}
pub fn schema_document() -> Value {
    serde_json::to_value(schemars::schema_for!(PublicContracts)).expect("schema serialization")
}
pub fn validate_observation(value: Value) -> std::result::Result<Value, String> {
    let parsed: Observation<Value> = serde_json::from_value(value).map_err(|e| e.to_string())?;
    serde_json::to_value(parsed).map_err(|e| e.to_string())
}
pub fn validate_option(descriptor: Value, value: Value) -> std::result::Result<(), String> {
    let parsed: OptionDescriptor = serde_json::from_value(descriptor).map_err(|e| e.to_string())?;
    parsed.validate(&value)
}
pub fn normalize_usage(value: Value) -> std::result::Result<Value, String> {
    let input: UsageInput = serde_json::from_value(value).map_err(|e| e.to_string())?;
    serde_json::to_value(UsageReading::try_from(input)?).map_err(|e| e.to_string())
}
