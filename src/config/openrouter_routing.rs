//! Typed OpenRouter provider routing for an `openai-compatible` preset
//! (Issue #939).
//!
//! `openrouter_provider` is a one-line TOML inline table with at most the
//! three keys `only`, `order`, and `allow_fallbacks`. The value is a
//! declaration sent as the top-level `provider` object of the generic
//! OpenAI-compatible request; CommandAgent never checks which providers exist,
//! never fills in an omitted key, and never infers routing from a model or
//! host name. An empty table `{}` explicitly clears an inherited routing.
//! Validation errors name the file, line, and key only, never the value.

use std::path::Path;

use anyhow::bail;
use serde_json::{Map, Value};

use super::{OpenAiCompatibleConfig, PresetConfig, sourced};

pub const OPENROUTER_PROVIDER_KEY: &str = "openrouter_provider";

/// Accepted keys, in the order the request object documents them.
pub const OPENROUTER_PROVIDER_FIELDS: [&str; 3] = ["only", "order", "allow_fallbacks"];

/// The longest accepted provider slug, in bytes.
pub const MAX_SLUG_BYTES: usize = 128;

/// The most slugs one `only` or `order` list may hold.
pub const MAX_SLUGS: usize = 32;

/// The parsed routing table. Every field keeps exactly what the preset
/// declared: an omitted key stays `None` and is not sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenRouterProviderRouting {
    only: Option<Vec<String>>,
    order: Option<Vec<String>>,
    allow_fallbacks: Option<bool>,
}

impl OpenRouterProviderRouting {
    /// `{}`: the preset clears any inherited routing and sends nothing.
    pub fn is_disabled(&self) -> bool {
        self.only.is_none() && self.order.is_none() && self.allow_fallbacks.is_none()
    }

    pub fn only(&self) -> Option<&[String]> {
        self.only.as_deref()
    }

    pub fn order(&self) -> Option<&[String]> {
        self.order.as_deref()
    }

    pub fn allow_fallbacks(&self) -> Option<bool> {
        self.allow_fallbacks
    }

    /// The request `provider` object: only the declared keys, with the list
    /// order and values unchanged.
    pub fn provider_object(&self) -> Value {
        let mut object = Map::new();
        if let Some(only) = &self.only {
            object.insert("only".to_string(), Value::from(only.clone()));
        }
        if let Some(order) = &self.order {
            object.insert("order".to_string(), Value::from(order.clone()));
        }
        if let Some(allow_fallbacks) = self.allow_fallbacks {
            object.insert("allow_fallbacks".to_string(), Value::Bool(allow_fallbacks));
        }
        Value::Object(object)
    }
}

/// The routing a resolved generic OpenAI-compatible configuration carries,
/// with the preset that declared it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOpenRouterRouting {
    pub routing: OpenRouterProviderRouting,
    pub source: String,
}

impl ResolvedOpenRouterRouting {
    pub fn is_declared(&self) -> bool {
        !self.routing.is_disabled()
    }
}

impl OpenAiCompatibleConfig {
    /// The preset's `openrouter_provider`, including an explicit `{}`.
    pub fn openrouter_provider(&self) -> Option<&ResolvedOpenRouterRouting> {
        self.openrouter_provider.as_ref()
    }
}

/// Resolves the selected preset's routing against the resolved roles. A
/// declared routing without any generic OpenAI-compatible role is a
/// configuration error; `{}` without one is accepted and sends nothing.
pub(super) fn resolve(
    preset: Option<&PresetConfig>,
    any_openai_compatible: bool,
) -> anyhow::Result<Option<ResolvedOpenRouterRouting>> {
    let Some(routing) = preset.and_then(|preset| preset.openrouter_provider.as_ref()) else {
        return Ok(None);
    };
    if !any_openai_compatible && !routing.value.is_disabled() {
        bail!(
            "{OPENROUTER_PROVIDER_KEY} (from {}) requires at least one openai-compatible role; it is not sent to lm-studio, ollama, openai, or gemini",
            routing.source
        );
    }
    Ok(Some(ResolvedOpenRouterRouting {
        routing: routing.value.clone(),
        source: routing.source.clone(),
    }))
}

