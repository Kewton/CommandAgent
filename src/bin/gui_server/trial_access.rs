use std::sync::Arc;

use anyhow::{Context, bail};
use axum::http::{HeaderMap, Uri, header};

const TOKEN_ENV: &str = "GUI_TRIAL_TOKEN";
const ORIGINS_ENV: &str = "GUI_TRIAL_ALLOWED_ORIGINS";

#[derive(Debug, Clone)]
pub struct TrialAccess {
    token: Option<Arc<str>>,
    allowed_origins: Arc<[String]>,
}

impl TrialAccess {
    pub fn from_environment(execution_enabled: bool) -> anyhow::Result<Self> {
        if !execution_enabled {
            return Ok(Self {
                token: None,
                allowed_origins: Arc::from([]),
            });
        }
        let token = std::env::var(TOKEN_ENV)
            .with_context(|| format!("{TOKEN_ENV} is required when --execution-root is set"))?;
        if token.len() < 32 || token.len() > 4096 || token.chars().any(char::is_whitespace) {
            bail!("{TOKEN_ENV} must contain 32..=4096 non-whitespace characters");
        }
        let allowed_origins = std::env::var(ORIGINS_ENV)
            .ok()
            .into_iter()
            .flat_map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|value| !value.is_empty())
            .map(|value| normalize_origin(&value))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self {
            token: Some(Arc::from(token)),
            allowed_origins: Arc::from(allowed_origins),
        })
    }

    pub fn authorize(&self, headers: &HeaderMap, require_origin: bool) -> Result<(), AccessError> {
        let expected = self.token.as_deref().ok_or(AccessError::Disabled)?;
        let supplied = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(AccessError::Unauthorized)?;
        if !constant_time_equal(expected.as_bytes(), supplied.as_bytes()) {
            return Err(AccessError::Unauthorized);
        }
        if require_origin && !self.origin_allowed(headers) {
            return Err(AccessError::ForbiddenOrigin);
        }
        Ok(())
    }

    fn origin_allowed(&self, headers: &HeaderMap) -> bool {
        let Some(origin) = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| normalize_origin(value).ok())
        else {
            return false;
        };
        if self
            .allowed_origins
            .iter()
            .any(|allowed| allowed == &origin)
        {
            return true;
        }
        let Some(host) = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };
        origin
            .parse::<Uri>()
            .ok()
            .and_then(|uri| {
                uri.authority()
                    .map(|authority| authority.as_str().to_string())
            })
            .is_some_and(|authority| authority.eq_ignore_ascii_case(host))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AccessError {
    Disabled,
    Unauthorized,
    ForbiddenOrigin,
}

fn normalize_origin(value: &str) -> anyhow::Result<String> {
    let uri = value
        .parse::<Uri>()
        .with_context(|| format!("invalid trial origin {value:?}"))?;
    let scheme = uri
        .scheme_str()
        .filter(|scheme| matches!(*scheme, "http" | "https"))
        .context("trial origin must use http or https")?;
    let authority = uri
        .authority()
        .context("trial origin must include a host")?;
    if uri.path() != "/" || uri.query().is_some() {
        bail!("trial origin must not include a path or query: {value:?}");
    }
    Ok(format!("{scheme}://{authority}"))
}

fn constant_time_equal(expected: &[u8], supplied: &[u8]) -> bool {
    let mut difference = expected.len() ^ supplied.len();
    let compared = expected.len().max(supplied.len());
    for index in 0..compared {
        let left = expected.get(index).copied().unwrap_or_default();
        let right = supplied.get(index).copied().unwrap_or_default();
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_proxy_origin_requires_the_runtime_token() {
        let access = TrialAccess {
            token: Some(Arc::from("commandagent-gui-test-token-000000000001")),
            allowed_origins: Arc::from(["https://admin.example.com".to_string()]),
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer commandagent-gui-test-token-000000000001"
                .parse()
                .unwrap(),
        );
        headers.insert(header::ORIGIN, "https://admin.example.com".parse().unwrap());
        headers.insert(header::HOST, "127.0.0.1:4173".parse().unwrap());

        assert!(access.authorize(&headers, true).is_ok());
        headers.insert(header::ORIGIN, "https://attacker.invalid".parse().unwrap());
        assert!(matches!(
            access.authorize(&headers, true),
            Err(AccessError::ForbiddenOrigin)
        ));
    }
}
