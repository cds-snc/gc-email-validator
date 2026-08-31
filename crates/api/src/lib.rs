mod generated_domains;

use std::fmt;

use lambda_http::{Body, Error, Request, Response, http::Method, http::StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use generated_domains::{DATASET_VERSION, NAMESPACE_ROOTS, RULES};

const CLASSIFY_PATH: &str = "/v1/email-domain-classifications";
const MAX_BODY_BYTES: usize = 4_096;

#[derive(Debug)]
pub struct DomainRule {
    pub domain: &'static str,
    pub include_subdomains: bool,
    pub gc_org_id: Option<u32>,
    pub organization_en: Option<&'static str>,
    pub organization_fr: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassificationRequest {
    email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    pub is_government_of_canada: bool,
    pub is_government_controlled_namespace: bool,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_domain: Option<&'static str>,
    pub match_type: MatchType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<Organization>,
    pub dataset_version: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchType {
    RecognizedDomain,
    NamespaceOnly,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub gc_org_id: u32,
    pub name_en: &'static str,
    pub name_fr: &'static str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("email must be a single common internet email address")]
    InvalidEmail,
    #[error("email exceeds 254 bytes")]
    EmailTooLong,
    #[error("email domain is invalid")]
    InvalidDomain,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    dataset_version: &'static str,
    rule_count: usize,
}

/// Classifies an email without DNS or network access.
///
/// `is_government_of_canada` means the domain matched the reviewed dataset.
/// `is_government_controlled_namespace` is broader: it also covers otherwise
/// unknown subdomains beneath a configured Government of Canada namespace.
pub fn classify_email(email: &str) -> Result<Classification, ValidationError> {
    let domain = extract_domain(email)?;
    let namespace_controlled = is_in_namespace(&domain);
    let matched = find_rule(&domain);

    let (matched_domain, organization, match_type) = match matched {
        Some(rule) => (
            Some(rule.domain),
            rule.gc_org_id.map(|gc_org_id| Organization {
                gc_org_id,
                name_en: rule.organization_en.unwrap_or(""),
                name_fr: rule.organization_fr.unwrap_or(""),
            }),
            MatchType::RecognizedDomain,
        ),
        None if namespace_controlled => (None, None, MatchType::NamespaceOnly),
        None => (None, None, MatchType::None),
    };

    Ok(Classification {
        is_government_of_canada: matched.is_some(),
        is_government_controlled_namespace: namespace_controlled,
        domain,
        matched_domain,
        match_type,
        organization,
        dataset_version: DATASET_VERSION,
    })
}

pub async fn handle_request(request: Request) -> Result<Response<Body>, Error> {
    let path = request.uri().path();

    if path == "/health" && request.method() == Method::GET {
        return json_response(
            StatusCode::OK,
            &HealthResponse {
                status: "ok",
                dataset_version: DATASET_VERSION,
                rule_count: RULES.len(),
            },
        );
    }

    if path != CLASSIFY_PATH {
        return error_response(StatusCode::NOT_FOUND, "notFound", "route not found");
    }

    if request.method() != Method::POST {
        let mut response = error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "methodNotAllowed",
            "use POST for this route",
        )?;
        response.headers_mut().insert("allow", "POST".parse()?);
        return Ok(response);
    }

    let body = request.body().as_ref();
    if body.len() > MAX_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payloadTooLarge",
            "request body must not exceed 4096 bytes",
        );
    }

    let payload: ClassificationRequest = match serde_json::from_slice(body) {
        Ok(payload) => payload,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalidJson",
                "body must be JSON with exactly one string field named email",
            );
        }
    };

    match classify_email(&payload.email) {
        Ok(classification) => json_response(StatusCode::OK, &classification),
        Err(error) => error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalidEmail", error),
    }
}

