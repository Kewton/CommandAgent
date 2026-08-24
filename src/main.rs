fn main() {
    clap_complete::CompleteEnv::with_factory(commandagent::provider_cli::command).complete();
    let parsed = commandagent::provider_cli::parse();
    if let Err(err) = commandagent::run_with_provider_options(parsed.cli, parsed.provider_options) {
        eprintln!("error: {err:#}");
        std::process::exit(commandagent::cli_error_exit_code(&err));
    }
}
