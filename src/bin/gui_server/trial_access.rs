use std::sync::Arc;

use anyhow::{Context, bail};
use axum::http::{HeaderMap, Uri, header};

const TOKEN_ENV: &str = "GUI_TRIAL_TOKEN";
const ORIGINS_ENV: &str = "GUI_TRIAL_ALLOWED_ORIGINS";
const PROXY_SAFE_AUTHORIZATION_HEADER: &str = "x-commandagent-trial-authorization";
type ValidatedEnvironment = (Option<Arc<str>>, Arc<[String]>);

#[derive(Debug, Clone)]
pub struct TrialAccess {
    token: Option<Arc<str>>,
    authentication_enabled: bool,
    /// Normalized, lowercased origins accepted as a mutation `Origin`.
    allowed_origins: Arc<[String]>,
    /// Lowercased authorities accepted as the request `Host`.
    allowed_hosts: Arc<[String]>,
}

impl TrialAccess {
    pub(super) fn validate_environment(authentication_enabled: bool) -> anyhow::Result<()> {
        validated_environment(authentication_enabled).map(|_| ())
    }

    /// In-process fixture constructor without a bound listener. It keeps the
    /// loopback authorities unguessable (port 0) so it is only useful for
    /// tests that call handlers directly; the served binary uses
    /// [`Self::from_environment_on_port`].
    pub fn from_environment(
        execution_enabled: bool,
        authentication_enabled: bool,
    ) -> anyhow::Result<Self> {
        Self::from_environment_on_port(execution_enabled, authentication_enabled, 0)
    }

    /// Build the access policy from the process environment.
    ///
    /// The token and `GUI_TRIAL_ALLOWED_ORIGINS` rules are the same whether or
    /// not an execution root is configured, so a dashboard-only server still
    /// accepts an allowlisted proxy authority as `Host`, still admits its
    /// Origin, and still fails closed on an invalid origin or a missing token
    /// (Issue #554). `execution_enabled` no longer changes what is read; it is
    /// retained so existing call sites keep one constructor.
    pub fn from_environment_on_port(
        _execution_enabled: bool,
        authentication_enabled: bool,
        listening_port: u16,
    ) -> anyhow::Result<Self> {
        let (token, allowed_origins) = validated_environment(authentication_enabled)?;
        Ok(Self::assemble(
            token,
            authentication_enabled,
            allowed_origins.to_vec(),
            listening_port,
        ))
    }

    fn assemble(
        token: Option<Arc<str>>,
        authentication_enabled: bool,
        allowed_origins: Vec<String>,
        listening_port: u16,
    ) -> Self {
        let mut origins = loopback_origins(listening_port).to_vec();
        let mut hosts = loopback_hosts(listening_port).to_vec();
        for origin in allowed_origins {
            if let Some(authority) = origin_authority(&origin) {
                hosts.push(authority);
            }
            origins.push(origin.to_ascii_lowercase());
        }
        Self {
            token,
            authentication_enabled,
            allowed_origins: Arc::from(origins),
            allowed_hosts: Arc::from(hosts),
        }
    }

    pub fn authentication_enabled(&self) -> bool {
        self.authentication_enabled
    }

    /// Reject any request whose `Host` is not exactly one of the allowed
    /// authorities (loopback with the listening port, or a configured proxy
    /// origin authority). A missing or duplicated `Host` is refused rather
    /// than trusting only the first value.
    pub fn authorize_host(&self, headers: &HeaderMap) -> Result<(), AccessError> {
        let mut values = headers.get_all(header::HOST).iter();
        let (Some(value), None) = (values.next(), values.next()) else {
            return Err(AccessError::ForbiddenHost);
        };
        let Ok(host) = value.to_str() else {
            return Err(AccessError::ForbiddenHost);
        };
        if self
            .allowed_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
        {
            Ok(())
        } else {
            Err(AccessError::ForbiddenHost)
        }
    }

