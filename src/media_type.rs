//! Media-type parsing, request `Content-Type` validation and `Accept`
//! negotiation for the JSON-only FHIR server.
//!
//! The server accepts two JSON forms for resource and Bundle bodies
//! (`application/fhir+json` and the documented `application/json`
//! compatibility extension), `application/json-patch+json` for JSON Patch,
//! and serves all FHIR representations as `application/fhir+json`.

use axum::http::HeaderMap;

use crate::error::AppError;

/// FHIR JSON representation type used for responses.
pub const FHIR_JSON: &str = "application/fhir+json";
/// Plain JSON compatibility type accepted for request bodies.
pub const JSON: &str = "application/json";
/// JSON Patch representation type.
pub const JSON_PATCH: &str = "application/json-patch+json";
/// Server-supported FHIR major.minor version, used for the `fhirVersion`
/// MIME parameter on request and response representations.
pub const SUPPORTED_FHIR_VERSION: &str = "6.0";

/// The kind of body a handler expects, which determines the set of
/// acceptable `Content-Type` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Resource and Bundle bodies: `application/fhir+json` or `application/json`.
    FhirResource,
    /// JSON Patch bodies: `application/json-patch+json` only.
    JsonPatch,
}

/// A parsed media type with case-insensitive parameters.
#[derive(Debug, Default)]
pub struct MediaType {
    /// The `type/subtype` essence, lowercased.
    pub essence: String,
    pub charset: Option<String>,
    pub fhir_version: Option<String>,
}

impl MediaType {
    /// Parse a media type such as `application/fhir+json; charset=utf-8;
    /// fhirVersion=6.0`. Parameter keys are matched case-insensitively.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("missing media type".to_owned());
        }
        let mut parts = raw.split(';');
        let essence = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        if !essence.contains('/') {
            return Err(format!("malformed media type '{raw}'"));
        }

        let mut mt = MediaType {
            essence,
            charset: None,
            fhir_version: None,
        };
        for param in parts {
            let param = param.trim();
            if param.is_empty() {
                continue;
            }
            let (key, value) = match param.split_once('=') {
                Some((k, v)) => (k.trim().to_ascii_lowercase(), unquote(v.trim())),
                None => (param.to_ascii_lowercase(), String::new()),
            };
            match key.as_str() {
                "charset" => mt.charset = Some(value),
                "fhirversion" => mt.fhir_version = Some(value),
                _ => {} // unknown parameters are ignored
            }
        }
        Ok(mt)
    }
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].to_owned()
    } else {
        value.to_owned()
    }
}

/// Validate the request `Content-Type` against the server's supported set for
/// `kind`. Returns `415` on a missing, malformed or unsupported type, a
/// non-UTF-8 charset, or a conflicting `fhirVersion` parameter.
pub fn validate_request_content_type(headers: &HeaderMap, kind: BodyKind) -> Result<(), AppError> {
    let raw = match headers.get(axum::http::header::CONTENT_TYPE) {
        Some(value) => value.to_str().map_err(|_| {
            AppError::UnsupportedMediaType("malformed Content-Type header".to_owned())
        })?,
        None => {
            return Err(AppError::UnsupportedMediaType(
                "missing Content-Type header".to_owned(),
            ));
        }
    };

    let mt = MediaType::parse(raw).map_err(AppError::UnsupportedMediaType)?;

    // Only UTF-8 is accepted when a charset is supplied.
    if let Some(charset) = &mt.charset
        && !charset.eq_ignore_ascii_case("utf-8")
    {
        return Err(AppError::UnsupportedMediaType(format!(
            "unsupported charset '{charset}'; only utf-8 is accepted"
        )));
    }

    // A fhirVersion parameter must identify the server's supported version.
    if let Some(version) = &mt.fhir_version
        && version != SUPPORTED_FHIR_VERSION
    {
        return Err(AppError::UnsupportedMediaType(format!(
            "unsupported fhirVersion '{version}'; server supports '{SUPPORTED_FHIR_VERSION}'"
        )));
    }

    let allowed: &[&str] = match kind {
        BodyKind::FhirResource => &[FHIR_JSON, JSON],
        BodyKind::JsonPatch => &[JSON_PATCH],
    };

    if allowed.contains(&mt.essence.as_str()) {
        Ok(())
    } else {
        Err(AppError::UnsupportedMediaType(format!(
            "unsupported Content-Type '{}'; expected one of: {}",
            mt.essence,
            allowed.join(", ")
        )))
    }
}

