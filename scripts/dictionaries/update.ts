// Gukhanmun: Regenerates validated dictionary sources as a reversible offline transaction.
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
import {
  data,
  fragments,
  hash,
  json,
  kinds,
  notices,
  outputs,
  read,
  safePath,
  updatePaths,
} from "./model.ts";
import type { Kind } from "./model.ts";
const project = fileURLToPath(new URL("../../", import.meta.url));
const decoder = new TextDecoder();
const names: Record<Kind, string> = {
  stdict: "*Standard Korean Dictionary* (標準國語大辭典)",
  opendict: "*Open Korean Dictionary* (우리말샘)",
};
/** Injectable process boundary; extraction outputs are confined to its staging env. */
export type Run = (
  cmd: string,
  args: string[],
  cwd: string,
  env?: Record<string, string>,
) => Promise<string>;
/** Runs offline tools with account and publication credentials removed. */
export const run: Run = async (cmd, args, cwd, env = {}) => {
  const r = await new Deno.Command(cmd, {
    args,
    cwd,
    env: { NIKL_USERNAME: "", NIKL_PASSWORD: "", GH_TOKEN: "", GITHUB_TOKEN: "", ...env },
    stdout: "piped",
    stderr: "piped",
  }).output();
  if (!r.success) {
    let detail = decoder.decode(r.stderr);
    for (const key of ["NIKL_USERNAME", "NIKL_PASSWORD", "GH_TOKEN", "GITHUB_TOKEN"]) {
      const value = Deno.env.get(key);
      if (value) {
        for (const secret of [value, encodeURIComponent(value)]) {
          detail = detail.replaceAll(secret, "[redacted]");
        }
      }
    }
    throw new Error(`${cmd} failed: ${detail.slice(-8192)}`);
  }
  return decoder.decode(r.stdout).trim();
};
/** Snapshot provenance after collector validation; output details belong to preparation. */
export interface Source {
  version: number;
  dictionary: Kind;
  link_key: string;
  source_url: string;
  archive_date: string;
  archive_filename: string;
  archive_member_key: string;
  archive_sha256: string;
  archive_bytes: number;
  record_count: number;
  unique_record_count: number;
  duplicate_record_count: number;
  shard_count: number;
  terminal_endpoint: number;
  inputs_sha256?: string;
  outputs?: Record<string, MappingReport>;
}
/** Changes compare complete row groups, preserving candidate and source-ID differences. */
export interface MappingReport {
  sha256: string;
  rows: number;
  keys: number;
  added: number;
  removed: number;
  changed: number;
}
/** Outcome is deterministic and transferable without account or source archive material. */
export interface Result {
  status: "updated" | "unchanged";
  summary: string;
  sources: Partial<Record<Kind, Source>>;
}
function groups(bytes: Uint8Array): Map<string, string[]> {
  const rows = decoder.decode(bytes).trimEnd().split("\n");
  const result = new Map<string, string[]>();
  for (const line of rows.slice(1)) {
    if (!line) continue;
    const key = line.split("\t")[0];
    const values = result.get(key) ?? [];
    values.push(line);
    result.set(key, values);
  }
  return result;
}
/** Counts key-level changes including alternate readings and positional forms. */
export async function mappingReport(before: Uint8Array, after: Uint8Array): Promise<MappingReport> {
  const old = groups(before), current = groups(after);
  return {
    sha256: await hash(after),
    rows: [...current.values()].reduce((n, rows) => n + rows.length, 0),
    keys: current.size,
    added: [...current.keys()].filter((k) => !old.has(k)).length,
    removed: [...old.keys()].filter((k) => !current.has(k)).length,
    changed:
      [...current.keys()].filter((k) =>
        old.has(k) && JSON.stringify(old.get(k)) !== JSON.stringify(current.get(k))
      ).length,
  };
}
function fragment(previous: Uint8Array | null, kind: Kind, date: string): Uint8Array {
  const prefix = kind === "stdict" ? "Standard Korean Dictionary" : "Open Korean Dictionary";
  const lines = (previous ? decoder.decode(previous) : "").split("\n"), kept = [];
  for (let i = 0; i < lines.length; i++) {
    if (new RegExp(`^\\s*-\\s+Updated the bundled \\*${prefix}\\*`).test(lines[i])) {
      while (i + 1 < lines.length && /^ {4}/.test(lines[i + 1])) i++;
    } else kept.push(lines[i]);
  }
  return new TextEncoder().encode(
    `${kept.join("\n").trimEnd()}\n\n - Updated the bundled ${
      names[kind]
    } snapshot to ${date}. [[#29]]\n`,
  );
}
function updateNotice(text: string, kind: Kind, source: Source): string {
  const label = kind === "stdict" ? "표준국어대사전" : "우리말샘";
  const pattern = new RegExp(`(Snapshot: \\*전체 내려받기[^\\n]*${label}[^\\n]*)(?:\\n|$)`);
  if (!pattern.test(text)) {
    if (kind === "opendict" || text.includes(label)) {
      throw new Error("snapshot notice marker missing");
    }
    return text;
  }
  return text.replace(pattern, `Snapshot: *${source.archive_filename.replaceAll("_", "\\_")}*\n`);
}
/** Validates sources, stages all extraction, then applies data and Sacho together. */
export async function update(root: string, directory: string, execute: Run = run): Promise<Result> {
  const temp = await Deno.makeTempDir({ prefix: "gukhanmun-dictionaries-" });
  const changes = new Map<string, Uint8Array>();
  const sources: Partial<Record<Kind, Source>> = {};
  try {
    await execute("mise", ["exec", "--", "sacho", "check"], root);
    const collection = JSON.parse(
      await execute("python", [
        "-B",
        `${project}/scripts/dictionaries/collect.py`,
        "validate",
        directory,
      ], root),
    );
    const statePath = `${temp}/state.json`;
    await execute("python", [
      "-B",
      `${project}/scripts/dictionaries/collect.py`,
      "state",
      root,
      statePath,
    ], root);
    const current = JSON.parse(await Deno.readTextFile(statePath));
    const affected: Kind[] = [];
    for (const kind of kinds) {
      const item = collection.dictionaries[kind];
      if (item.status === "unchanged") {
        const old = current.dictionaries[kind];
        if (!old || JSON.stringify(old) !== JSON.stringify(item.source)) {
          throw new Error(`${kind}: skipped source does not match current target inputs/outputs`);
        }
        continue;
      }
      const source: Source = item.source;
      const staged = `${temp}/${kind}`;
      await Deno.mkdir(staged);
      await execute("mise", ["run", `${kind}-extract`, "--", `${directory}/${kind}.zip`], root, {
        GUKHANMUN_DICTS_OUTPUT_DIR: staged,
      });
      const reports: Record<string, MappingReport> = {};
      let behaviorChanged = false;
      for (const name of outputs[kind]) {
        const bytes = await Deno.readFile(`${staged}/${name}`);
        const old = await read(root, `${data(kind)}/${name}`);
        if (!old) throw new Error("target dictionary file missing");
        reports[name] = await mappingReport(old, bytes);
        if (await hash(old) !== await hash(bytes)) {
          behaviorChanged = true;
          changes.set(`${data(kind)}/${name}`, bytes);
        }
      }
      const oldBytes = await read(root, `${data(kind)}/source.json`);
      const old = oldBytes ? JSON.parse(decoder.decode(oldBytes)) : null;
      const sameSource = old && old.link_key === source.link_key &&
        old.archive_sha256 === source.archive_sha256 && old.archive_date === source.archive_date;
      // A recipe-only change with identical outputs should not create provenance PRs.
      if (sameSource && !behaviorChanged) continue;
      source.inputs_sha256 = current.inputs_sha256;
      source.outputs = reports;
      sources[kind] = source;
      changes.set(`${data(kind)}/source.json`, json(source));
      const metadataBytes = await read(root, `${data(kind)}/metadata.json`);
      if (!metadataBytes) throw new Error("target metadata integration missing");
      const metadata = JSON.parse(decoder.decode(metadataBytes));
      metadata.source = `${kind}-json-${source.archive_date.replaceAll("-", "")}`;
      metadata.build_date = `${source.archive_date}T00:00:00Z`;
      changes.set(`${data(kind)}/metadata.json`, json(metadata));
      if (behaviorChanged) {
        affected.push(kind);
        for (const path of fragments(kind)) {
          changes.set(path, fragment(await read(root, path), kind, source.archive_date));
        }
      }
      for (const path of notices) {
        if (kind === "stdict" && path !== "NOTICE.md") continue;
        const bytes = changes.get(path) ?? await read(root, path);
        if (!bytes) throw new Error("shipped notice missing");
        changes.set(
          path,
          new TextEncoder().encode(updateNotice(decoder.decode(bytes), kind, source)),
        );
      }
    }
    // Remove byte-identical metadata/notices rather than manufacturing a transaction.
    for (const [path, bytes] of changes) {
      const old = await read(root, path);
      if (old && await hash(old) === await hash(bytes)) changes.delete(path);
    }
    if (!changes.size) {
      return { status: "unchanged", sources: {}, summary: "No dictionary data changes." };
    }
    const touched = [...changes.keys(), ...(affected.length ? ["CHANGES.md"] : [])];
    const backups = new Map<string, { bytes: Uint8Array | null; mode: number | null }>();
    for (const path of touched) {
      const bytes = await read(root, path);
      backups.set(path, { bytes, mode: bytes ? (await Deno.stat(`${root}/${path}`)).mode : null });
    }
    try {
      for (const [path, bytes] of changes) {
        if (!updatePaths.includes(path)) throw new Error("update path outside allowlist");
        await safePath(root, path);
        await Deno.mkdir(`${root}/${path.slice(0, path.lastIndexOf("/"))}`, { recursive: true });
        await Deno.writeFile(`${root}/${path}`, bytes);
      }
      const changedNotices = notices.filter((path) => changes.has(path));
      if (changedNotices.length) {
        await execute("mise", ["exec", "--", "hongdown", "-w", ...changedNotices], root);
      }
      if (affected.length) {
        await execute(
          "mise",
          ["exec", "--", "hongdown", "-w", ...affected.flatMap(fragments)],
          root,
        );
        for (const args of [["sync", "--force"], ["fmt"], ["check"]]) {
          await execute("mise", ["exec", "--", "sacho", ...args], root);
        }
      }
    } catch (error) {
      for (const [path, backup] of backups) {
        if (backup.bytes === null) {
          try {
            await Deno.remove(`${root}/${path}`);
          } catch (e) {
            if (!(e instanceof Deno.errors.NotFound)) throw e;
          }
        } else {
          await Deno.writeFile(`${root}/${path}`, backup.bytes);
          if (backup.mode !== null && Deno.build.os !== "windows") {
            await Deno.chmod(`${root}/${path}`, backup.mode);
          }
        }
      }
      throw error;
    }
    const summary = kinds.filter((kind) => sources[kind]).map((kind) => {
      const s = sources[kind]!;
      return `### ${
        names[kind]
      }\n\nSource: ${s.archive_date}, download ${s.link_key}, SHA-256 \`${s.archive_sha256}\`.\n\n` +
        Object.entries(s.outputs!).map(([name, r]) =>
          `- ${name}: ${r.keys} keys, ${r.added} added, ${r.removed} removed, ${r.changed} changed.`
        ).join("\n");
    }).join("\n\n");
    return { status: "updated", sources, summary };
  } finally {
    await Deno.remove(temp, { recursive: true });
  }
}
if (import.meta.main) {
  try {
    const directory = Deno.args[0];
    if (!directory) throw new Error("validated collection directory required");
    const result = await update(Deno.env.get("GUKHANMUN_DICTS_ROOT") ?? project, directory);
    if (Deno.env.get("GUKHANMUN_DICTS_RESULT")) {
      await Deno.writeFile(Deno.env.get("GUKHANMUN_DICTS_RESULT")!, json(result));
    }
    if (Deno.env.get("GITHUB_OUTPUT")) {
      await Deno.writeTextFile(Deno.env.get("GITHUB_OUTPUT")!, `status=${result.status}\n`, {
        append: true,
      });
    }
    if (Deno.env.get("GITHUB_STEP_SUMMARY")) {
      await Deno.writeTextFile(Deno.env.get("GITHUB_STEP_SUMMARY")!, `${result.summary}\n`, {
        append: true,
      });
    }
    console.log(result.summary);
  } catch (e) {
    const message = e instanceof Error ? e.message : "dictionary update failed";
    console.error(message);
    if (Deno.env.get("GITHUB_STEP_SUMMARY")) {
      await Deno.writeTextFile(
        Deno.env.get("GITHUB_STEP_SUMMARY")!,
        `Dictionary update failed:\n\n\`\`\`text\n${message}\n\`\`\`\n`,
        { append: true },
      );
    }
    Deno.exit(1);
  }
}
