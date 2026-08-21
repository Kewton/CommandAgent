use std::io::Write;

use anyhow::Context;
use clap::CommandFactory;
use clap_complete::Shell;
use clap_complete::env::{Bash, Elvish, EnvCompleter, Fish, Powershell, Zsh};
use clap_mangen::Man;

use crate::cli::Cli;

pub(crate) fn write_completions(shell: Shell, writer: &mut dyn Write) -> anyhow::Result<()> {
    let executable = std::env::current_exe()
        .context("failed to locate the commandagent executable for dynamic completions")?;
    let executable = executable.to_string_lossy();
    let completer: &dyn EnvCompleter = match shell {
        Shell::Bash => &Bash,
        Shell::Elvish => &Elvish,
        Shell::Fish => &Fish,
        Shell::PowerShell => &Powershell,
        Shell::Zsh => &Zsh,
        _ => anyhow::bail!("unsupported completion shell {shell}"),
    };
    completer
        .write_registration(
            "COMPLETE",
            "commandagent",
            "commandagent",
            &executable,
            writer,
        )
        .context("failed to generate commandagent shell completions")?;
    writer
        .flush()
        .context("failed to write commandagent shell completions")
}

pub(crate) fn write_man_page(writer: &mut dyn Write) -> std::io::Result<()> {
    Man::new(Cli::command()).render(writer)
}
