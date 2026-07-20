use std::io::Write;

use anyhow::Context;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use clap_mangen::Man;

use crate::cli::Cli;

pub(crate) fn write_completions(shell: Shell, writer: &mut dyn Write) -> anyhow::Result<()> {
    let mut command = Cli::command();
    generate(shell, &mut command, "commandagent", writer);
    writer
        .flush()
        .context("failed to write commandagent shell completions")
}

pub(crate) fn write_man_page(writer: &mut dyn Write) -> std::io::Result<()> {
    Man::new(Cli::command()).render(writer)
}
