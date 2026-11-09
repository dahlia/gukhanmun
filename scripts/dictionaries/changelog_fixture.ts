// Gukhanmun: Provides literal old and new changelog configurations for updater tests.
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

import type { ChangelogLayout } from "./changelog.ts";
/** A literal target configuration, independent of the controller checkout's layout. */
export function configuration(layout: ChangelogLayout): string {
  return `[changelog]
path = "CHANGES.md"
title = "Gukhanmun changelog"
materialize = true
region-detection = "marker"
[fragments]
directory = "changes.d"
next-file = "next"
[links]
"#" = "https://github.com/dahlia/gukhanmun/issues/{n}"
[check]
paths = []
[[section-patterns]]
source = "crates/{name}"
id = "${layout === "legacy" ? "{name}" : "{name} (Rust)"}"
directory = "${layout === "legacy" ? "crates" : "rust"}/{name}"
[[section-patterns]]
source = "packages/{name}"
id = "@gukhanmun/{name}${layout === "legacy" ? "" : " (JavaScript/TypeScript)"}"
directory = "${layout === "legacy" ? "packages" : "javascript"}/{name}"
${
    layout === "legacy" ? "" : `[[section-patterns]]
source = "python/{name}"
id = "{name} (Python)"
directory = "python/{name}"
`
  }`;
}
