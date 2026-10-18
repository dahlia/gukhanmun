// Gukhanmun: Loads local account variables only for explicit dictionary collection.
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

import { fileURLToPath } from "node:url";

const result = await new Deno.Command("python", {
  args: ["-B", fileURLToPath(new URL("./collect.py", import.meta.url)), "collect", ...Deno.args],
  stdin: "null",
  stdout: "inherit",
  stderr: "inherit",
}).output();
Deno.exit(result.code);
