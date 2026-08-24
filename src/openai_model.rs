//! OpenAI model-identity policy used by configuration and doctor checks.

use anyhow::bail;

pub(crate) const GPT_5_6_TERRA: &str = "gpt-5.6-terra";
const GPT_5_6_ALIAS: &str = "gpt-5.6";
const GPT_5_6_MODELS: [&str; 3] = ["gpt-5.6-luna", GPT_5_6_TERRA, "gpt-5.6-sol"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModelIdentity<'a> {
    pub(crate) family_id: &'a str,
    pub(crate) snapshot_pinned: bool,
}

pub(crate) fn validate_strict_id(model: &str, role: &str) -> anyhow::Result<()> {
    if model == GPT_5_6_ALIAS {
        bail!(
            "OpenAI {role} model alias `gpt-5.6` is ambiguous. Specify an exact Luna, Terra, or Sol model ID (`gpt-5.6-luna`, `gpt-5.6-terra`, or `gpt-5.6-sol`) or a provider-published snapshot-qualified ID."
        );
    }
    if model.starts_with(GPT_5_6_TERRA)
        && model != GPT_5_6_TERRA
        && !is_snapshot_of(model, GPT_5_6_TERRA)
    {
        bail!(
            "OpenAI {role} Terra model `{model}` is not an exact model ID or a date-qualified snapshot. Use `gpt-5.6-terra` or a provider-published `gpt-5.6-terra-YYYY-MM-DD` snapshot."
        );
    }
    Ok(())
}

pub(crate) fn identity(model: &str) -> Option<ModelIdentity<'_>> {
    GPT_5_6_MODELS.iter().find_map(|family_id| {
        if model == *family_id {
            Some(ModelIdentity {
                family_id,
                snapshot_pinned: false,
            })
        } else if is_snapshot_of(model, family_id) {
            Some(ModelIdentity {
                family_id,
                snapshot_pinned: true,
            })
        } else {
            None
        }
    })
}

fn is_snapshot_of(model: &str, family_id: &str) -> bool {
    let Some(date) = model
        .strip_prefix(family_id)
        .and_then(|tail| tail.strip_prefix('-'))
    else {
        return false;
    };
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terra_accepts_exact_and_date_snapshot_ids_only() {
        for model in ["gpt-5.6-terra", "gpt-5.6-terra-2026-08-18"] {
            validate_strict_id(model, "executor").unwrap();
        }
        for model in [
            "gpt-5.6-terra-latest",
            "gpt-5.6-terra-2026-8-18",
            "gpt-5.6-terra-2026-08-18-extra",
        ] {
            assert!(validate_strict_id(model, "executor").is_err(), "{model}");
        }
    }

    #[test]
    fn family_alias_is_rejected_for_every_role() {
        for role in ["executor", "planner"] {
            let error = validate_strict_id("gpt-5.6", role).unwrap_err().to_string();
            assert!(error.contains(role), "{error}");
            assert!(error.contains(GPT_5_6_TERRA), "{error}");
        }
    }

    #[test]
    fn identity_distinguishes_exact_id_from_snapshot_pin() {
        assert_eq!(
            identity(GPT_5_6_TERRA),
            Some(ModelIdentity {
                family_id: GPT_5_6_TERRA,
                snapshot_pinned: false,
            })
        );
        assert!(
            identity("gpt-5.6-terra-2026-08-18")
                .unwrap()
                .snapshot_pinned
        );
        assert!(identity("gpt-4.1").is_none());
    }
}