/// Validate the `Accept` header for response negotiation. Returns `406` when
/// no supported representation (or wildcard) is offered, and when a matching
/// range carries a conflicting `fhirVersion` parameter.
pub fn validate_accept(headers: &HeaderMap) -> Result<(), AppError> {
    let Some(accept) = headers.get(axum::http::header::ACCEPT) else {
        return Ok(());
    };
    let raw = accept
        .to_str()
        .map_err(|_| AppError::NotAcceptable("malformed Accept header".to_owned()))?;

    let supported = [FHIR_JSON, JSON];

    for range in raw.split(',') {
        let Ok(mt) = MediaType::parse(range) else {
            continue;
        };
        let matches = mt.essence == "*/*"
            || mt.essence == "application/*"
            || supported.contains(&mt.essence.as_str());
        if !matches {
            continue;
        }
        // A fhirVersion parameter on a matching range must agree with the server.
        if let Some(version) = &mt.fhir_version
            && version != SUPPORTED_FHIR_VERSION
        {
            return Err(AppError::NotAcceptable(format!(
                "conflicting fhirVersion '{version}' in Accept; server supports '{SUPPORTED_FHIR_VERSION}'"
            )));
        }
        return Ok(());
    }

    Err(AppError::NotAcceptable(format!(
        "no supported representation in Accept header; server supports {supported:?}"
    )))
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::{
        BodyKind, MediaType, SUPPORTED_FHIR_VERSION, validate_accept, validate_request_content_type,
    };

    fn headers(entries: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in entries {
            let name = axum::http::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            map.insert(name, v.to_string().parse().unwrap());
        }
        map
    }

    #[test]
    fn parses_essence_and_params_case_insensitively() {
        let mt = MediaType::parse("Application/FHIR+JSON; Charset=UTF-8; fhirVersion=6.0")
            .expect("parse");
        assert_eq!(mt.essence, "application/fhir+json");
        assert_eq!(mt.charset.as_deref(), Some("UTF-8"));
        assert_eq!(mt.fhir_version.as_deref(), Some("6.0"));
    }

    #[test]
    fn parses_quoted_param() {
        let mt = MediaType::parse("application/json; charset=\"utf-8\"").expect("parse");
        assert_eq!(mt.charset.as_deref(), Some("utf-8"));
    }

    #[test]
    fn rejects_malformed_essence() {
        assert!(MediaType::parse("not-a-media-type").is_err());
    }

    #[test]
    fn accepts_fhir_and_plain_json_for_resources() {
        for ct in ["application/fhir+json", "application/json"] {
            let h = headers(&[("content-type", ct)]);
            validate_request_content_type(&h, BodyKind::FhirResource).expect(ct);
        }
    }

    #[test]
    fn accepts_patch_only_for_json_patch() {
        let h = headers(&[("content-type", "application/json-patch+json")]);
        validate_request_content_type(&h, BodyKind::JsonPatch).expect("patch ok");
    }

    #[test]
    fn rejects_plain_json_for_patch() {
        let h = headers(&[("content-type", "application/json")]);
        let err = validate_request_content_type(&h, BodyKind::JsonPatch).expect_err("reject");
        assert!(err.to_string().contains("application/json-patch+json"));
    }

    #[test]
    fn rejects_missing_content_type() {
        let h = HeaderMap::new();
        let err = validate_request_content_type(&h, BodyKind::FhirResource).expect_err("missing");
        assert!(err.to_string().contains("missing Content-Type"));
    }

    #[test]
    fn rejects_unsupported_media_type() {
        let h = headers(&[("content-type", "text/plain")]);
        let err = validate_request_content_type(&h, BodyKind::FhirResource).expect_err("reject");
        assert!(err.to_string().contains("unsupported Content-Type"));
    }

    #[test]
    fn rejects_non_utf8_charset() {
        let h = headers(&[("content-type", "application/fhir+json; charset=iso-8859-1")]);
        let err = validate_request_content_type(&h, BodyKind::FhirResource).expect_err("reject");
        assert!(err.to_string().contains("only utf-8"));
    }

    #[test]
    fn accepts_utf8_charset() {
        let h = headers(&[("content-type", "application/fhir+json; charset=UTF-8")]);
        validate_request_content_type(&h, BodyKind::FhirResource).expect("utf-8 ok");
    }

    #[test]
    fn rejects_conflicting_fhir_version() {
        let h = headers(&[("content-type", "application/fhir+json; fhirVersion=4.0")]);
        let err = validate_request_content_type(&h, BodyKind::FhirResource).expect_err("reject");
        assert!(err.to_string().contains("fhirVersion"));
    }

    #[test]
    fn accepts_supported_fhir_version() {
        let h = headers(&[(
            "content-type",
            &format!("application/fhir+json; fhirVersion={SUPPORTED_FHIR_VERSION}"),
        )]);
        validate_request_content_type(&h, BodyKind::FhirResource).expect("version ok");
    }

    #[test]
    fn accept_absent_is_fine() {
        let h = HeaderMap::new();
        validate_accept(&h).expect("no accept is fine");
    }

    #[test]
    fn accept_wildcard_and_supported_forms() {
        for accept in [
            "*/*",
            "application/*",
            "application/fhir+json",
            "application/json",
        ] {
            let h = headers(&[("accept", accept)]);
            validate_accept(&h).expect(accept);
        }
    }

    #[test]
    fn accept_rejects_unsupported_format() {
        let h = headers(&[("accept", "text/html")]);
        let err = validate_accept(&h).expect_err("reject");
        assert!(err.to_string().contains("Accept"));
    }

    #[test]
    fn accept_rejects_conflicting_fhir_version() {
        let h = headers(&[("accept", "application/fhir+json; fhirVersion=5.0")]);
        let err = validate_accept(&h).expect_err("reject");
        assert!(err.to_string().contains("fhirVersion"));
    }
}
