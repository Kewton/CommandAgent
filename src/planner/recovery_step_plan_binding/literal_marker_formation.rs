//! Closed file-marker strengthening grammar, not a JavaScript equivalence engine.
//! Regexes qualify only when their entire pattern denotes one literal substring.
use super::*;
use crate::minimal_loop::evidence::import_check;
use crate::planner::recovery_contract_authority::{inline_admission, verifier_obligations};

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct Obligation {
    pub original_command: String,
    pub normalized_command: String,
    pub original_step: String,
    pub original_step_index: usize,
    pub expected_result: String,
    pub predicates: Predicates,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct Predicates {
    target: String,
    // Order and diagnostic messages are part of the retained contract.
    checks: Vec<(String, String)>,
    stdout: String,
}

pub(super) fn capture(
    config: &Config,
    contract: &CompletionContract,
    plan: &StepPlan,
) -> anyhow::Result<Vec<Obligation>> {
    let admitted = verifier_obligations::admitted_commands(config, contract, plan)?;
    let mut obligations = Vec::new();
    for (index, step) in plan.steps.iter().enumerate() {
        if step.expected_result != "pass" {
            continue;
        }
        for command in &step.verify {
            let Ok(normalized) = crate::planner::verify::normalize_planner_verify_command(command)
            else {
                continue;
            };
            // Existing normalization can split compounds. Projection does not
            // prove compound equivalence; retain those via existing scope rules.
            let [normalized] = normalized.as_slice() else {
                continue;
            };
            if !admitted.contains(normalized)
                || inline_admission::refusal(config, contract, normalized)?.is_none()
            {
                continue;
            }
            if let Some((predicates, _)) = parse(normalized) {
                obligations.push(Obligation {
                    original_command: command.clone(),
                    normalized_command: normalized.clone(),
                    original_step: step.id.clone(),
                    original_step_index: index,
                    expected_result: step.expected_result.clone(),
                    predicates,
                });
            }
        }
    }
    Ok(obligations)
}

pub(super) fn matches(obligation: &Obligation, candidate: &str) -> bool {
    parse(candidate)
        .is_some_and(|(predicates, direct)| direct && predicates == obligation.predicates)
}

pub(super) fn preserve_raw_commands(
    obligations: &[Obligation],
    model: &StepPlan,
) -> anyhow::Result<()> {
    for command in model.steps.iter().flat_map(|s| &s.verify) {
        let normalized =
            crate::planner::verify::normalize_planner_verify_command(command).unwrap_or_default();
        if obligations
            .iter()
            .any(|o| !matches(o, command) && normalized.iter().any(|c| matches(o, c)))
        {
            return Err(crate::planner::recovery_inspection::verifier_obligations::failure(
                crate::planner::recovery_inspection::verifier_obligations::FailureClass::ProposalRepairable,
                "literal-marker replacement must explicitly preserve the predicates, stdout and failure propagation without shell wrappers",
            ));
        }
    }
    Ok(())
}

pub(super) fn guidance(obligations: &[Obligation]) -> String {
    if obligations.is_empty() {
        return String::new();
    }
    format!(
        "\nThe current classifier cannot recognize the original failure path (an assert may already be present). For the captured literal file-marker checks only, explicitly propose direct assert(s.includes(literal),message) checks with identical file, ordered conditions, messages and console.log output; remove swallowing catches. This is a text check, not proof of application behavior. Preserve all owner/output duties and other gates. Saved draft obligations:\n{}",
        serde_json::to_string(obligations).expect("serializable marker obligations")
    )
}

fn parse(command: &str) -> Option<(Predicates, bool)> {
    let source = import_check::source(command)?;
    let (assert_bound, rest) =
        match source.strip_prefix("const assert=require('node:assert/strict');") {
            Some(rest) => (true, rest),
            None => (false, source.as_str()),
        };
    let rest = rest.strip_prefix("const s=require('fs').readFileSync('")?;
    let (target, mut rest) = rest.split_once("','utf8');")?;
    // No escape or computed path; path spelling is retained exactly.
    if target.is_empty()
        || !target
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-[]".contains(&b))
    {
        return None;
    }
    verifier_obligations::normalized(target).ok()?;
    let mut checks = Vec::new();
    let mut direct = true;
    let stdout;
    loop {
        rest = rest.trim_start();
        if let Some(tail) = rest.strip_prefix("console.log(") {
            let (value, tail) = quoted(tail)?;
            if !matches!(tail, ")" | ");") || checks.is_empty() {
                return None;
            }
            stdout = value;
            break;
        }
        let (literal, message, tail, is_direct) = if let Some(tail) = rest.strip_prefix("if(!/") {
            let (pattern, tail) = tail.split_once("/.test(s))throw new Error(")?;
            let (message, tail) = quoted(tail)?;
            (
                regex_literal(pattern)?,
                message,
                tail.strip_prefix(");")?,
                false,
            )
        } else {
            if !assert_bound {
                return None;
            }
            let (swallowed, tail) = rest
                .strip_prefix("try{")
                .map_or((false, rest), |r| (true, r));
            let (literal, tail, includes) = if let Some(tail) =
                tail.strip_prefix("assert(s.includes(")
            {
                let (literal, tail) = quoted(tail)?;
                (literal, tail.strip_prefix("),")?, true)
            } else {
                let (pattern, tail) = tail.strip_prefix("assert(/")?.split_once("/.test(s),")?;
                (regex_literal(pattern)?, tail, false)
            };
            let (message, tail) = quoted(tail)?;
            let tail = tail.strip_prefix(");")?;
            let tail = if swallowed {
                tail.strip_prefix("}catch(error){};")?
            } else {
                tail
            };
            (literal, message, tail, includes && !swallowed)
        };
        if literal.is_empty() {
            return None;
        }
        checks.push((literal, message));
        direct &= is_direct;
        rest = tail;
    }
    Some((
        Predicates {
            target: target.into(),
            checks,
            stdout,
        },
        direct,
    ))
}

/// Intentionally closed ASCII JS string grammar; no escapes/interpolation or
/// embedded executable fragments. Shell quoting is decoded by the final parser.
fn quoted(source: &str) -> Option<(String, &str)> {
    let quote = source.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let (value, tail) = source[1..].split_once(quote)?;
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b" _-=:./\"[]".contains(&b))
    {
        return None;
    }
    Some((value.into(), tail))
}

fn regex_literal(pattern: &str) -> Option<String> {
    let mut literal = String::new();
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let escaped = chars.next()?;
            if !".[](){}*+?^$|/\\\"".contains(escaped) {
                return None;
            }
            literal.push(escaped);
        } else if ch.is_ascii_alphanumeric() || " _-=: \"".contains(ch) {
            literal.push(ch);
        } else {
            return None;
        }
    }
    Some(literal)
}