/// Refuses `openrouter_provider` written as a dotted or quoted preset key
/// (`openrouter_provider.x = ...`, `"openrouter\u005fprovider" = ...`). The
/// key is decoded with the TOML rules, and the unknown suffix is never shown,
/// so it cannot leak through the general unknown-key diagnostic.
pub(super) fn reject_unsupported_key(path: &Path, line_no: usize, key: &str) -> anyhow::Result<()> {
    let segments = decoded_key(key);
    if key != OPENROUTER_PROVIDER_KEY
        && segments.first().map(String::as_str) == Some(OPENROUTER_PROVIDER_KEY)
    {
        bail!("{}", unsupported_form(path, line_no));
    }
    Ok(())
}

/// Refuses a `[preset.<name>.openrouter_provider...]` table or array-of-tables
/// section header, decoded with the TOML rules, naming only the fixed key.
pub(super) fn reject_unsupported_section(
    path: &Path,
    line_no: usize,
    section: &str,
) -> anyhow::Result<()> {
    let segments = decoded_header(section);
    if segments.first().map(String::as_str) == Some("preset")
        && segments
            .iter()
            .skip(2)
            .any(|segment| segment == OPENROUTER_PROVIDER_KEY)
    {
        bail!("{}", unsupported_form(path, line_no));
    }
    Ok(())
}

/// The decoded segments of a TOML key (quotes, escapes, and dotted
/// boundaries resolved as TOML does). A key the TOML parser rejects decodes to
/// no segments; the parser's own message is dropped, never shown.
fn decoded_key(key: &str) -> Vec<String> {
    format!("{key} = 0")
        .parse::<toml::Table>()
        .map(|table| key_path(&table))
        .unwrap_or_default()
}

/// The decoded segments of a section header's inner text; `section` is what
/// lies between the outer `[` and `]`, so an array-of-tables header still
/// carries its inner brackets.
fn decoded_header(section: &str) -> Vec<String> {
    format!("[{section}]")
        .parse::<toml::Table>()
        .map(|table| key_path(&table))
        .unwrap_or_default()
}

/// Follows the single-entry chain a one-key document or one header builds.
fn key_path(table: &toml::Table) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = table;
    while current.len() == 1 {
        let Some((name, value)) = current.iter().next() else {
            break;
        };
        segments.push(name.clone());
        match value {
            toml::Value::Table(next) => current = next,
            toml::Value::Array(items) => match items.first() {
                Some(toml::Value::Table(next)) if items.len() == 1 => current = next,
                _ => break,
            },
            _ => break,
        }
    }
    segments
}

fn unsupported_form(path: &Path, line_no: usize) -> String {
    format!(
        "{}:{} {OPENROUTER_PROVIDER_KEY} must be a one-line inline table in a [preset.<name>] section; dotted keys, quoted keys, and table sections are not supported (the key and value are not shown)",
        path.display(),
        line_no
    )
}

/// Parses `openrouter_provider` into `preset`, recording its source.
pub(super) fn parse_into(
    preset: &mut PresetConfig,
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
    source: &str,
) -> anyhow::Result<()> {
    preset.openrouter_provider = Some(sourced(parse_value(path, line_no, key, value)?, source));
    Ok(())
}

/// Parses one `openrouter_provider = { ... }` value. The messages never
/// include the rejected value or the TOML text.
pub(super) fn parse_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<OpenRouterProviderRouting> {
    let location = format!("{}:{} {key}", path.display(), line_no);
    let Some(table) = format!("value = {value}")
        .parse::<toml::Table>()
        .ok()
        .and_then(|mut document| document.remove("value"))
        .and_then(|value| match value {
            toml::Value::Table(table) => Some(table),
            _ => None,
        })
    else {
        bail!(
            "{location} expects a one-line inline table with only, order, or allow_fallbacks (the value is not shown)"
        );
    };
    let mut routing = OpenRouterProviderRouting::default();
    for (field, value) in table {
        match field.as_str() {
            "only" => routing.only = Some(parse_slugs(&location, "only", value)?),
            "order" => routing.order = Some(parse_slugs(&location, "order", value)?),
            "allow_fallbacks" => match value {
                toml::Value::Boolean(flag) => routing.allow_fallbacks = Some(flag),
                _ => bail!("{location}.allow_fallbacks expects a boolean (the value is not shown)"),
            },
            _ => bail!(
                "{location} contains an unsupported key; only {} are accepted (the key and value are not shown)",
                OPENROUTER_PROVIDER_FIELDS.join(", ")
            ),
        }
    }
    Ok(routing)
}

