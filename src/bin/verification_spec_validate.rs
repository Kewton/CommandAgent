//! Evaluation-only stdin/stdout adapter for the VerificationSpec v0 parser.

use std::io::{self, Read};

use anyhow::{Context, Result};
use commandagent::verification_spec::{VerificationIntent, parse_provider_spec};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    goal: String,
    intent: String,
    raw: String,
}

#[derive(Serialize)]
struct Response {
    valid: bool,
    spec: Option<commandagent::verification_spec::VerificationSpec>,
    errors: Vec<String>,
}

fn main() -> Result<()> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("read validation request from stdin")?;
    let request: Request = serde_json::from_str(&input).context("parse validation request")?;
    let intent = match request.intent.as_str() {
        "create" => VerificationIntent::Create,
        "fix" => VerificationIntent::Fix,
        "investigate" => VerificationIntent::Investigate,
        value => anyhow::bail!("unsupported intent: {value}"),
    };
    let response = match parse_provider_spec(&request.goal, intent, &request.raw) {
        Ok(spec) => Response {
            valid: true,
            spec: Some(spec),
            errors: Vec::new(),
        },
        Err(error) => Response {
            valid: false,
            spec: None,
            errors: error.codes,
        },
    };
    serde_json::to_writer(io::stdout(), &response).context("write validation response")?;
    Ok(())
}
