//! The generic request/result envelope of `artifact_contract_check`.
//!
//! The host understands only protocol names, request/check/target IDs, input
//! digests, statuses, counts, and bounded diagnostics. Check and target IDs are
//! opaque: no field name, ID rule, or domain verdict of any adapter is
//! interpreted here.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub(crate) const REQUEST_PROTOCOL: &str = "artifact-contract.request/v1";
pub(crate) const ENVELOPE_PROTOCOL: &str = "artifact-contract.envelope/v1";
pub(crate) const RESULT_PROTOCOL: &str = "artifact-contract.result/v1";

pub(crate) const MAX_REQUEST_BYTES: usize = 16_384;
pub(crate) const MAX_STREAM_BYTES: usize = 24_000;
pub(crate) const MAX_JSON_DEPTH: usize = 16;
pub(crate) const MAX_ROWS: usize = 512;
pub(crate) const MAX_INPUTS: usize = 32;
pub(crate) const MAX_DIAGNOSTICS_PER_ROW: usize = 16;
pub(crate) const MAX_DIAGNOSTICS_TOTAL: usize = 64;
pub(crate) const MAX_DIAGNOSTIC_CODE_BYTES: usize = 64;
pub(crate) const MAX_DIAGNOSTIC_MESSAGE_BYTES: usize = 512;
const MAX_SLUG_BYTES: usize = 64;
const MAX_OPAQUE_ID_BYTES: usize = 128;
const MAX_VIOLATIONS: usize = 16;

/// The domain verdict ladder. `Full` exists only so the cap is explicit: every
/// verdict leaves this module through [`cap_domain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DomainVerdict {
    None,
    Partial,
    Full,
}

impl DomainVerdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Partial => "partial",
            Self::Full => "full",
        }
    }
}

/// First-version host cap: a domain outcome never exceeds `partial`.
pub(crate) const DOMAIN_CAP: DomainVerdict = DomainVerdict::Partial;

pub(crate) fn cap_domain(verdict: DomainVerdict) -> DomainVerdict {
    verdict.min(DOMAIN_CAP)
}

/// A row key: an opaque check ID plus an optional opaque target ID.
pub(crate) type RowKey = (String, Option<String>);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub(crate) protocol: String,
    pub(crate) request_id: String,
    pub(crate) inputs: Vec<RequestInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestInput {
    pub(crate) input_id: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Envelope<'a> {
    pub(crate) protocol: &'static str,
    pub(crate) request_id: &'a str,
    pub(crate) nonce: &'a str,
    pub(crate) workspace: &'a str,
    pub(crate) inputs: Vec<EnvelopeInput<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EnvelopeInput<'a> {
    pub(crate) input_id: &'a str,
    pub(crate) path: &'a str,
    pub(crate) sha256: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdapterResult {
    pub(crate) protocol: String,
    pub(crate) request_id: String,
    pub(crate) nonce: String,
    pub(crate) input_digests: Vec<InputDigest>,
    pub(crate) checks: Vec<CheckRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InputDigest {
    pub(crate) input_id: String,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckRow {
    pub(crate) check_id: String,
    pub(crate) target_id: Option<String>,
    pub(crate) status: RowStatus,
    pub(crate) counts: Counts,
    #[serde(default)]
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RowStatus {
    Passed,
    Failed,
    Unresolved,
    NotRun,
    NotProvided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Counts {
    pub(crate) expected: Option<u64>,
    pub(crate) executed: u64,
    pub(crate) passed: u64,
    pub(crate) failed: u64,
    pub(crate) unresolved: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Diagnostic {
    pub(crate) code: String,
    pub(crate) message: String,
}

/// What the host expects a result to be bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expectation {
    pub(crate) request_id: String,
    pub(crate) nonce: String,
    pub(crate) input_digests: BTreeMap<String, String>,
    pub(crate) required: BTreeSet<RowKey>,
    pub(crate) optional: BTreeSet<RowKey>,
}

/// The generic facts the host derives from a validated result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RowFacts {
    pub(crate) rows: usize,
    pub(crate) required_passed: usize,
    pub(crate) known_mismatch: bool,
    pub(crate) required_unresolved: bool,
    pub(crate) required_not_run: bool,
    pub(crate) optional_unverified: bool,
    /// Registered optional rows the adapter did not report at all. They count
    /// as unverified, exactly like an explicit `not_provided`.
    pub(crate) optional_missing: usize,
}

impl RowFacts {
    /// Native acceptance holds only when every required row passed and no row
    /// reports a known mismatch.
    pub(crate) fn native_passed(&self) -> bool {
        !self.known_mismatch && !self.required_unresolved && !self.required_not_run
    }

    /// The uncapped domain verdict. A known mismatch is `none` even when
    /// unresolved rows coexist; an unexecuted required row is never success.
    pub(crate) fn uncapped_verdict(&self) -> DomainVerdict {
        if self.known_mismatch || self.required_not_run {
            DomainVerdict::None
        } else if self.required_unresolved || self.optional_unverified {
            DomainVerdict::Partial
        } else {
            DomainVerdict::Full
        }
    }
}

pub(crate) fn validate_slug(value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= MAX_SLUG_BYTES
        && value
            .bytes()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "must be a slug of 1..{MAX_SLUG_BYTES} lowercase ASCII letters, digits, `-`, `_`, or `.`"
        ))
    }
}

pub(crate) fn validate_opaque_id(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= MAX_OPAQUE_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\\'));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{label} must be 1..{MAX_OPAQUE_ID_BYTES} printable ASCII bytes without quotes or backslashes"
        ))
    }
}