fn extract_domain(email: &str) -> Result<String, ValidationError> {
    if email.len() > 254 {
        return Err(ValidationError::EmailTooLong);
    }
    if email.is_empty() || email.trim() != email || email.chars().any(char::is_control) {
        return Err(ValidationError::InvalidEmail);
    }

    let (local, raw_domain) = email.split_once('@').ok_or(ValidationError::InvalidEmail)?;
    if local.is_empty()
        || local.len() > 64
        || local.contains('@')
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !local.bytes().all(is_valid_local_byte)
    {
        return Err(ValidationError::InvalidEmail);
    }

    let domain = idna::domain_to_ascii(raw_domain).map_err(|_| ValidationError::InvalidDomain)?;
    let domain = domain.to_ascii_lowercase();
    validate_domain(&domain)?;
    Ok(domain)
}

fn is_valid_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'.' | b'!'
                | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'/'
                | b'='
                | b'?'
                | b'^'
                | b'_'
                | b'`'
                | b'{'
                | b'|'
                | b'}'
                | b'~'
        )
}

fn validate_domain(domain: &str) -> Result<(), ValidationError> {
    if domain.is_empty() || domain.len() > 253 || domain.ends_with('.') || !domain.contains('.') {
        return Err(ValidationError::InvalidDomain);
    }

    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(ValidationError::InvalidDomain);
    }

    Ok(())
}

fn is_in_namespace(domain: &str) -> bool {
    NAMESPACE_ROOTS.iter().any(|root| {
        domain == *root
            || domain
                .strip_suffix(root)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

fn find_rule(domain: &str) -> Option<&'static DomainRule> {
    let mut candidate = domain;
    loop {
        if let Ok(index) = RULES.binary_search_by_key(&candidate, |rule| rule.domain) {
            let rule = &RULES[index];
            if candidate == domain || rule.include_subdomains {
                return Some(rule);
            }
        }

        candidate = candidate.split_once('.')?.1;
    }
}

fn json_response<T: Serialize>(status: StatusCode, value: &T) -> Result<Response<Body>, Error> {
    let body = serde_json::to_string(value)?;
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json; charset=utf-8")
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::Text(body))?)
}

fn error_response(
    status: StatusCode,
    code: &'static str,
    message: impl fmt::Display,
) -> Result<Response<Body>, Error> {
    json_response(
        status,
        &ErrorEnvelope {
            error: ErrorBody {
                code,
                message: message.to_string(),
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_exact_and_child_domains_case_insensitively() {
        let exact = classify_email("Ada.Lovelace@STATCAN.GC.CA").unwrap();
        assert!(exact.is_government_of_canada);
        assert_eq!(exact.domain, "statcan.gc.ca");
        assert_eq!(exact.matched_domain, Some("statcan.gc.ca"));

        let child = classify_email("ada@mail.statcan.gc.ca").unwrap();
        assert!(child.is_government_of_canada);
        assert_eq!(child.matched_domain, Some("statcan.gc.ca"));
    }

    #[test]
    fn distinguishes_namespace_ownership_from_a_recognized_domain() {
        let result = classify_email("person@unlisted.canada.ca").unwrap();
        assert!(!result.is_government_of_canada);
        assert!(result.is_government_controlled_namespace);
        assert_eq!(result.match_type, MatchType::NamespaceOnly);
    }

    #[test]
    fn rejects_suffix_confusion() {
        for email in [
            "person@evilgc.ca",
            "person@statcan.gc.ca.example.com",
            "person@gc.ca.example.org",
        ] {
            let result = classify_email(email).unwrap();
            assert!(!result.is_government_of_canada, "{email}");
            assert!(!result.is_government_controlled_namespace, "{email}");
        }
    }

    #[test]
    fn rejects_ambiguous_or_unsupported_email_syntax() {
        for email in [
            "Display Name <person@statcan.gc.ca>",
            "person@@statcan.gc.ca",
            ".person@statcan.gc.ca",
            "person..name@statcan.gc.ca",
            "person@localhost",
            "person@-statcan.gc.ca",
            " person@statcan.gc.ca",
        ] {
            assert!(classify_email(email).is_err(), "{email}");
        }
    }
}
