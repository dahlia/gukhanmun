// Gukhanmun: Generates static shell completions from the CLI schema.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::io::{self, Write};

use clap::{Command, ValueEnum};
use clap_complete::{Generator, shells};

mod powershell;

/// Shells with static completion scripts for the public command.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Nushell,
}

/// Generate from a built copy of the actual parser, without conversion state.
/// The public command name is stable even when a wrapper relocates the binary.
pub(super) fn write_completion(
    shell: Shell,
    mut command: Command,
    writer: &mut dyn Write,
) -> io::Result<()> {
    command.set_bin_name("gukhanmun");
    command.build();
    let generator: &dyn Generator = match shell {
        Shell::Bash => &shells::Bash,
        Shell::Zsh => &shells::Zsh,
        Shell::Fish => &shells::Fish,
        Shell::Powershell => &powershell::PowerShell,
        Shell::Nushell => &clap_complete_nushell::Nushell,
    };
    generator.try_generate(&command, writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, CommandFactory};

    #[test]
    fn schema_additions_reach_every_generator() {
        for shell in Shell::value_variants() {
            let command = crate::Cli::command().arg(
                Arg::new("future-option")
                    .long("future-option")
                    .visible_alias("future-alias")
                    .value_parser(["future-value-a", "future-value-b"]),
            );
            let mut output = Vec::new();
            write_completion(*shell, command, &mut output).unwrap();
            let output = String::from_utf8(output).unwrap();
            for expected in [
                "future-option",
                "future-alias",
                "future-value-a",
                "future-value-b",
            ] {
                assert!(output.contains(expected), "{shell:?} omitted {expected}");
            }
        }
    }

    #[test]
    fn write_errors_are_returned_by_every_generator() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("deliberate write failure"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        for shell in Shell::value_variants() {
            assert!(write_completion(*shell, crate::Cli::command(), &mut FailingWriter).is_err());
        }
    }
}