    pub fn authorize(&self, headers: &HeaderMap, require_origin: bool) -> Result<(), AccessError> {
        if let Some(expected) = self.token.as_deref() {
            let supplied = headers
                .get(PROXY_SAFE_AUTHORIZATION_HEADER)
                .or_else(|| headers.get(header::AUTHORIZATION))
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .ok_or(AccessError::Unauthorized)?;
            if !constant_time_equal(expected.as_bytes(), supplied.as_bytes()) {
                return Err(AccessError::Unauthorized);
            }
        }
        if require_origin && !self.origin_allowed(headers) {
            return Err(AccessError::ForbiddenOrigin);
        }
        Ok(())
    }

    /// A mutation `Origin` must be an exact allowed origin: a loopback origin
    /// over `http`, or one listed by `GUI_TRIAL_ALLOWED_ORIGINS`. The old
    /// "any authority that matches the request Host" rule is gone, so a
    /// rebinding pair is never accepted just because it is self-consistent.
    fn origin_allowed(&self, headers: &HeaderMap) -> bool {
        let Some(origin) = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| normalize_origin(value).ok())
        else {
            return false;
        };
        let origin = origin.to_ascii_lowercase();
        self.allowed_origins
            .iter()
            .any(|allowed| allowed == &origin)
    }
}

fn loopback_hosts(port: u16) -> [String; 3] {
    [
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ]
}

fn loopback_origins(port: u16) -> [String; 3] {
    [
        format!("http://localhost:{port}"),
        format!("http://127.0.0.1:{port}"),
        format!("http://[::1]:{port}"),
    ]
}

fn origin_authority(origin: &str) -> Option<String> {
    origin
        .parse::<Uri>()
        .ok()?
        .authority()
        .map(|authority| authority.as_str().to_ascii_lowercase())
}

fn validated_environment(authentication_enabled: bool) -> anyhow::Result<ValidatedEnvironment> {
    let token = authentication_enabled
        .then(|| {
            let token = std::env::var(TOKEN_ENV).with_context(|| {
                format!("{TOKEN_ENV} is required when --trial-token-auth is on")
            })?;
            if token.len() < 32 || token.len() > 4096 || token.chars().any(char::is_whitespace) {
                bail!("{TOKEN_ENV} must contain 32..=4096 non-whitespace characters");
            }
            Ok::<Arc<str>, anyhow::Error>(Arc::from(token))
        })
        .transpose()?;
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
    Ok((token, Arc::from(allowed_origins)))
}

#[derive(Debug, Clone, Copy)]
pub enum AccessError {
    Unauthorized,
    ForbiddenOrigin,
    ForbiddenHost,
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

    const PORT: u16 = 4173;

    fn access_for_test(token: Option<&str>, origins: &[&str]) -> TrialAccess {
        TrialAccess::assemble(
            token.map(Arc::from),
            token.is_some(),
            origins.iter().map(|origin| origin.to_string()).collect(),
            PORT,
        )
    }

