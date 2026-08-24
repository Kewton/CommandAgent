use std::ffi::{OsStr, OsString};

use clap::builder::{EnumValueParser, PossibleValue, TypedValueParser};
use clap::{Arg, CommandFactory, FromArgMatches, ValueEnum};

use crate::cli::{Cli, ProviderArg};
use crate::config::ProviderCliOptions;

const OPENAI_COMPATIBLE: &str = "openai-compatible";

#[derive(Debug, Clone)]
pub struct ParsedCli {
    pub cli: Cli,
    pub provider_options: ProviderCliOptions,
}

#[derive(Clone, Debug)]
struct ProviderValueParser;

impl TypedValueParser for ProviderValueParser {
    type Value = ProviderArg;

    fn parse_ref(
        &self,
        command: &clap::Command,
        argument: Option<&clap::Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        if value == OPENAI_COMPATIBLE {
            return Ok(ProviderArg::LmStudio);
        }
        EnumValueParser::<ProviderArg>::new().parse_ref(command, argument, value)
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        let mut values = ProviderArg::value_variants()
            .iter()
            .filter_map(clap::ValueEnum::to_possible_value)
            .collect::<Vec<_>>();
        values.push(PossibleValue::new(OPENAI_COMPATIBLE));
        Some(Box::new(values.into_iter()))
    }
}

pub fn command() -> clap::Command {
    Cli::command()
        .mut_arg("provider", |argument| {
            argument.value_parser(ProviderValueParser)
        })
        .mut_arg("planner_provider", |argument| {
            argument.value_parser(ProviderValueParser)
        })
        .arg(
            Arg::new("base_url")
                .long("base-url")
                .value_name("URL")
                .help_heading("Models and Providers")
                .help("Set the base URL for the generic OpenAI-compatible provider; an optional trailing `/v1` is normalized."),
        )
        .arg(
            Arg::new("api_key_env")
                .long("api-key-env")
                .value_name("NAME")
                .help_heading("Models and Providers")
                .help("Read an optional OpenAI-compatible bearer token from this process-environment variable."),
        )
}

pub fn parse() -> ParsedCli {
    parse_from(std::env::args_os()).unwrap_or_else(|error| error.exit())
}

pub fn parse_from<I, T>(arguments: I) -> Result<ParsedCli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
    let mut matches = command().try_get_matches_from(arguments)?;
    let executor_openai_compatible = matches_provider(&matches, "provider", OPENAI_COMPATIBLE);
    let planner_openai_compatible =
        matches_provider(&matches, "planner_provider", OPENAI_COMPATIBLE);
    let base_url = matches.remove_one::<String>("base_url");
    let api_key_env = matches.remove_one::<String>("api_key_env");
    let cli = Cli::from_arg_matches_mut(&mut matches)?;
    Ok(ParsedCli {
        cli,
        provider_options: ProviderCliOptions {
            executor_openai_compatible,
            planner_openai_compatible,
            base_url,
            api_key_env,
        },
    })
}

fn matches_provider(matches: &clap::ArgMatches, id: &str, expected: &str) -> bool {
    matches
        .get_raw(id)
        .and_then(|mut values| values.next_back())
        .is_some_and(|value| value == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generic_provider_and_leaf_arguments_without_changing_cli_type() {
        let parsed = parse_from([
            "commandagent",
            "--provider",
            OPENAI_COMPATIBLE,
            "--planner-provider=openai-compatible",
            "--base-url",
            "http://127.0.0.1:8000/v1",
            "--api-key-env",
            "VLLM_API_KEY",
        ])
        .unwrap();

        assert_eq!(parsed.cli.provider, Some(ProviderArg::LmStudio));
        assert_eq!(parsed.cli.planner_provider, Some(ProviderArg::LmStudio));
        assert!(parsed.provider_options.executor_openai_compatible);
        assert!(parsed.provider_options.planner_openai_compatible);
        assert_eq!(
            parsed.provider_options.base_url.as_deref(),
            Some("http://127.0.0.1:8000/v1")
        );
        assert_eq!(
            parsed.provider_options.api_key_env.as_deref(),
            Some("VLLM_API_KEY")
        );
    }

    #[test]
    fn augmented_help_lists_generic_provider_arguments() {
        let help = command().render_long_help().to_string();
        for expected in [OPENAI_COMPATIBLE, "--base-url", "--api-key-env"] {
            assert!(help.contains(expected), "missing {expected} in {help}");
        }
    }
}
