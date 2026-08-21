use clap::{CommandFactory, Parser};

fn main() {
    clap_complete::CompleteEnv::with_factory(commandagent::cli::Cli::command).complete();
    let cli = commandagent::cli::Cli::parse();
    if let Err(err) = commandagent::run(cli) {
        eprintln!("error: {err:#}");
        std::process::exit(commandagent::cli_error_exit_code(&err));
    }
}
