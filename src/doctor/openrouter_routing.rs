//! `config.openrouter_provider_routing`: the preset's `openrouter_provider`
//! per real provider role (Issue #939). The check sends no request and asks no
//! server anything; it reports the `provider` object CommandAgent would send,
//! never that the server supports or honoured it. A rejected value never
//! reaches this check: an invalid preset fails in config validation first.

use serde_json::{Value, json};

use crate::config::openrouter_routing::OPENROUTER_PROVIDER_KEY;
use crate::config::{Config, ProviderRole};
use crate::providers::compatible_effort::role_name;

use super::{CheckStatus, DoctorCheck};

const ROLES: [ProviderRole; 3] = [
    ProviderRole::Executor,
    ProviderRole::Planner,
    ProviderRole::Classifier,
];

pub(super) fn routing_check(config: &Config) -> DoctorCheck {
    let compatible = config.openai_compatible.as_ref();
    let resolved = compatible.and_then(|compatible| compatible.openrouter_provider());
    let setting = match resolved {
        None => "omitted",
        Some(resolved) if resolved.is_declared() => "declared",
        Some(_) => "disabled",
    };
    let requested = resolved
        .filter(|resolved| resolved.is_declared())
        .map(|resolved| resolved.routing.provider_object());
    let source = resolved.map(|resolved| resolved.source.as_str());
    let applies =
        |role: ProviderRole| compatible.is_some_and(|compatible| compatible.applies_to(role));
    let roles = ROLES
        .iter()
        .map(|role| {
            let applies = applies(*role);
            json!({
                "role": role_name(*role),
                "provider": config.provider_label(*role),
                "status": if applies { setting } else { "not_applicable" },
                "requested_provider": if applies { requested.clone() } else { None },
                "source": if applies { source } else { None },
            })
        })
        .collect::<Vec<_>>();
    let applicable_roles = ROLES
        .iter()
        .filter(|role| applies(**role))
        .map(|role| role_name(*role))
        .collect::<Vec<_>>();
    let status = if applicable_roles.is_empty() {
        "not_applicable"
    } else {
        setting
    };
    let role_list = applicable_roles.join(", ");
    let message = match status {
        "declared" => format!(
            "{} (declared by {}; provider object sent for {role_list}; server support not verified)",
            requested.as_ref().map(Value::to_string).unwrap_or_default(),
            source.unwrap_or_default()
        ),
        "disabled" => format!(
            "disabled by {} (provider key absent for {role_list})",
            source.unwrap_or_default()
        ),
        "omitted" => format!("omitted (provider key absent for {role_list})"),
        _ => "not applicable (no openai-compatible role)".to_string(),
    };
    DoctorCheck::new(
        "config.openrouter_provider_routing",
        "configuration",
        "OpenRouter provider routing",
        CheckStatus::Pass,
        message,
        None,
        json!({
            "preset_key": OPENROUTER_PROVIDER_KEY,
            "status": status,
            "declared": status == "declared",
            "requested_provider": if status == "declared" { requested } else { None },
            "source": if status == "not_applicable" { None } else { source },
            "applicable_roles": applicable_roles,
            "roles": roles,
            "server_support_verified": false,
        }),
    )
}