pub(crate) fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!("{label} must be 64 lowercase hex digits"))
    }
}

pub(crate) fn parse_request(bytes: &[u8]) -> Result<Request, String> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(format!("request exceeds {MAX_REQUEST_BYTES} bytes"));
    }
    check_json_depth(bytes)?;
    let request: Request =
        serde_json::from_slice(bytes).map_err(|error| format!("request is invalid: {error}"))?;
    if request.protocol != REQUEST_PROTOCOL {
        return Err(format!("request protocol must be `{REQUEST_PROTOCOL}`"));
    }
    validate_opaque_id("request_id", &request.request_id)?;
    if request.inputs.is_empty() || request.inputs.len() > MAX_INPUTS {
        return Err(format!("request must name 1..{MAX_INPUTS} inputs"));
    }
    let mut seen = BTreeSet::new();
    for input in &request.inputs {
        validate_opaque_id("input_id", &input.input_id)?;
        if !seen.insert(input.input_id.as_str()) {
            return Err(format!("request repeats input `{}`", input.input_id));
        }
    }
    Ok(request)
}

/// Parse adapter stdout. Bytes, depth, and shape are bounded before any
/// semantic check runs.
pub(crate) fn parse_result(stdout: &[u8]) -> Result<AdapterResult, String> {
    if stdout.len() > MAX_STREAM_BYTES {
        return Err(format!("result exceeds {MAX_STREAM_BYTES} bytes"));
    }
    check_json_depth(stdout)?;
    serde_json::from_slice(stdout)
        .map_err(|error| format!("result is not a valid envelope: {error}"))
}

/// Validate a parsed result against the host expectation and the observed exit
/// code. Every violation is reported; none is repaired.
pub(crate) fn validate_result(
    result: &AdapterResult,
    expectation: &Expectation,
    exit_code: i32,
) -> Result<RowFacts, Vec<String>> {
    let mut violations = Vec::new();
    if result.protocol != RESULT_PROTOCOL {
        violations.push(format!("result protocol must be `{RESULT_PROTOCOL}`"));
    }
    if result.request_id != expectation.request_id {
        violations.push("result is bound to a different request_id".to_string());
    }
    if result.nonce != expectation.nonce {
        violations.push("result is bound to a different run nonce".to_string());
    }
    let mut digests = BTreeMap::new();
    for digest in &result.input_digests {
        if digests
            .insert(digest.input_id.clone(), digest.sha256.clone())
            .is_some()
        {
            violations.push(format!("result repeats input digest `{}`", digest.input_id));
        }
    }
    if digests != expectation.input_digests {
        violations.push("result input digests differ from the host-observed inputs".to_string());
    }
    let facts = validate_rows(result, expectation, &mut violations);
    let expected_exit = if facts.native_passed() { 0 } else { 1 };
    if exit_code != expected_exit {
        violations.push(format!(
            "adapter exit code {exit_code} disagrees with its reported rows (expected {expected_exit})"
        ));
    }
    if violations.is_empty() {
        Ok(facts)
    } else {
        violations.truncate(MAX_VIOLATIONS);
        Err(violations)
    }
}

