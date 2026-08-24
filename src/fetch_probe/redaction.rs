use url::Url;

const SENSITIVE_QUERY_MARKERS: &[&str] = &[
    "api_key",
    "apikey",
    "authorization",
    "credential",
    "password",
    "secret",
    "signature",
    "token",
];

pub fn contains_secret_query(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    url.query_pairs()
        .any(|(key, value)| !value.is_empty() && sensitive_key(&key))
}

pub fn scrub_url_query(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_string();
    };
    if !url
        .query_pairs()
        .any(|(key, value)| !value.is_empty() && sensitive_key(&key))
    {
        return raw.to_string();
    }
    let pairs = url
        .query_pairs()
        .map(|(key, value)| {
            let value = if sensitive_key(&key) && !value.is_empty() {
                "<REDACTED>".to_string()
            } else {
                value.into_owned()
            };
            (key.into_owned(), value)
        })
        .collect::<Vec<_>>();
    url.query_pairs_mut().clear().extend_pairs(pairs);
    url.to_string()
}

fn sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    SENSITIVE_QUERY_MARKERS
        .iter()
        .any(|marker| normalized == *marker || normalized.ends_with(&format!("_{marker}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_scrub_preserves_non_secret_pairs_and_removes_secret_values() {
        let raw = "https://example.test/a?page=2&access_token=very-secret&lang=ja";
        let scrubbed = scrub_url_query(raw);
        assert!(contains_secret_query(raw));
        assert!(!scrubbed.contains("very-secret"));
        assert!(scrubbed.contains("page=2"));
        assert!(scrubbed.contains("access_token=%3CREDACTED%3E"));
        assert_eq!(
            scrub_url_query("https://example.test/a?page=2&lang=ja"),
            "https://example.test/a?page=2&lang=ja"
        );
    }
}
