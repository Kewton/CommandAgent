use std::ffi::OsString;
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();

    if args
        .iter()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        print_help();
        return ExitCode::SUCCESS;
    }

    if args
        .iter()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("commandagent {VERSION}");
        return ExitCode::SUCCESS;
    }

    println!("CommandAgent MVP skeleton");
    println!("Run `commandagent --help` for usage.");
    ExitCode::SUCCESS
}

fn print_help() {
    println!(
        "CommandAgent {VERSION}\n\
\n\
Usage:\n\
  commandagent [OPTIONS]\n\
\n\
Options:\n\
  -h, --help       Print help\n\
  -V, --version    Print version\n\
\n\
CommandAgent is a minimal local-first coding agent. The MVP migration keeps\n\
only the minimal loop, interactive REPL, provider adapters, built-in tools,\n\
and /ultra-plan-run style step execution."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_exits_successfully() {
        assert_eq!(run(["commandagent", "--help"]), ExitCode::SUCCESS);
    }

    #[test]
    fn version_exits_successfully() {
        assert_eq!(run(["commandagent", "--version"]), ExitCode::SUCCESS);
    }
}
