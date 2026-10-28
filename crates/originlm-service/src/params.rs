use originlm_contracts::{DomainError, ErrorCode, Result};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub(crate) fn object(params: &Value) -> Result<&Map<String, Value>> {
    object_with_limit(params, 65_536)
}
pub(crate) fn object_with_limit(params: &Value, max_bytes: usize) -> Result<&Map<String, Value>> {
    if serde_json::to_vec(params).map_err(|_| invalid())?.len() > max_bytes {
        return Err(invalid());
    }
    params.as_object().ok_or_else(invalid)
}
pub(crate) fn allow(params: &Value, fields: &[&str]) -> Result<()> {
    if object(params)?
        .keys()
        .any(|key| !fields.contains(&key.as_str()))
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn text<'a>(params: &'a Value, key: &str) -> Result<Option<&'a str>> {
    params
        .get(key)
        .map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
                .ok_or_else(invalid)
        })
        .transpose()
}
pub(crate) fn boolean(params: &Value, key: &str) -> Result<bool> {
    params
        .get(key)
        .map(|v| v.as_bool().ok_or_else(invalid))
        .transpose()
        .map(|v| v.unwrap_or(false))
}
pub(crate) fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|_| invalid())
}
pub(crate) fn invalid() -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, "Request parameters are invalid")
}
