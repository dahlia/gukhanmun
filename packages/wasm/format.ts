// Gukhanmun: Shared JavaScript format parsing and deprecation warnings.
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

import { getLogger } from "@logtape/logtape";
import type { MediaType } from "@gukhanmun/types";

const logger = getLogger(["gukhanmun", "format"]);

function warnLegacyFormat(format: string, replacement: MediaType): void {
  logger.warn(
    "The {format} format is deprecated and will be removed in Gukhanmun 1.0. Use {replacement} instead.",
    { kind: "deprecation", format, replacement },
  );
}

/** Reports the legacy Markdown object form, without logging the object itself. */
export function warnLegacyMarkdown(gfm: boolean | undefined): void {
  const old = gfm === undefined ? '{ format: "markdown" }' : `{ format: "markdown", gfm: ${gfm} }`;
  warnLegacyFormat(old, gfm ? "text/markdown; variant=GFM" : "text/markdown");
}

// Use Unicode White_Space to match Rust's str::trim() in the CLI parser.
function trim(s: string): string {
  return s.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");
}

function asciiLower(s: string): string {
  return s.replace(/[A-Z]/g, (c) => c.toLowerCase());
}

/**
 * Maps media types to the internal selectors understood by both bindings.
 * Unrecognized strings pass through for the binding to reject as usual.
 */
export function prepareStringFormat(format: string): string | { format: "markdown"; gfm: true } {
  switch (format) {
    case "text":
    case "html":
    case "markdown":
      warnLegacyFormat(JSON.stringify(format), `text/${format === "text" ? "plain" : format}`);
      return format;
  }

  // Keep these matching rules in sync with the CLI: case-sensitive base type,
  // case-insensitive variant key and value, and first variant wins.
  const [base, ...params] = format.split(";");
  switch (trim(base)) {
    case "text/plain":
      return "text";
    case "text/html":
      return "html";
    case "text/markdown":
      for (const param of params) {
        const eq = param.indexOf("=");
        if (eq < 0 || asciiLower(trim(param.slice(0, eq))) !== "variant") continue;
        const value = trim(param.slice(eq + 1));
        const unquoted = value.length >= 2 && value.startsWith('"') && value.endsWith('"')
          ? value.slice(1, -1)
          : value;
        switch (asciiLower(unquoted)) {
          case "gfm":
            return { format: "markdown", gfm: true };
          case "commonmark":
            return "markdown";
          default:
            return format;
        }
      }
      return "markdown";
    default:
      return format;
  }
}