fn parse_slugs(location: &str, field: &str, value: toml::Value) -> anyhow::Result<Vec<String>> {
    let toml::Value::Array(entries) = value else {
        bail!("{location}.{field} expects an array of provider slugs (the value is not shown)");
    };
    if entries.is_empty() {
        bail!(
            "{location}.{field} must not be empty; use {OPENROUTER_PROVIDER_KEY} = {{}} to clear routing"
        );
    }
    if entries.len() > MAX_SLUGS {
        bail!("{location}.{field} accepts at most {MAX_SLUGS} provider slugs");
    }
    let mut slugs: Vec<String> = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let position = index + 1;
        let toml::Value::String(slug) = entry else {
            bail!("{location}.{field} entry {position} expects a string (the value is not shown)");
        };
        if let Some(reason) = slug_problem(&slug) {
            bail!("{location}.{field} entry {position} {reason} (the value is not shown)");
        }
        if slugs.contains(&slug) {
            bail!(
                "{location}.{field} entry {position} repeats an earlier provider slug (the value is not shown)"
            );
        }
        slugs.push(slug);
    }
    Ok(slugs)
}

/// Why `slug` is rejected, or `None` when it is a well-formed slug: non-empty
/// segments of ASCII letters, digits, `-`, `_`, or `.`, joined by `/`.
fn slug_problem(slug: &str) -> Option<&'static str> {
    if slug.is_empty() {
        return Some("must not be empty");
    }
    if slug.chars().any(char::is_control) {
        return Some("must not contain control characters");
    }
    if slug.contains("${") {
        return Some("must not use ${ENV}; routing values are not expanded");
    }
    if slug.len() > MAX_SLUG_BYTES {
        return Some("is longer than 128 bytes");
    }
    let well_formed = slug.split('/').all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
    });
    (!well_formed).then_some("is not a provider slug (letters, digits, -, _, . joined by /)")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::{
        MAX_SLUGS, OpenRouterProviderRouting, parse_value, reject_unsupported_key,
        reject_unsupported_section, resolve,
    };
    use crate::config::PresetConfig;
    use crate::config::resolution::merge_inherited_preset;
    use crate::config::sourced;

    const KEY: &str = "preset.glm.openrouter_provider";

    fn parse(value: &str) -> anyhow::Result<OpenRouterProviderRouting> {
        parse_value(Path::new("config.toml"), 7, KEY, value)
    }

    fn rejected(value: &str, reason: &str, secret: &str) {
        let error = format!("{:#}", parse(value).expect_err(value));
        assert!(
            error.starts_with("config.toml:7 preset.glm.openrouter_provider"),
            "{error}"
        );
        assert!(error.contains(reason), "{error}");
        assert!(!secret.is_empty() && !error.contains(secret), "{error}");
    }

    #[test]
    fn three_keys_keep_their_declared_values_and_order() {
        let routing = parse(
            r#"{ only = ["open-inference/fp4", "b"], order = ["z", "a.b_c"], allow_fallbacks = false }"#,
        )
        .unwrap();
        assert_eq!(
            routing.provider_object(),
            json!({"only": ["open-inference/fp4", "b"], "order": ["z", "a.b_c"], "allow_fallbacks": false})
        );
        assert_eq!(routing.only().unwrap()[0], "open-inference/fp4");
        assert_eq!(routing.order().unwrap(), ["z", "a.b_c"]);
        assert!(!routing.is_disabled());
    }

    #[test]
    fn an_omitted_key_is_not_filled_in() {
        let routing = parse(r#"{ only = ["open-inference"] }"#).unwrap();
        assert_eq!(routing.allow_fallbacks(), None);
        assert!(routing.order().is_none());
        assert_eq!(
            routing.provider_object(),
            json!({"only": ["open-inference"]})
        );
        let routing = parse("{ allow_fallbacks = true }").unwrap();
        assert_eq!(routing.provider_object(), json!({"allow_fallbacks": true}));
        let routing = parse(r#"{ order = ["a"] }"#).unwrap();
        assert_eq!(routing.provider_object(), json!({"order": ["a"]}));
    }

    #[test]
    fn an_empty_table_disables_routing() {
        let routing = parse("{}").unwrap();
        assert!(routing.is_disabled());
        assert_eq!(routing.provider_object(), json!({}));
    }

    #[test]
    fn invalid_shapes_are_rejected_without_the_value() {
        rejected(
            r#"{ only = ["a"], extra_body = "SECRET-939-a" }"#,
            "unsupported key",
            "SECRET-939-a",
        );
        rejected(
            r#"{ model = "SECRET-939-m" }"#,
            "unsupported key",
            "SECRET-939-m",
        );
        rejected(
            r#"{ provider = { only = ["SECRET-939-n"] } }"#,
            "unsupported key",
            "SECRET-939-n",
        );
        rejected(
            r#"{ only = "SECRET-939-s" }"#,
            "expects an array",
            "SECRET-939-s",
        );
        rejected(r#"{ only = [1939] }"#, "expects a string", "1939");
        rejected(
            r#"{ allow_fallbacks = "SECRET-939-b" }"#,
            "expects a boolean",
            "SECRET-939-b",
        );
        rejected("{ only = [] }", "must not be empty", "only = []");
        rejected(r#"{ order = [""] }"#, "must not be empty", "order = [\"\"]");
        rejected(r#""SECRET-939-q""#, "inline table", "SECRET-939-q");
        rejected(r#"["SECRET-939-r"]"#, "inline table", "SECRET-939-r");
        rejected(
            r#"{ only = ["SECRET-939-x" "#,
            "inline table",
            "SECRET-939-x",
        );
    }

    #[test]
    fn malformed_slugs_are_rejected_without_the_value() {
        rejected(
            r#"{ only = ["SECRET-939/"] }"#,
            "not a provider slug",
            "SECRET-939",
        );
        rejected(
            r#"{ only = ["/SECRET-939"] }"#,
            "not a provider slug",
            "SECRET-939",
        );
        rejected(
            r#"{ only = ["SECRET 939"] }"#,
            "not a provider slug",
            "SECRET 939",
        );
        rejected(
            r#"{ only = ["SECRET-939//x"] }"#,
            "not a provider slug",
            "SECRET-939",
        );
        rejected(
            r#"{ order = ["SECRET-939\u0007"] }"#,
            "control characters",
            "SECRET-939",
        );
        rejected(
            r#"{ only = ["SECRET-939\n"] }"#,
            "control characters",
            "SECRET-939",
        );
        rejected(r#"{ only = ["${SECRET_939}"] }"#, "${ENV}", "SECRET_939");
        rejected(
            r#"{ only = ["a", "SECRET-939", "SECRET-939"] }"#,
            "entry 3 repeats",
            "SECRET-939",
        );
        let long = format!(r#"{{ only = ["{}"] }}"#, "S".repeat(129));
        rejected(&long, "longer than 128 bytes", &"S".repeat(129));
        let exact = format!(r#"{{ only = ["{}"] }}"#, "s".repeat(128));
        assert!(parse(&exact).is_ok());
    }

    #[test]
    fn list_length_is_bounded() {
        let slugs = |count: usize| {
            (0..count)
                .map(|index| format!("\"p{index}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        assert!(parse(&format!("{{ order = [{}] }}", slugs(MAX_SLUGS))).is_ok());
        rejected(
            &format!("{{ order = [{}] }}", slugs(MAX_SLUGS + 1)),
            "at most 32",
            "p32",
        );
    }

    #[test]
    fn the_same_slug_may_appear_in_only_and_order() {
        let routing = parse(r#"{ only = ["a"], order = ["a"] }"#).unwrap();
        assert_eq!(routing.only(), routing.order());
    }

    #[test]
    fn declared_routing_needs_an_openai_compatible_role() {
        let preset = |value: &str| PresetConfig {
            openrouter_provider: Some(sourced(parse(value).unwrap(), "preset:glm")),
            ..PresetConfig::default()
        };
        let declared = preset(r#"{ only = ["a"] }"#);
        let error = format!("{:#}", resolve(Some(&declared), false).unwrap_err());
        assert!(
            error.contains("requires at least one openai-compatible role"),
            "{error}"
        );
        let resolved = resolve(Some(&declared), true).unwrap().unwrap();
        assert!(resolved.is_declared());
        assert_eq!(resolved.source, "preset:glm");

        let disabled = preset("{}");
        assert!(resolve(Some(&disabled), false).unwrap().is_some());
        assert!(
            !resolve(Some(&disabled), true)
                .unwrap()
                .unwrap()
                .is_declared()
        );
        assert!(
            resolve(Some(&PresetConfig::default()), false)
                .unwrap()
                .is_none()
        );
        assert!(resolve(None, false).unwrap().is_none());
    }

    #[test]
    fn inheritance_replaces_the_whole_table_and_keeps_its_source() {
        let mut child = PresetConfig {
            openrouter_provider: Some(sourced(
                parse(r#"{ order = ["b"] }"#).unwrap(),
                "preset:child",
            )),
            ..PresetConfig::default()
        };
        let parent = PresetConfig {
            openrouter_provider: Some(sourced(
                parse(r#"{ only = ["a"], allow_fallbacks = false }"#).unwrap(),
                "preset:parent",
            )),
            ..PresetConfig::default()
        };
        merge_inherited_preset(&mut child, &parent);
        let merged = child.openrouter_provider.unwrap();
        assert_eq!(merged.source, "preset:child");
        assert_eq!(merged.value.provider_object(), json!({"order": ["b"]}));

        let mut cleared = PresetConfig {
            openrouter_provider: Some(sourced(parse("{}").unwrap(), "preset:child")),
            ..PresetConfig::default()
        };
        merge_inherited_preset(&mut cleared, &parent);
        assert!(cleared.openrouter_provider.unwrap().value.is_disabled());

        let mut inherits = PresetConfig::default();
        merge_inherited_preset(&mut inherits, &parent);
        let inherited = inherits.openrouter_provider.unwrap();
        assert_eq!(inherited.source, "preset:parent");
        assert_eq!(
            inherited.value.provider_object(),
            json!({"only": ["a"], "allow_fallbacks": false})
        );
    }

    #[test]
    fn dotted_quoted_and_section_forms_are_refused_without_the_suffix() {
        let path = Path::new("config.toml");
        for key in [
            "openrouter_provider.\"PRIVATE_MARKER_939\"",
            "openrouter_provider.PRIVATE_MARKER_939",
            "openrouter_provider . only",
            "\"openrouter_provider\".PRIVATE_MARKER_939",
            "\"openrouter_provider\"",
            "\"openrouter\\u005fprovider\".PRIVATE_MARKER_939",
            "\"openrouter\\U0000005Fprovider\".PRIVATE_MARKER_939",
            "'openrouter_provider'.PRIVATE_MARKER_939",
        ] {
            let error = format!("{:#}", reject_unsupported_key(path, 4, key).unwrap_err());
            assert!(
                error.starts_with("config.toml:4 openrouter_provider must be"),
                "{error}"
            );
            assert!(
                !error.contains("PRIVATE_MARKER_939") && !error.contains("only"),
                "{error}"
            );
        }
        for section in [
            "preset.routed.openrouter_provider",
            " preset . routed . openrouter_provider ",
            "preset.routed.openrouter_provider.PRIVATE_MARKER_939",
            "[preset.routed.openrouter_provider]",
            "preset.\"routed\".\"openrouter_provider\"",
            "preset.routed.\"openrouter\\u005fprovider\"",
            "[preset.routed.\"openrouter\\u005fprovider\"]",
            "\"pre\\u0073et\".routed.openrouter_provider",
        ] {
            let error = format!(
                "{:#}",
                reject_unsupported_section(path, 9, section).unwrap_err()
            );
            assert!(
                error.starts_with("config.toml:9 openrouter_provider must be"),
                "{error}"
            );
            assert!(!error.contains("PRIVATE_MARKER_939"), "{error}");
        }
        for key in [
            "openrouter_provider",
            "model",
            "openrouter_provider_x",
            "provider",
            "\"openrouter_provider.not_a_child\"",
            "'openrouter_provider.x'",
        ] {
            assert!(reject_unsupported_key(path, 1, key).is_ok(), "{key}");
        }
        for section in [
            "preset.routed",
            "preset.openrouter_provider",
            "other.openrouter_provider",
            "preset.routed.\"openrouter_provider.x\"",
        ] {
            assert!(
                reject_unsupported_section(path, 1, section).is_ok(),
                "{section}"
            );
        }
    }
}
