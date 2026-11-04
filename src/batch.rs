// Copyright 2026 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

//! Sequential CLI command execution over an already-open device session.

use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use anyhow::Context;
use clap::{CommandFactory, FromArgMatches, Parser};
use mboot::{CommunicationError, protocols::Protocol};

use crate::{Blhost, Commands};

#[cfg(test)]
mod tests;

#[derive(Parser)]
#[command(name = "batch", disable_help_subcommand = true)]
struct BatchCommand {
    #[command(subcommand)]
    command: Commands,
}

fn batch_command() -> clap::Command {
    let mut command = BatchCommand::command();
    crate::configure_help_flags(&mut command);
    command
}

fn split_command_line(line: &str) -> Option<Vec<String>> {
    // Python's shlex (blhost) starts comments at any unquoted '#', whereas Rust's
    // shlex only recognizes them at word boundaries.
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if quote != Some('\'') => escaped = true,
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            '#' if quote.is_none() => return shlex::split(&line[..index]),
            _ => {}
        }
    }
    shlex::split(line)
}

impl<T: Protocol> Blhost<T> {
    pub(super) fn execute_batch(&mut self, command_file: &str) -> anyhow::Result<()> {
        let file = File::open(command_file).with_context(|| format!("Cannot open batch file '{command_file}'"))?;
        let mut parser = batch_command();
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line_number = index + 1;
            let location = format!("Batch {command_file}:{line_number}");
            let line = line.with_context(|| format!("{location}: cannot read command"))?;
            let words = split_command_line(&line).with_context(|| {
                format!(
                    "{location} ({}): invalid quoting or escape",
                    line.split_whitespace().next().unwrap_or("<empty>")
                )
            })?;
            let Some(name) = words.first() else {
                continue;
            };
            let context = format!("{location} ({name})");
            let matches = parser
                .try_get_matches_from_mut(std::iter::once("batch").chain(words.iter().map(String::as_str)))
                .with_context(|| context.clone())?;
            let command = BatchCommand::from_arg_matches(&matches)
                .with_context(|| context.clone())?
                .command;
            anyhow::ensure!(
                !matches!(command, Commands::Batch { .. } | Commands::ListUsb { .. }),
                "{context}: this command is not supported inside a batch"
            );
            self.args.command = command;
            let status = self.execute_command().with_context(|| context.clone())?;
            if !status.is_success() {
                return Err(anyhow::Error::new(CommunicationError::from(status)).context(context));
            }
        }
        Ok(())
    }
}
