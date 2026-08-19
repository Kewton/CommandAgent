use clap::Parser;

fn main() {
    let cli = commandagent::cli::Cli::parse();
    if let Err(err) = commandagent::run(cli) {
        eprintln!("error: {err:#}");
        std::process::exit(commandagent::cli_error_exit_code(&err));
    }
}
