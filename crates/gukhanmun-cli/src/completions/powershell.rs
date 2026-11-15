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

use clap::{Command, ValueHint};
use clap_complete::Generator;
use serde_json::json;

/// A static PowerShell generator with parser-derived option value candidates.
/// The upstream generator currently covers option names only.
pub(super) struct PowerShell;

impl Generator for PowerShell {
    fn file_name(&self, name: &str) -> String {
        format!("_{name}.ps1")
    }

    fn generate(&self, command: &Command, writer: &mut dyn Write) {
        self.try_generate(command, writer)
            .expect("failed to write completion file");
    }

    fn try_generate(&self, command: &Command, writer: &mut dyn Write) -> io::Result<()> {
        let options: Vec<_> = command
            .get_arguments()
            .filter(|arg| !arg.is_positional() && !arg.is_hide_set())
            .map(|arg| {
                let mut names = Vec::new();
                if let Some(shorts) = arg.get_short_and_visible_aliases() {
                    names.extend(shorts.into_iter().map(|name| format!("-{name}")));
                }
                if let Some(longs) = arg.get_long_and_visible_aliases() {
                    names.extend(longs.into_iter().map(|name| format!("--{name}")));
                }
                let values: Vec<_> = arg
                    .get_value_parser()
                    .possible_values()
                    .into_iter()
                    .flatten()
                    .filter(|value| !value.is_hide_set())
                    .map(|value| value.get_name().to_owned())
                    .collect();
                json!({
                    "names": names,
                    "takes_value": arg.get_num_args().is_some_and(|arity| arity.takes_values()),
                    "values": values,
                    "path": matches!(arg.get_value_hint(), ValueHint::FilePath | ValueHint::AnyPath | ValueHint::DirPath),
                    "help": arg.get_help().map(ToString::to_string).unwrap_or_default(),
                })
            })
            .collect();
        let schema = serde_json::to_string(&options).map_err(io::Error::other)?;
        let script = include_str!("powershell.ps1").replace("__GUKHANMUN_SCHEMA__", &schema);
        writer.write_all(script.as_bytes())
    }
}