    fn headers_with_host(host: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, host.parse().unwrap());
        headers
    }

    #[test]
    fn explicit_proxy_origin_requires_the_runtime_token() {
        let access = access_for_test(
            Some("commandagent-gui-test-token-000000000001"),
            &["https://admin.example.com"],
        );
        let mut headers = headers_with_host("127.0.0.1:4173");
        headers.insert(
            header::AUTHORIZATION,
            "Bearer commandagent-gui-test-token-000000000001"
                .parse()
                .unwrap(),
        );
        headers.insert(header::ORIGIN, "https://admin.example.com".parse().unwrap());

        assert!(access.authorize(&headers, true).is_ok());
        headers.insert(header::ORIGIN, "https://attacker.invalid".parse().unwrap());
        assert!(matches!(
            access.authorize(&headers, true),
            Err(AccessError::ForbiddenOrigin)
        ));
    }

    #[test]
    fn proxy_safe_bearer_header_survives_authorization_stripping() {
        let access = access_for_test(
            Some("commandagent-gui-test-token-000000000001"),
            &["https://admin.example.com"],
        );
        let mut headers = headers_with_host("127.0.0.1:4173");
        headers.insert(
            PROXY_SAFE_AUTHORIZATION_HEADER,
            "Bearer commandagent-gui-test-token-000000000001"
                .parse()
                .unwrap(),
        );
        headers.insert(header::ORIGIN, "https://admin.example.com".parse().unwrap());

        assert!(access.authorize(&headers, true).is_ok());
        headers.insert(
            PROXY_SAFE_AUTHORIZATION_HEADER,
            "Bearer commandagent-gui-test-token-wrong-value"
                .parse()
                .unwrap(),
        );
        assert!(matches!(
            access.authorize(&headers, true),
            Err(AccessError::Unauthorized)
        ));
    }

    #[test]
    fn disabled_token_auth_still_requires_an_allowed_post_origin() {
        let access = access_for_test(None, &[]);
        let mut headers = headers_with_host("127.0.0.1:4173");
        headers.insert(header::ORIGIN, "http://127.0.0.1:4173".parse().unwrap());

        assert!(access.authorize(&headers, true).is_ok());
        headers.insert(header::ORIGIN, "https://attacker.invalid".parse().unwrap());
        assert!(matches!(
            access.authorize(&headers, true),
            Err(AccessError::ForbiddenOrigin)
        ));
    }

    #[test]
    fn host_must_match_loopback_and_listening_port_exactly() {
        let access = access_for_test(None, &[]);
        for host in [
            "127.0.0.1:4173",
            "localhost:4173",
            "[::1]:4173",
            "LOCALHOST:4173",
            "127.0.0.1:4173",
        ] {
            assert!(
                access.authorize_host(&headers_with_host(host)).is_ok(),
                "expected {host} to be allowed"
            );
        }
        for host in [
            "127.0.0.1:1",
            "localhost",
            "localhost:4173.",
            "127.0.0.1:4173.",
            "[::ffff:127.0.0.1]:4173",
            "2130706433:4173",
            "127.0.0.1.attacker.example:4173",
            "localhost.attacker.example:4173",
            "user@127.0.0.1:4173",
            "attacker.example:4173",
            "127.0.0.1:4173@attacker.example",
        ] {
            assert!(
                matches!(
                    access.authorize_host(&headers_with_host(host)),
                    Err(AccessError::ForbiddenHost)
                ),
                "expected {host} to be rejected"
            );
        }
    }

    #[test]
    fn host_header_must_be_present_exactly_once() {
        let access = access_for_test(None, &[]);
        assert!(matches!(
            access.authorize_host(&HeaderMap::new()),
            Err(AccessError::ForbiddenHost)
        ));

        let mut duplicated = HeaderMap::new();
        duplicated.append(header::HOST, "127.0.0.1:4173".parse().unwrap());
        duplicated.append(header::HOST, "attacker.example:4173".parse().unwrap());
        assert!(matches!(
            access.authorize_host(&duplicated),
            Err(AccessError::ForbiddenHost)
        ));
    }

    #[test]
    fn allowlist_authority_is_accepted_as_host_and_origin() {
        let access = access_for_test(None, &["https://gui.example.test"]);
        assert!(
            access
                .authorize_host(&headers_with_host("gui.example.test"))
                .is_ok()
        );
        assert!(matches!(
            access.authorize_host(&headers_with_host("other.example.test")),
            Err(AccessError::ForbiddenHost)
        ));

        let mut headers = headers_with_host("127.0.0.1:4173");
        headers.insert(header::ORIGIN, "https://gui.example.test".parse().unwrap());
        assert!(access.authorize(&headers, true).is_ok());

        headers.insert(
            header::ORIGIN,
            "https://other.example.test".parse().unwrap(),
        );
        assert!(matches!(
            access.authorize(&headers, true),
            Err(AccessError::ForbiddenOrigin)
        ));
    }

    #[test]
    fn mutation_origin_scheme_must_match_the_loopback_http_serving() {
        let access = access_for_test(None, &[]);
        let mut headers = headers_with_host("127.0.0.1:4173");
        for origin in [
            "http://127.0.0.1:4173",
            "http://localhost:4173",
            "http://[::1]:4173",
        ] {
            headers.insert(header::ORIGIN, origin.parse().unwrap());
            assert!(access.authorize(&headers, true).is_ok(), "{origin}");
        }
        for origin in ["https://127.0.0.1:4173", "http://127.0.0.1:1", "null"] {
            headers.insert(header::ORIGIN, origin.parse().unwrap());
            assert!(
                matches!(
                    access.authorize(&headers, true),
                    Err(AccessError::ForbiddenOrigin)
                ),
                "expected {origin} to be rejected"
            );
        }
    }
}
