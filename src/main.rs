use clap::Parser;

fn main() {
    let cli = commandagent::cli::Cli::parse();
    if let Err(err) = commandagent::run(cli) {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
