//! The common system guidance of a direct `--prompt` run (Issue #925).
//!
//! `COMMANDAGENT_SYSTEM_GUIDANCE` is read once from the process environment
//! when a direct prompt run starts. An unset or empty value adds nothing, so the
//! request stays byte-identical. Any other value must be UTF-8, at most 8 KiB,
//! and not only whitespace; otherwise the run fails with the variable name and
//! a short reason, never the value. The text is ordinary guidance: it joins the
//! request guidance block that every provider request of the run rebuilds, and
//! grants or removes no tool, completion, or evidence rule. No config file or
//! dotenv is read for it.

use std::ffi::OsStr;

use anyhow::{Result, bail};

pub const SYSTEM_GUIDANCE_ENV: &str = "COMMANDAGENT_SYSTEM_GUIDANCE";

/// The largest accepted value, in UTF-8 bytes.
pub const MAX_BYTES: usize = 8 * 1024;

/// Separates an adopted request pattern's guidance from the common guidance.
pub const COMMON_HEADING: &str = "\n\nCommon guidance:\n";

/// Reads the variable from the process environment.
pub fn from_env() -> Result<Option<String>> {
    parse(std::env::var_os(SYSTEM_GUIDANCE_ENV).as_deref())
}

/// Checks one value. `None` and the empty string both mean "not set".
pub fn parse(value: Option<&OsStr>) -> Result<Option<String>> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some(text) = value.to_str() else {
        bail!("{SYSTEM_GUIDANCE_ENV} is not valid UTF-8");
    };
    if text.len() > MAX_BYTES {
        bail!("{SYSTEM_GUIDANCE_ENV} is longer than 8 KiB");
    }
    if text.trim().is_empty() {
        bail!("{SYSTEM_GUIDANCE_ENV} contains only whitespace");
    }
    Ok(Some(text.to_string()))
}

/// The request guidance of the run: the adopted pattern's block, the common
/// guidance, both (pattern first, then the common heading), or nothing.
pub fn compose(pattern: Option<&str>, common: Option<&str>) -> Option<String> {
    match (pattern, common) {
        (pattern, None) => pattern.map(str::to_string),
        (None, Some(common)) => Some(common.to_string()),
        (Some(pattern), Some(common)) => Some(format!("{pattern}{COMMON_HEADING}{common}")),
    }
}

#[cfg(test)]
mod tests;
