use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CursorPayload {
    pub version: u8,
    pub operation: String,
    pub context_id: String,
    pub auth_fingerprint: String,
    pub filters_fingerprint: String,
    pub after: CursorKey,
    pub expires_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CursorKey {
    pub patient_rank: u8,
    pub resource_type: String,
    pub id: String,
    pub version_id: i64,
}

pub fn encode(payload: &CursorPayload, secret: &[u8]) -> Result<String, AppError> {
    let body = serde_json::to_vec(payload)
        .map_err(|error| AppError::Internal(format!("failed to encode cursor: {error}")))?;
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|_| AppError::Internal("invalid cursor signing key".to_owned()))?;
    mac.update(&body);
    let signature = mac.finalize().into_bytes();
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(body),
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

pub fn decode(value: &str, secret: &[u8]) -> Result<CursorPayload, AppError> {
    let (body, signature) = value
        .split_once('.')
        .ok_or_else(|| AppError::BadRequest("invalid paging cursor".to_owned()))?;
    let body = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| AppError::BadRequest("invalid paging cursor".to_owned()))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| AppError::BadRequest("invalid paging cursor".to_owned()))?;
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|_| AppError::Internal("invalid cursor signing key".to_owned()))?;
    mac.update(&body);
    mac.verify_slice(&signature)
        .map_err(|_| AppError::BadRequest("invalid paging cursor signature".to_owned()))?;
    let payload: CursorPayload = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("invalid paging cursor".to_owned()))?;
    if payload.version != 1 || payload.expires_at < chrono::Utc::now().timestamp() {
        return Err(AppError::BadRequest(
            "paging cursor is expired or unsupported".to_owned(),
        ));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::{CursorKey, CursorPayload, decode, encode};

    fn payload(expires_at: i64) -> CursorPayload {
        CursorPayload {
            version: 1,
            operation: "patient-everything".to_owned(),
            context_id: "p1".to_owned(),
            auth_fingerprint: "auth".to_owned(),
            filters_fingerprint: "filters".to_owned(),
            after: CursorKey {
                patient_rank: 1,
                resource_type: "Observation".to_owned(),
                id: "o1".to_owned(),
                version_id: 1,
            },
            expires_at,
        }
    }

    #[test]
    fn cursor_round_trips_and_rejects_tampering_or_expiry() {
        let secret = b"cursor-test-secret-at-least-32-bytes";
        let value = encode(&payload(chrono::Utc::now().timestamp() + 60), secret).unwrap();
        assert_eq!(decode(&value, secret).unwrap().context_id, "p1");
        assert!(decode(&(value + "x"), secret).is_err());

        let expired = encode(&payload(chrono::Utc::now().timestamp() - 1), secret).unwrap();
        assert!(decode(&expired, secret).is_err());
    }
}
