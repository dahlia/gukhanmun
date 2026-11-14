// Gukhanmun: build the FOSS for All Conference 2026 A0 poster PDF.
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

// Pipeline:
//
//  1. Compile dictionary.tsv, the poster's own entries for words the bundled
//     dictionaries split, with gukhanmun-mkdict.  Pipe body.html (mixed-script
//     Korean) through the workspace's own gukhanmun CLI with that dictionary,
//     which adds hangul ruby to every hanja word.  Elements with class="keep",
//     lang="en" scopes, and <pre>/<code> pass untouched.
//  2. Generate SVG QR codes with segno (via uvx).
//  3. Inject the converted body into poster.html and print it to PDF with
//     headless Chrome; @page in poster.css fixes the A0 size.
//  4. Render PNG previews with pdftoppm and report the page size and fonts.
//
// Environment overrides: GUKHANMUN and GUKHANMUN_MKDICT (prebuilt binaries),
// CHROME (browser path).

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = process.env.MISE_MONOREPO_ROOT ?? resolve(here, "../../..");
const dist = join(here, "dist");
mkdirSync(dist, { recursive: true });

const qrCodes: Record<string, string> = {
  playground: "https://gukhanmun.org/playground",
  github: "https://github.com/dahlia/gukhanmun",
  sponsor: "https://gukhanmun.org/sponsorship",
};

function run(cmd: string, args: string[], input?: string, quiet = false): string {
  return execFileSync(cmd, args, {
    input,
    encoding: "utf8",
    stdio: [input == null ? "ignore" : "pipe", "pipe", quiet ? "ignore" : "inherit"],
    maxBuffer: 64 * 1024 * 1024,
  });
}

function cargoRun(pkg: string, args: string[], input?: string): string {
  return run("cargo", [
    "run",
    "--quiet",
    "--release",
    "--manifest-path",
    join(root, "Cargo.toml"),
    "-p",
    pkg,
    "--",
    ...args,
  ], input);
}

const dictionaryPath = join(dist, "poster.gukfst");

function buildDictionary(): void {
  const args = ["--validate", "--output", dictionaryPath, join(here, "dictionary.tsv")];
  if (process.env.GUKHANMUN_MKDICT) run(process.env.GUKHANMUN_MKDICT, args);
  else cargoRun("gukhanmun-mkdict", args);
}

function convert(html: string): string {
  const options = [
    "--dictionary",
    dictionaryPath,
    "--format",
    "text/html",
    "--rendering",
    "ruby-on-hanja",
    "--html-preserve-class",
    "keep",
  ];
  if (process.env.GUKHANMUN) return run(process.env.GUKHANMUN, options, html);
  return cargoRun("gukhanmun-cli", options, html);
}

function findChrome(): string {
  if (process.env.CHROME) return process.env.CHROME;
  for (const name of ["google-chrome-stable", "google-chrome", "chromium-browser", "chromium"]) {
    try {
      return run("which", [name]).trim();
    } catch {
      // Try the next candidate.
    }
  }
  throw new Error("No Chrome or Chromium found; set CHROME to its path.");
}

// 1. Ruby conversion.
buildDictionary();
const body = convert(readFileSync(join(here, "body.html"), "utf8"));

// 2. QR codes.
for (const [name, url] of Object.entries(qrCodes)) {
  const out = join(dist, `qr-${name}.svg`);
  if (existsSync(out)) continue;
  run("uvx", [
    "--from",
    "segno==1.6.6",
    "segno",
    "--error=m",
    "--border=0",
    `--output=${out}`,
    url,
  ]);
}

// 3. Assemble and print.
for (
  const [from, to] of [
    [join(here, "poster.css"), "poster.css"],
    [join(here, "assets", "kyunghyang-1981-05-07.png"), "kyunghyang-1981-05-07.png"],
    [join(root, "logo-square.svg"), "logo-square.svg"],
  ]
) {
  copyFileSync(from, join(dist, to));
}
const template = readFileSync(join(here, "poster.html"), "utf8");
const htmlPath = join(dist, "poster.html");
writeFileSync(htmlPath, template.replace("<!-- @body -->", body));

// Layout report: the height of each top-level block in millimetres, read back
// from a copy of the page through --dump-dom.
const measureScript = `<script>
addEventListener("load", () => {
  const mm = (px) => Math.round(px / (96 / 25.4));
  const poster = document.querySelector(".poster");
  const rows = [...poster.children].map((el) =>
    [el.className || el.tagName, mm(el.getBoundingClientRect().height)]);
  rows.push(["total (A0 is 1189)", mm(poster.scrollHeight)]);
  const out = document.createElement("pre");
  out.id = "measure";
  out.textContent = JSON.stringify(rows);
  document.body.append(out);
});
</script>`;
const measurePath = join(dist, "measure.html");
writeFileSync(measurePath, template.replace("<!-- @body -->", body + measureScript));
const dom = run(
  findChrome(),
  [
    "--headless",
    "--disable-gpu",
    "--window-size=3179,4494",
    "--virtual-time-budget=15000",
    "--dump-dom",
    pathToFileURL(measurePath).href,
  ],
  undefined,
  true,
);
const measured = /<pre id="measure">(.*?)<\/pre>/s.exec(dom)?.[1];
if (measured) {
  console.log("Layout (mm):");
  for (const [name, height] of JSON.parse(measured.replaceAll("&quot;", '"'))) {
    console.log(`  ${String(height).padStart(5)}  ${name}`);
  }
}

const pdfPath = join(dist, "poster.pdf");
run(
  findChrome(),
  [
    "--headless",
    "--disable-gpu",
    "--no-pdf-header-footer",
    "--run-all-compositor-stages-before-draw",
    "--virtual-time-budget=15000",
    `--print-to-pdf=${pdfPath}`,
    pathToFileURL(htmlPath).href,
  ],
  undefined,
  true,
);

// 4. Previews and checks.
run("pdftoppm", ["-r", "24", "-png", "-singlefile", pdfPath, join(dist, "preview")]);
run("pdftoppm", ["-r", "72", "-png", "-singlefile", pdfPath, join(dist, "preview-72dpi")]);
const info = run("pdfinfo", [pdfPath]);
const pages = /^Pages:\s+(\d+)/m.exec(info)?.[1];
const size = /^Page size:\s+(.+)$/m.exec(info)?.[1];
console.log(`PDF: ${pdfPath}`);
console.log(`Pages: ${pages}; page size: ${size} (A0 is 2383.94 x 3370.39 pts)`);
const fonts = run("pdffonts", [pdfPath]).split("\n").slice(2).filter(Boolean);
const unembedded = fonts.filter((line) => line.split(/\s+/).at(-5) !== "yes");
console.log(`Fonts: ${fonts.length}, not embedded: ${unembedded.length}`);
for (const line of unembedded) console.log(`  ${line}`);
if (pages !== "1") {
  console.error("The poster must be a single page; content overflows.");
  process.exitCode = 1;
}
