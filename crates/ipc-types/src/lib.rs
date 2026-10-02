use meridian_shared_types::TypedError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request<'a> {
    pub request_id: String,
    pub method: &'a str,
    pub params: Value,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reply {
    pub request_id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<TypedError>,
}
pub fn decode_reply(line: &[u8], expected_id: &str) -> Result<Value, TypedError> {
    let protocol = || {
        TypedError::new(
            "PROTOCOL_INVALID",
            "The owned runtime returned an invalid response.",
            "Reconnect the developer session.",
            true,
        )
    };
    if line.len() > MAX_LINE_BYTES {
        return Err(protocol());
    }
    let reply: Reply = serde_json::from_slice(line).map_err(|_| protocol())?;
    if reply.request_id != expected_id {
        return Err(protocol());
    }
    match (reply.ok, reply.result, reply.error) {
        (true, Some(value), None) => Ok(value),
        (false, None, Some(error)) => Err(error),
        _ => Err(protocol()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_crossed_request_ids() {
        assert!(decode_reply(br#"{"requestId":"old","ok":true,"result":{}}"#, "new").is_err());
    }
    #[test]
    fn refuses_ambiguous_success() {
        assert!(decode_reply(br#"{"requestId":"a","ok":true,"result":{},"error":{"code":"X","message":"x","recovery":"x","retryable":false}}"#,"a").is_err());
    }
    #[test]
    fn preserves_real_clear_failure() {
        assert_eq!(decode_reply(br#"{"requestId":"a","ok":false,"error":{"code":"RESTORE_FAILED","message":"x","recovery":"retry","retryable":true}}"#,"a").unwrap_err().code,"RESTORE_FAILED");
    }
}
