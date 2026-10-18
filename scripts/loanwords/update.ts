// Gukhanmun: Regenerates pinned NIKL loanword data transactionally.
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

/** Offline regeneration and transactional NIKL source updates. */
import { fileURLToPath } from "node:url";
const project = fileURLToPath(new URL("../../", import.meta.url));
const data = "crates/gukhanmun-loanwords/data";
const sections = [
  "crates/gukhanmun-loanwords",
  "crates/gukhanmun",
  "crates/gukhanmun-cli",
  "packages/loanwords-fst",
  "packages/loanwords-cdb",
];
const fragments = sections.map((section) => `changes.d/${section}/loanword-data-update.md`);
/** Exact paths that the scheduled updater may change or transfer. */
export const updatePaths = [
  `${data}/snapshot.json`,
  `${data}/loanwords.tsv`,
  `${data}/report.json`,
  `${data}/metadata.json`,
  `${data}/pending-update.json`,
  ...fragments,
  "CHANGES.md",
];
const encoder = new TextEncoder();
const decoder = new TextDecoder();
/** Computes an artifact integrity checksum. */
export async function hash(bytes: Uint8Array): Promise<string> {
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as BufferSource)))
    .map((n) => n.toString(16).padStart(2, "0")).join("");
}
// The pinned Deno runtime supplies source-aware JSON beyond TypeScript's declarations.
interface RawNumber {
  readonly rawJSON: string;
}
const sourceJSON = JSON as typeof JSON & {
  rawJSON(source: string): RawNumber;
  isRawJSON(value: unknown): value is RawNumber;
};
/** Stable JSON used for generated metadata and semantic fingerprints. */
export function canonical(value: unknown): string {
  if (sourceJSON.isRawJSON(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    return `{${
      Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonical(record[key])}`)
        .join(",")
    }}`;
  }
  return JSON.stringify(value);
}
async function optional(path: string): Promise<Uint8Array | null> {
  try {
    if ((await Deno.lstat(path)).isSymlink) throw new Error("symlink update path refused");
    return await Deno.readFile(path);
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return null;
    throw error;
  }
}
/** Redacts configured service keys and their query encodings from tool diagnostics. */
export function redactDiagnostics(text: string): string {
  const secrets = new Set<string>();
  for (const name of ["NIKL_LOANWORD_API_KEY", "KOREAN_GO_KR_SERVICE_KEY"]) {
    const value = Deno.env.get(name);
    if (!value) continue;
    secrets.add(value);
    let decoded = value;
    try {
      decoded = decodeURIComponent(value);
    } catch {
      // An invalid configured escape must not prevent redaction of the raw value.
    }
    secrets.add(decoded);
    for (const key of [value, decoded]) {
      secrets.add(encodeURIComponent(key));
      // ureq keeps URI-component punctuation except apostrophes, and uses %20 for spaces.
      secrets.add(encodeURIComponent(key).replaceAll("'", "%27"));
      secrets.add(new URLSearchParams({ serviceKey: key }).toString().slice("serviceKey=".length));
    }
  }
  for (const secret of [...secrets].sort((a, b) => b.length - a.length)) {
    const escaped = secret.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const pattern = escaped.replace(
      /%[\da-f]{2}/gi,
      (escape) =>
        escape.replace(/[a-f]/gi, (char) => `[${char.toLowerCase()}${char.toUpperCase()}]`),
    );
    text = text.replaceAll(new RegExp(pattern, "g"), "[redacted]");
  }
  return text;
}
async function run(command: string, args: string[], cwd = project): Promise<boolean> {
  const child = new Deno.Command(command, { args, cwd, stdout: "piped", stderr: "piped" });
  const result = await child.output();
  if (!result.success) {
    console.error(redactDiagnostics(`${command}: operation failed`));
    const detail = redactDiagnostics(decoder.decode(result.stderr)).trim();
    // Redact before truncating so a boundary cannot expose a partial key.
    if (detail) console.error(detail.slice(-16_384));
  }
  return result.success;
}
async function cli(args: string[]): Promise<boolean> {
  const binary = Deno.env.get("GUKHANMUN_LOANWORDS_BIN");
  return binary ? await run(binary, args) : await run("cargo", [
    "run",
    "--locked",
    "--target-dir",
    "target/loanwords-tools",
    "-p",
    "gukhanmun-dict-extract",
    "--features",
    "loanwords-collect",
    "--bin",
    "gukhanmun-loanwords",
    "--",
    ...args,
  ]);
}
function metadata(snapshot: Record<string, unknown>): Record<string, string> {
  const collected = String(snapshot.collected_at);
  const instant = collected.startsWith("unix:")
    ? Number(collected.slice(5)) * 1000
    : Date.parse(collected);
  const date = Number.isFinite(instant) && Math.abs(instant) <= 8640000000000000
    ? new Date(instant).toISOString()
    : "1970-01-01T00:00:00.000Z";
  return {
    build_date: date,
    selector: "0003",
    source: "National Institute of Korean Language loanword spelling examples",
    source_url: "https://korean.go.kr/kornorms/exampleReqList.do",
    records_sha256: String(snapshot.records_sha256),
    record_count: String(snapshot.total_count),
    license: "NIKL free use; see ATTRIBUTION.md",
  };
}
type SourceSnapshot = Record<string, unknown> & { records: { id: string }[] };
function parseSource(text: string): SourceSnapshot {
  return JSON.parse(
    text,
    (_key: string, value: unknown, context?: { source: string }) => {
      if (typeof value !== "number") return value;
      if (!context) throw new Error("source-aware JSON runtime required");
      return context.source !== JSON.stringify(value) ? sourceJSON.rawJSON(context.source) : value;
    },
  );
}
function snapshotBytes(snapshot: Record<string, unknown>): Uint8Array {
  const { records, ...header } = snapshot;
  return encoder.encode(
    `${canonical(header).slice(0, -1)},\n"records":[\n${
      (records as unknown[]).map(canonical).join(",\n")
    }\n]}\n`,
  );
}
async function transaction(
  root: string,
  changes: Map<string, Uint8Array | null>,
  notes: string | null,
): Promise<void> {
  const paths = [...new Set([...changes.keys(), ...(notes ? [...fragments, "CHANGES.md"] : [])])];
  const backups = new Map<string, Uint8Array | null>();
  for (const path of paths) backups.set(path, await optional(`${root}/${path}`));
  try {
    for (const [path, bytes] of changes) {
      if (bytes === null) {
        if (backups.get(path) !== null) await Deno.remove(`${root}/${path}`);
      } else {
        await Deno.mkdir(`${root}/${path.slice(0, path.lastIndexOf("/"))}`, { recursive: true });
        await Deno.writeFile(`${root}/${path}`, bytes);
      }
    }
    if (notes) {
      for (const fragment of fragments) {
        await Deno.mkdir(`${root}/${fragment.slice(0, fragment.lastIndexOf("/"))}`, {
          recursive: true,
        });
        const previous = backups.get(fragment);
        const content = previous
          ? decoder.decode(previous)
          : "---\nlinks:\n  '#18': https://github.com/dahlia/gukhanmun/issues/18\n---\n";
        const lines = content.split("\n");
        const kept: string[] = [];
        for (let index = 0; index < lines.length; index++) {
          if (/^\s*-\s+Updated the NIKL loanword examples snapshot\b/.test(lines[index])) {
            while (index + 1 < lines.length && /^ {4}/.test(lines[index + 1])) index++;
          } else kept.push(lines[index]);
        }
        await Deno.writeTextFile(
          `${root}/${fragment}`,
          `${kept.join("\n").trimEnd()}\n\n - ${notes}\n`,
        );
      }
      if (
        !await run("mise", ["exec", "--", "hongdown", "-w", ...fragments], root) ||
        !await run("mise", ["exec", "--", "sacho", "sync", "--force"], root) ||
        !await run("mise", ["exec", "--", "sacho", "fmt"], root) ||
        !await run("mise", ["exec", "--", "sacho", "check"], root)
      ) {
        throw new Error("changelog regeneration failed");
      }
    }
  } catch (error) {
    for (const [path, bytes] of backups) {
      if (bytes === null) {
        try {
          await Deno.remove(`${root}/${path}`);
        } catch (e) {
          if (!(e instanceof Deno.errors.NotFound)) throw e;
        }
      } else await Deno.writeFile(`${root}/${path}`, bytes);
    }
    throw error;
  }
}
/** Fingerprints source, editorial choices and generated behavior, excluding collection time. */
export async function generationFingerprint(
  snapshot: Record<string, unknown>,
  report: Record<string, unknown>,
): Promise<string> {
  return await hash(
    encoder.encode(
      canonical({
        records: snapshot.records_sha256,
        decisions: report.decisions_sha256,
        tsv: report.tsv_sha256,
        audit: report.audit,
      }),
    ),
  );
}
/** Outcome includes an upstream fingerprint for idempotent bot-branch refreshes. */
export interface UpdateResult {
  /** unchanged, updated, or pending editorial review. */
  status: "unchanged" | "updated" | "pending";
  /** Hash of the exact proposed tracked state, ignoring collection timestamps. */
  fingerprint: string;
}
/** Regenerates/checks a pinned corpus or validates a complete live/offline update. */
export async function update(
  root: string,
  mode: string,
  source?: string,
  priorCandidate?: string,
): Promise<UpdateResult> {
  const temp = await Deno.makeTempDir({ prefix: "gukhanmun-loanwords-" });
  try {
    const oldBytes = await optional(`${root}/${data}/snapshot.json`);
    const incoming = `${temp}/snapshot.json`;
    if (mode === "update") {
      if (source) await Deno.copyFile(source, incoming);
      else if (!await cli(["collect", "--rows", "1000", "--output", incoming])) {
        throw new Error("collection failed; pinned data preserved");
      }
    } else {
      if (!oldBytes) throw new Error("pinned snapshot is missing");
      await Deno.writeFile(incoming, oldBytes);
    }
    // Validate the original JSON before normalization can hide duplicate keys.
    const current = parseSource(await Deno.readTextFile(incoming));
    const tsv = `${temp}/loanwords.tsv`,
      report = `${temp}/report.json`,
      diagnostic = `${temp}/audit.json`;
    const args = [
      "--snapshot",
      incoming,
      "--decisions",
      `${root}/${data}/decisions.json`,
      "--output",
      tsv,
      "--report",
      report,
    ];
    if (!await cli(["extract", ...args, "--diagnostic", diagnostic])) {
      if (mode !== "update") throw new Error("pinned editorial decisions require review");
      const auditDocument = JSON.parse(await Deno.readTextFile(diagnostic));
      const audit = auditDocument.audit;
      const keys = [...new Set<string>([...audit.unresolved, ...audit.stale_decisions])].sort();
      if (!keys.length) throw new Error("extraction failed without editorial conflicts");
      const sourceIds = new Set<string>();
      for (const key of keys.slice(0, 100)) {
        for (const candidate of audit.candidates[key] ?? []) {
          for (const source of candidate.sources) sourceIds.add(source.id);
        }
      }
      const evidence = current.records.filter((record: { id: string }) => sourceIds.has(record.id))
        .slice(0, 200);
      const pending = encoder.encode(
        JSON.stringify(
          {
            format_version: 1,
            status: "pending-editorial-review",
            records_sha256: current.records_sha256,
            total_count: current.total_count,
            affected_count: keys.length,
            truncated: keys.length > 100,
            source_records_truncated: sourceIds.size > evidence.length,
            source_records: evidence,
            candidates: Object.fromEntries(
              keys.slice(0, 100).map((key) => [key, audit.candidates[key] ?? []]),
            ),
          },
          null,
          2,
        ) + "\n",
      );
      if (pending.length > 2 * 1024 * 1024) throw new Error("pending audit exceeds report limit");
      await transaction(root, new Map([[`${data}/pending-update.json`, pending]]), null);
      return { status: "pending", fingerprint: await hash(pending) };
    }
    let previous = oldBytes ? parseSource(decoder.decode(oldBytes)) : null;
    if (
      mode === "update" && previous?.records_sha256 !== current.records_sha256 && priorCandidate
    ) {
      const bytes = await optional(priorCandidate);
      let candidate: SourceSnapshot | null = null;
      if (bytes) {
        try {
          candidate = parseSource(decoder.decode(bytes));
        } catch {
          // Corrupt unmerged proposals are rebuilt from the newly collected source.
        }
      }
      if (
        candidate && candidate.records_sha256 === current.records_sha256 && await cli([
          "audit",
          "--snapshot",
          priorCandidate,
          "--output",
          `${temp}/candidate.tsv`,
          "--report",
          `${temp}/candidate-audit.json`,
        ])
      ) previous = candidate;
    }
    if (previous && previous.records_sha256 === current.records_sha256) {
      if (
        typeof previous.collected_at !== "string" || typeof previous.collector_version !== "string"
      ) {
        throw new Error("invalid prior collection metadata");
      }
      current.collected_at = previous.collected_at;
      current.collector_version = previous.collector_version;
    }
    await Deno.writeFile(incoming, snapshotBytes(current));
    // Refresh the report's snapshot hash after retaining prior collection metadata.
    if (!await cli(["extract", ...args])) {
      throw new Error("normalized snapshot failed validation");
    }
    const generated = new Map<string, Uint8Array | null>([
      [`${data}/snapshot.json`, await Deno.readFile(incoming)],
      [`${data}/loanwords.tsv`, await Deno.readFile(tsv)],
      [`${data}/report.json`, await Deno.readFile(report)],
      [`${data}/metadata.json`, encoder.encode(JSON.stringify(metadata(current), null, 2) + "\n")],
      [`${data}/pending-update.json`, null],
    ]);
    const reportValue = JSON.parse(decoder.decode(generated.get(`${data}/report.json`)!));
    const fingerprint = await generationFingerprint(current, reportValue);
    let changed = false;
    for (const [path, bytes] of generated) {
      const previous = await optional(`${root}/${path}`);
      if (
        bytes === null
          ? previous !== null
          : previous === null || await hash(previous) !== await hash(bytes)
      ) changed = true;
    }
    if (mode === "check") {
      if (changed) throw new Error("pinned loanword artifacts are stale");
    } else if (changed) {
      await transaction(
        root,
        generated,
        mode === "update"
          ? `Updated the NIKL loanword examples snapshot (\`${
            String(current.records_sha256).slice(0, 12)
          }\`; ${current.total_count} source records) and regenerated its dictionary data. [[#18]]`
          : null,
      );
    }
    return { status: changed ? "updated" : "unchanged", fingerprint };
  } finally {
    await Deno.remove(temp, { recursive: true });
  }
}
async function main(): Promise<void> {
  const [mode = "check", source] = Deno.args;
  if (mode === "collect") {
    if (!source) throw new Error("external source destination required");
    if (!await cli(["collect", "--rows", "1000", "--output", source])) {
      throw new Error("source collection failed");
    }
    const text = await Deno.readTextFile(source);
    if (redactDiagnostics(text) !== text) {
      await Deno.remove(source);
      throw new Error("credential-bearing source refused");
    }
    return;
  }
  if (!["check", "regenerate", "update"].includes(mode)) {
    throw new Error("unknown loanword operation");
  }
  const root = Deno.env.get("GUKHANMUN_LOANWORDS_ROOT") ?? project;
  if (root !== project) {
    for (
      const path of [
        "mise.toml",
        "sacho.toml",
        `${data}/decisions.json`,
        "crates/gukhanmun-dict-extract/Cargo.toml",
      ]
    ) {
      if (!(await Deno.stat(`${root}/${path}`)).isFile) {
        throw new Error("unsupported target layout");
      }
    }
  }
  const result = await update(
    root,
    mode,
    source,
    Deno.env.get("GUKHANMUN_LOANWORDS_PRIOR_CANDIDATE"),
  );
  console.log(JSON.stringify(result));
  const resultFile = Deno.env.get("GUKHANMUN_LOANWORDS_RESULT");
  // Offline verification must not overwrite the proposed update's publication outcome.
  if (resultFile && mode === "update") await Deno.writeTextFile(resultFile, JSON.stringify(result));
  const output = Deno.env.get("GITHUB_OUTPUT");
  if (output) {
    await Deno.writeTextFile(
      output,
      `status=${result.status}\nfingerprint=${result.fingerprint}\n`,
      { append: true },
    );
  }
}
if (import.meta.main) {
  try {
    await main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : "loanword operation failed");
    Deno.exit(1);
  }
}