fn validate_rows(
    result: &AdapterResult,
    expectation: &Expectation,
    violations: &mut Vec<String>,
) -> RowFacts {
    let mut facts = RowFacts {
        rows: result.checks.len(),
        ..RowFacts::default()
    };
    if result.checks.len() > MAX_ROWS {
        violations.push(format!("result has more than {MAX_ROWS} rows"));
        return facts;
    }
    let mut seen = BTreeSet::new();
    let mut diagnostics_total = 0usize;
    for row in &result.checks {
        let key = (row.check_id.clone(), row.target_id.clone());
        let label = row_label(&key);
        if let Err(reason) = validate_opaque_id("check_id", &row.check_id) {
            violations.push(reason);
        }
        if let Some(Err(reason)) = row
            .target_id
            .as_deref()
            .map(|target| validate_opaque_id("target_id", target))
        {
            violations.push(reason);
        }
        if !seen.insert(key.clone()) {
            violations.push(format!("row {label} is reported more than once"));
            continue;
        }
        let required = expectation.required.contains(&key);
        if !required && !expectation.optional.contains(&key) {
            violations.push(format!("row {label} is not registered for this adapter"));
            continue;
        }
        if let Err(reason) = counts_agree(row.status, row.counts) {
            violations.push(format!("row {label}: {reason}"));
        }
        diagnostics_total += row.diagnostics.len();
        if row.diagnostics.len() > MAX_DIAGNOSTICS_PER_ROW
            || row.diagnostics.iter().any(|diagnostic| {
                diagnostic.code.is_empty()
                    || diagnostic.code.len() > MAX_DIAGNOSTIC_CODE_BYTES
                    || diagnostic.message.len() > MAX_DIAGNOSTIC_MESSAGE_BYTES
            })
        {
            violations.push(format!("row {label}: diagnostics exceed their bounds"));
        }
        match (row.status, required) {
            (RowStatus::Failed, _) => facts.known_mismatch = true,
            (RowStatus::Passed, true) => facts.required_passed += 1,
            (RowStatus::Unresolved, true) => facts.required_unresolved = true,
            (RowStatus::NotRun, true) => facts.required_not_run = true,
            (RowStatus::NotProvided, true) => {
                violations.push(format!("required row {label} may not be not_provided"));
            }
            (RowStatus::Unresolved | RowStatus::NotRun | RowStatus::NotProvided, false) => {
                facts.optional_unverified = true;
            }
            (RowStatus::Passed, false) => {}
        }
    }
    if diagnostics_total > MAX_DIAGNOSTICS_TOTAL {
        violations.push(format!(
            "result has more than {MAX_DIAGNOSTICS_TOTAL} diagnostics"
        ));
    }
    for missing in expectation.required.difference(&seen) {
        violations.push(format!("required row {} is missing", row_label(missing)));
    }
    facts.optional_missing = expectation.optional.difference(&seen).count();
    if facts.optional_missing > 0 {
        facts.optional_unverified = true;
    }
    facts
}

/// The count identity: executed = passed + failed + unresolved, bounded by
/// `expected`, and consistent with the row status. An unexecuted row can never
/// stand for a success.
pub(crate) fn counts_agree(status: RowStatus, counts: Counts) -> Result<(), String> {
    let total = counts
        .passed
        .checked_add(counts.failed)
        .and_then(|sum| sum.checked_add(counts.unresolved));
    if total != Some(counts.executed) {
        return Err("executed does not equal passed + failed + unresolved".to_string());
    }
    if counts
        .expected
        .is_some_and(|expected| counts.executed > expected)
    {
        return Err("executed exceeds expected".to_string());
    }
    let complete = counts
        .expected
        .is_none_or(|expected| counts.executed == expected);
    let consistent = match status {
        RowStatus::Passed => counts.executed > 0 && counts.passed == counts.executed && complete,
        RowStatus::Failed => counts.failed > 0,
        RowStatus::Unresolved => {
            counts.failed == 0 && counts.executed > 0 && (counts.unresolved > 0 || !complete)
        }
        RowStatus::NotRun | RowStatus::NotProvided => counts.executed == 0,
    };
    if consistent {
        Ok(())
    } else {
        Err("status disagrees with its counts".to_string())
    }
}

fn row_label(key: &RowKey) -> String {
    match &key.1 {
        Some(target) => format!("`{}`/`{target}`", key.0),
        None => format!("`{}`", key.0),
    }
}

/// Reject JSON nested deeper than [`MAX_JSON_DEPTH`] before deserializing it.
pub(crate) fn check_json_depth(bytes: &[u8]) -> Result<(), String> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return Err(format!("JSON nesting exceeds depth {MAX_JSON_DEPTH}"));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}
