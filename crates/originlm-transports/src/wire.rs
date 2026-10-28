use originlm_contracts::{DomainError, ErrorCode, Result};
use serde_json::{Value, json};

pub(crate) struct Request {
    pub id: Value,
    pub method: String,
    pub params: Value,
}
pub(crate) fn parse(line: &[u8]) -> std::result::Result<Option<Request>, Value> {
    let value: Value = serde_json::from_slice(line).map_err(|_| invalid(-32700))?;
    let map = value.as_object().ok_or_else(|| invalid(-32600))?;
    if map
        .keys()
        .any(|k| !["jsonrpc", "id", "method", "params"].contains(&k.as_str()))
        || value["jsonrpc"] != "2.0"
    {
        return Err(invalid(-32600));
    }
    let method = value["method"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
        .ok_or_else(|| invalid(-32600))?;
    let Some(id) = value.get("id") else {
        return Ok(None);
    };
    let valid_id = id.is_null()
        || id
            .as_i64()
            .is_some_and(|n| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n))
        || id
            .as_str()
            .is_some_and(|s| s.len() <= 128 && !s.chars().any(char::is_control));
    if !valid_id {
        return Err(invalid(-32600));
    }
    Ok(Some(Request {
        id: id.clone(),
        method: method.into(),
        params: value.get("params").cloned().unwrap_or_else(|| json!({})),
    }))
}
pub(crate) fn reply(id: Value, outcome: Result<Value>) -> Value {
    match outcome {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(error) => {
            let code = match error.code {
                ErrorCode::InvalidRequest => -32602,
                ErrorCode::CapabilityUnsupported => -32601,
                _ => -32000,
            };
            json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":"OriginLM request failed","data":error}})
        }
    }
}
pub(crate) fn invalid(code: i32) -> Value {
    json!({"jsonrpc":"2.0","id":null,"error":{"code":code,"message":"Invalid JSON-RPC request","data":{"code":"INVALID_REQUEST","message":"Invalid JSON-RPC request","retryable":false}}})
}
pub(crate) fn error(code: ErrorCode) -> DomainError {
    DomainError::new(code, "Native transport cannot complete the operation")
}
