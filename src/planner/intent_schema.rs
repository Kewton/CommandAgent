//! Strict IntentSchema configuration. YAML declares structure only; evidence
//! normalization, checkpoints, material text, and adjudication remain Rust.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IntentSchema {
    pub intent: String,
    pub phases: Vec<Phase>,
    pub evidence: Vec<String>,
    pub assurance: Assurance,
}
#[derive(Debug, Deserialize)]
pub struct Phase {
    pub id: String,
    pub role: String,
    pub owns_outputs: bool,
    pub verify_binding: String,
    pub material_injection: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Assurance {
    pub full: String,
    pub failed: String,
}
pub fn load() -> anyhow::Result<IntentSchema> {
    load_raw(
        include_str!("../../intents/investigate.yaml"),
        "investigate",
        &["reproduce-candidate", "diagnose", "bind-verify"],
    )
}
pub fn load_fix() -> anyhow::Result<IntentSchema> {
    load_raw(
        include_str!("../../intents/fix.yaml"),
        "fix",
        &[
            "reproduce-before",
            "isolate-cause",
            "implement-fix",
            "verify-after",
        ],
    )
}
fn load_raw(raw: &str, expected: &str, expected_ids: &[&str]) -> anyhow::Result<IntentSchema> {
    let schema: IntentSchema = serde_yaml::from_str(raw)?;
    if schema.intent != expected
        || schema
            .phases
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>()
            .as_slice()
            != expected_ids
    {
        anyhow::bail!("invalid investigate IntentSchema phase contract");
    }
    let roles = ["reproduce", "isolate", "implement", "verify", "bind"];
    for p in &schema.phases {
        if !roles.contains(&p.role.as_str()) {
            anyhow::bail!("unknown IntentSchema role: {}", p.role);
        }
        if p.verify_binding.is_empty()
            || (p.owns_outputs && p.material_injection.is_none() && p.id == "diagnose")
        {
            anyhow::bail!("incomplete IntentSchema phase: {}", p.id);
        }
    }
    if schema.evidence.is_empty()
        || schema.assurance.full.is_empty()
        || schema.assurance.failed.is_empty()
    {
        anyhow::bail!("incomplete IntentSchema assurance/evidence");
    }
    Ok(schema)
}
#[cfg(test)]
mod tests {
    use super::{load, load_fix};
    #[test]
    fn investigate_schema_is_strict_and_complete() {
        let s = load().expect("embedded schema");
        assert_eq!(s.phases.len(), 3);
        assert!(s.evidence.contains(&"I2".to_string()));
    }
    #[test]
    fn fix_schema_is_strict_and_complete() {
        let s = load_fix().expect("embedded schema");
        assert_eq!(s.phases.len(), 4);
        assert!(s.evidence.contains(&"F1".to_string()));
    }
}
