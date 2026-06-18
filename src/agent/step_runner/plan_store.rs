use super::{PlanError, StepPlan, render_step_plan_yaml, validate_step_plan};
use crate::util::workspace_paths::plans_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn save_step_plan(cwd: impl AsRef<Path>, plan: &StepPlan) -> Result<PathBuf, PlanError> {
    validate_step_plan(plan)?;
    let dir = plans_dir(cwd.as_ref());
    fs::create_dir_all(&dir).map_err(|err| PlanError::Io {
        path: dir.clone(),
        message: err.to_string(),
    })?;
    let path = dir.join(format!(
        "plan-{}-{}.yaml",
        now_ms(),
        slug(&plan.goal).unwrap_or_else(|| "step-plan".to_string())
    ));
    fs::write(&path, render_step_plan_yaml(plan)).map_err(|err| PlanError::Io {
        path: path.clone(),
        message: err.to_string(),
    })?;
    Ok(path)
}

fn slug(value: &str) -> Option<String> {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 48 {
            break;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { None } else { Some(out) }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
