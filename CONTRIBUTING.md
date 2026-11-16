Contributing
============

Before diving in, read [*DESIGN.md*](./DESIGN.en.md) to understand the overall
design, architecture, and roadmap.

This repository uses [mise] as the single entry point for development tools and
commands.

[mise]: https://mise.jdx.dev/


Setup
-----

Install the configured tools:

~~~~ sh
mise install
~~~~

List available project commands:

~~~~ sh
mise tasks
~~~~

### Python development

Use mise to install and select interpreters and uv. The root development
environment and *uv.lock* are managed by uv. The root is a virtual workspace;
automatic dependency setup runs `uv sync --locked --no-install-workspace`
without compiling the native extension. Native build tasks initialize Rust
explicitly. No shell activation or additional interpreter manager is needed,
even with another venv active.

For the first lockfile, or after intentionally changing dependencies, bypass
automatic dependency setup while updating the lock:

~~~~ sh
mise exec --no-deps -- uv lock
mise run py-deps
~~~~

The default development interpreter is CPython 3.15.2. `mise run py-build`
builds and inspects its wheel; `mise run py-test` tests the installed wheel
outside the checkout. `mise run py-typecheck` checks the same installed package
with mypy, Pyright, basedpyright, ty and Pyrefly, including negative consumer
examples and native stub signatures. `mise run check` runs fast Python
tooling tests without building the extension; `mise run ci` includes the runtime
and all five type checkers.

The official dictionary data distributions under _python/gukhanmun-\*-\*/_ ship
no committed binaries. `mise run py-dicts-stage` copies the artifacts that the
`*-data` tasks generate under *packages/* and writes their license, attribution
and provenance files; `mise run py-dicts-build` builds and inspects the six
wheels and sdists; `mise run py-dicts-test` installs them through the core
extras from those local artifacts and tests the official data. `mise run ci`
includes both.

`mise run py-wheel-smoke` installs the host wheel and the data wheels the way
users would: each venv's own pip installs wheels only, from a local package
index of those artifacts, with no Rust tools on `PATH` and no inherited pip
settings. It then runs *tests/python-wheel/smoke.py* without the core
package's data, resolves every extra, and runs the official dictionary corpus
with all of them. *scripts/python/wheel\_smoke.py* needs only the standard
library, so CI runs the same checks on bare interpreters, and it writes an
evidence record under *target/python/smoke/*. `mise run ci` includes it.

`mise run py-sdist` builds the native sdist and checks that it stands alone:
the binding crate, the workspace crates it needs, the Cargo manifests and
lockfile, the Python sources, stubs and licenses, but none of the source data
of the optional dictionary crates, which the binding does not compile.
`mise run py-sdist-test` unpacks it outside the checkout and builds a wheel
from it with `uv build`, then smoke-tests that wheel together with the six data
wheels that `py-dicts-build` rebuilt from their own sdists.
*target/python/sdist-rebuilt/rebuilds.json* links each sdist to its rebuilt
wheel. The native rebuild runs without `--locked`, because maturin ships the
whole workspace lockfile but lists only the crates the binding needs; the
check accepts a lockfile that only drops unused entries. `mise run ci`
includes it.

The release consists of 31 files: a CPython wheel for eight targets, PyPy 3.11
and 3.12 wheels for five of them, the six data wheels and seven sdists.
*scripts/python/artifacts.py* defines the targets in one table, and CI
generates its matrix from it. `mise run py-dist -- <target>` builds, audits
and executes one target's wheels on a matching host:

~~~~ sh
mise run py-dist -- linux-x86_64
~~~~

Linux targets build inside digest-pinned PyPA manylinux and musllinux images
with their interpreters, so they need Docker or Podman. Set
`GUKHANMUN_CONTAINER_ENGINE=podman` to use Podman, and
`GUKHANMUN_CONTAINER_VOLUME_OPTIONS=z` on SELinux hosts. The audit checks the
wheel structure, the extension's architecture and libraries, and the platform
floor with the image's `auditwheel`, macOS's `lipo` and `otool`, or `pefile` on
Windows. The test runs the smoke checks with the oldest and newest CPython and
each PyPy; musl targets use the image interpreters, and glibc targets also run
once inside the manylinux image on its glibc. A target refuses to run on
another OS or architecture, because emulated or cross-compiled artifacts do
not count as executed. The outputs, with their provenance, audit and evidence
records, go to _target/python/release/<target>/_. `artifacts.py manifest`
inventories a directory of release files and records, and
`artifacts.py verify --complete` requires all 31 files with an audit, the
required executions and an sdist rebuild for each.

Install the complete interpreter set only when running the semantic matrix:

~~~~ sh
mise --env python install
mise run py-test-all
mise run py-test-all -- -e py311
mise run py-test-all -- -e pypy311,pypy312
~~~~

The optional *mise.python.toml* pins CPython 3.11–3.15 and PyPy 3.11/3.12.
The launcher runs environments serially and resolves each exact mise
installation. Missing interpreters fail with an installation instruction;
uv downloads and system Python substitutes are disabled. Use
`py-test-all` rather than calling tox directly. The tox runner uses the locked
test group, builds a noneditable wheel for each interpreter/ABI, and copies
runtime tests to temporary directories outside the checkout. Logs identify the
interpreter, base installation, installed distribution and native module hash.

These are development and semantic checks; the release artifacts are
validated separately, as described below. Free-threaded CPython is not
supported. Core tests use synthetic dictionaries and do not require the
optional official datasets.

GitHub Actions runs installed-wheel semantic tests on CPython 3.11–3.15 and
PyPy 3.11/3.12 on Linux. On macOS and Windows, the standard `ci` job tests
installed wheels with the current development CPython and runs all five type
checkers against them. The Python semantic jobs add CPython 3.11 and
PyPy 3.11/3.12. Each job installs its selected mise interpreter and builds a
native wheel for that environment.

GitHub Actions also produces the release artifact set on every pull request
and push, without any registry credentials. A `python-context` job generates
the target matrix from *scripts/python/artifacts.py* and the artifact
version: the workspace version on tags, `X.Y.Z.devN` on upstream pushes to
`main` and `X.Y-maintenance` (with the workflow run number as `N`), and
`X.Y.Z.dev0` on other non-tag events. The latter only exercises development
versioning and is never published. A workspace
version that is already a development release is used as it is. Every job
that produces or checks artifacts stamps that version first. `python-sdist`
builds the seven sdists and the data wheels and rebuilds every sdist outside
the checkout; `python-wheels` builds and audits each target on its native
runner; `python-wheel-test` installs the wheels on a fresh runner of the
same kind, without Rust, and executes them. `python-dists` writes the manifest,
verifies the complete set, and uploads it with the evidence records as the
`python-dists` artifact. Before uploading it, the job also validates all
packaged metadata and provenance. `python-publish-check` downloads that same
immutable artifact ID without registry credentials and writes a validation
receipt. The complete bundle is retained for 90 days.


Running checks
--------------

Run fast checks suitable for pre-commit hooks:

~~~~ sh
mise run check
~~~~

> [!TIP]
> To register that same task as this clone's Git pre-commit hook:
>
> ~~~~ sh
> mise generate git-pre-commit --task=check --write
> ~~~~

Run the full local verification gate:

~~~~ sh
mise run ci
~~~~

Useful individual commands:

~~~~ sh
mise run fmt
mise run fmt-check
mise run clippy
mise run ci
mise run doc
mise run test
mise run typecheck
~~~~


Conventions
-----------

Avoid relying on globally installed Rust tools or ad hoc command variants when
working on this repository. Add or update a mise task when the project needs a
new repeated development command.

Every new source file (Rust, TypeScript, JavaScript) must open with the GPLv3
notice block:

~~~~ rust
// Gukhanmun: <one-line description of what this file does>.
// Copyright (C) <year>  Hong Minhee
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
~~~~

Use `//` line comments for both Rust and TypeScript/JavaScript files.  Place
the block before any other content (imports, `#![...]` attributes, module
docs, etc.).

Document every new public Rust API with rustdoc comments. Public documentation
should explain the API's role, important invariants, and where it fits in the
pipeline so that `mise run doc` stays useful as an API review gate.


Writing docs
------------

After editing any Markdown file, format it with `hongdown -w`:

~~~~ sh
hongdown -w path/to/file.md
~~~~

Follow these prose conventions:

 -  Use sentence case for headings and subheadings, not title case.
 -  Avoid em dashes (—); use a comma, semicolon, or rewrite the sentence.
 -  No spaces around slashes (write “input/output”, not “input / output”).
 -  Use italics for file paths and file names (*CONTRIBUTING.md*,
    *src/main.rs*), and for document, section, and book titles (*DESIGN.md*,
    *Rendering modes*).
 -  Wrap dictionary names in italics for English prose
    (e.g., *Standard Korean Dictionary*, *Open Korean Dictionary*) and in
    double angle brackets (《…》) for Korean prose
    (e.g., 《標準國語大辭典》, 《우리말샘》).
 -  Wrap inline code in backticks (`mise run check`, `--flag`).
 -  Use the official spelling of proper nouns exactly.  If unsure, verify on
    the official website (e.g., Node.js, not NodeJS or Node).
 -  Treat “hangul” and “hanja” as common nouns. Use lowercase within sentences
    and headings, and capitalize them only at the start of a sentence or
    heading, for example, “Hanja variant sets”. Ethnic and language names are
    proper nouns and take an initial capital: Korean, Sino-Korean, Chinese.
 -  When linking between pages of the Rspress documentation site under *docs/*,
    use a relative path ending in the *.md* or *.mdx* extension, optionally
    followed by an anchor (for example,
    `../cli/dictionary.md#building-a-custom-dictionary`).  The explicit
    extension keeps the anchor intact through `hongdown`.

### Korean documentation

Before writing, translating, or editing Korean documentation, read and follow
the bundled
[korean-mixed-script skill](.agents/skills/korean-mixed-script/SKILL.md),
including its referenced tables and relevant guidance. Use the repository's
copy even when a system-wide copy is installed, so the conventions match this
checkout. Keep native Korean number words in Hangul.

For `ko-Kore` documentation, also follow the
[terminology glossary](GLOSSARY.ko-Kore.md). It records the selected
wording, readings, and distinctions between concepts. Apply it to the Korean
README and design document as well. When introducing a term or changing a
selected form, update the glossary and the affected Korean prose together.
Preserve code identifiers and reading examples, and include the original
spelling at each document's first mention of an unfamiliar technical term.

The skill is installed from [dahlia/dotfiles] with `npx skills`. Keep its files
and *skills-lock.json* together when updating it.

[dahlia/dotfiles]: https://github.com/dahlia/dotfiles/tree/main/HOME/.agents/skills/korean-mixed-script


Writing the changelog
---------------------

When adding a feature, changing existing behaviour, or fixing a bug, add or
update a [Sacho] fragment under *changes.d/*. Fragments are the source of truth;
*CHANGES.md* materializes the current development version automatically.
Documentation-only changes, including edits under *docs/*, prose in Markdown
files, and code comments, do not need an entry.

Create a fragment named after the change in each affected public surface:

~~~~ sh
mise exec -- sacho add --section "gukhanmun-core (Rust)" clearer-readings
mise exec -- sacho add --section "@gukhanmun/wasm (JavaScript/TypeScript)" clearer-readings
mise exec -- sacho add --section "gukhanmun (Python)" clearer-readings
~~~~

Rust crate fragments live under *changes.d/rust/*, JavaScript/TypeScript
package fragments under *changes.d/javascript/*, and Python distribution
fragments under *changes.d/python/*. Source trees stay under *crates/*,
*packages/*, and *python/*, respectively. Section IDs and headings include the
language, such as `gukhanmun (Rust)` and `gukhanmun (Python)`; quote the
complete ID when passing `--section`. The patterns in *sacho.toml* render Rust
first, then JavaScript/TypeScript, then Python, with IDs sorted within each
family.

The seven initial Python sections are `gukhanmun (Python)` and
`gukhanmun-{stdict,opendict,loanwords}-{fst,cdb} (Python)`. The latter is
shorthand for six section IDs, such as `gukhanmun-stdict-fst (Python)`. Sacho
can create fragments in these sections before their source directories exist;
no placeholder package is needed. Changes in the private
*crates/gukhanmun-python/* binding belong to `gukhanmun (Python)`, without a
separate Rust fragment.

Repeat a change under every affected surface, including the crate where the
behaviour originates and each umbrella crate, CLI, binding, or shared type
package that exposes it. This duplication is intentional: a reader should be
able to follow only the sections for the components they use. Changes to
internal generators belong under the public crate whose behaviour or data they
alter, not under the generator itself. Related changes from one pull request
can share a fragment as several bullets in each affected section.

For a workspace-wide change, repeat the relevant note in each affected public
crate or package section. All fragments belong to a section directory.

Keep each bullet focused on one user-visible change. State what changed and how
users access or configure it; include implementation details only when they
explain an observable constraint or compatibility decision. End every bullet
with bracketed issue or pull request references, such as `[[#42], [#64]]`.
Run `mise exec -- sacho resolve-links path/to/fragment.md` to resolve the
references and generate their frontmatter pins. Pins already in the frontmatter
take precedence over resolved URLs. For a pull request that has not been
created yet, pin its exact URL explicitly after resolving existing references;
a redirect lookup cannot resolve an absent pull request. For example:

~~~~ yaml
---
links:
  "#64": https://github.com/dahlia/gukhanmun/pull/64
---
~~~~

After editing a fragment, run `mise exec -- hongdown -w path/to/fragment.md`
as for other Markdown. Then apply Sacho's final formatting and regenerate and
validate the changelog:

~~~~ sh
mise exec -- sacho sync --force
mise exec -- sacho fmt
mise exec -- sacho check
~~~~

`sync --force` rebuilds the generated region from fragments without prompting,
so keep every intended unreleased edit in a fragment.

Commit the fragments and generated *CHANGES.md* together.
`mise run check` validates fragments and materialized output, and
`mise run fmt` formats them. Sacho owns formatting for *changes.d/* and
*CHANGES.md*, so the repository-wide Hongdown pass excludes them. Source-path
coverage enforcement is disabled; contributors choose the affected sections
using the public-surface rules above.

Historical section maps in *sacho.toml* let `sacho carry <version>` restore old
released notes into the current language families without editing released
history. Current sections use explicit titles so `carry` recognizes them before
applying historical title patterns. Repeated `sacho carry` runs replace the
fragment files they generated, so inspect and edit those files after the last
run.

When merging unreleased fragments from a branch that still uses
*changes.d/crates/* or *changes.d/packages/*, move each fragment into
*changes.d/rust/* or *changes.d/javascript/* before running
`sacho sync --force`. Keep each fragment's contents and pins unchanged, then
run the formatting and validation commands above. `mise run check` rejects
fragments left in the legacy directories, outside a component section, or in a
Rust section for the private Python binding.

Run `mise run changelog-test` to verify historical and repeated carry and the
Python section reservations in isolated fixtures.

The official Sacho agent skill is installed under *.agents/skills/sacho/*.
In a fresh clone, run
`mise exec -- sacho init --no-interactive --integration-executable sacho` once
to register the merge drivers from *.gitattributes* in local Git configuration.

[Sacho]: https://sacho.dev/


Testing
-------

The test suite is laid out along the four axes documented in
[*DESIGN.md*](./DESIGN.en.md):

 -  **Regression fixtures** live under *tests/fixtures/* at the workspace root
    and are consumed by `cargo test -p gukhanmun --test fixtures`.  Each
    fixture is an `<stem>.input.<ext>` / `<stem>.expected.<ext>` pair with an
    optional `<stem>.toml` sidecar describing preset, dictionary records,
    homophone window, recovery policy, or assertion kind.  Sidecar fields are
    parsed by *crates/gukhanmun/tests/common/mod.rs*.

 -  **Property-based tests** live in *crates/gukhanmun-core/tests/properties.rs*
    and share generators with *crates/gukhanmun-core/tests/common/mod.rs*
    (`arb_hangul_only_string`, `arb_mixed_script_chunks`).  Existing
    case-driven assertions in *core\_mvp.rs* are not replaced; new properties
    should pull from `common::*` so the generator surface stays consistent.

 -  **Snapshot tests** use `insta` and live in
    *crates/gukhanmun-core/tests/snapshots.rs*.  The recorded shape is the
    test-layer projection `common::tokens_to_snapshot_value`, not a derived
    `Serialize` on the public types—internal renames inside `gukhanmun-core`
    do not churn *.snap* files automatically.

 -  **CommonMark conformance** lives under *tests/fixtures/commonmark/* and
    runs through the same fixture harness.  Each case is a hanja-free
    Markdown input plus the expected pulldown-cmark-to-cmark output, pinning
    that Gukhanmun does not perturb syntax it has no opinion about.

### Authoring a new fixture

1.  Add `<stem>.input.<ext>` (and `<stem>.toml` if the fixture needs a
    non-default preset, dictionary, or recovery policy) under
    `tests/fixtures/<category>/`.
2.  Run
    `GUKHANMUN_BLESS_FIXTURES=1 cargo test -p gukhanmun --test fixtures <filter>`
    once to capture the current converter output as `<stem>.expected.<ext>`.
3.  Run the same command without `GUKHANMUN_BLESS_FIXTURES=1` to confirm the
    new baseline holds, then commit the new files.

The harness normalises each fixture stem into a `libtest`-friendly test
name by replacing hyphens with underscores.  A fixture saved as
*html/initial-sound-raw.input.html* is therefore reported (and filtered)
as `html::initial_sound_raw`.

Bless mode tolerates a missing expected file (so the very first run works)
but only blesses `assertion.kind = "exact"`; `contains` fixtures must list
their needles in the sidecar.

### Updating an `insta` snapshot

`cargo-insta` is installed automatically by `mise install` as a managed
tool.  After editing engine behaviour, run the interactive reviewer:

~~~~ sh
mise run insta-review
~~~~

Accept the new baseline only after confirming the diff reflects the
intended change.  Snapshot review is intentionally not part of `mise run test`
because it requires a TTY.


AI usage
--------

If you use AI tools (such as Claude Code, GitHub Copilot, Cursor, etc.) while
contributing, you must disclose this in your pull request description and
commit messages.  See [*AI\_POLICY.md*](./AI_POLICY.md) for the complete policy.

In commit messages, disclose AI assistance with an `Assisted-by` trailer of the
form `AGENT_NAME:MODEL_VERSION`, one line per tool.  Do not use `Co-authored-by`
for AI assistants; that trailer is reserved for human co-authors.

~~~~
Assisted-by: Claude Code:claude-opus-5-5
Assisted-by: Codex:gpt-6.1-sol
~~~~


Release process
---------------

The single source of truth for the version is `workspace.package.version` in
*Cargo.toml*.  This repository keeps it set to the *next* planned release, not
the last released one.  Branch builds therefore publish development versions
such as `1.2.4-dev.456+<sha>` while `1.2.4` is in development, and the release
itself is just a signed tag whose name equals that version.  After releasing
`1.2.4`, immediately bump the workspace to `1.2.5` before landing new work.

CI derives every published version from `workspace.package.version` at publish
time: the `x.y.z-dev.N+<sha>` development publishes on `main` and the `x.y.z`
release publishes on a tag, restamping the Rust and JavaScript manifests alike.
You never hand-edit those versions for a release; the bump command below is the
only place a contributor sets a version.

### Version bumps

Version bumps go through `mise run bump-execute`, which wraps `cargo-release`.
`cargo-release` updates the shared workspace version, the intra-workspace
dependency requirements, and *Cargo.lock*, then writes a signed commit.  It does
not update the npm, JSR or Python manifests. The task also syncs every
*package.json* and *deno.json* under *packages/* and every Python
_python/\*/pyproject.toml_, including the exact pins of the core package's
dictionary extras, to the applied version, refreshes *uv.lock*,
and runs `sacho next`. Python versions use [PEP 440] spelling, such as
`1.2.0rc2` for the Rust version `1.2.0-rc.2`. The `bump-execute` task amends the
signed commit with those manifests, the lockfile, *changes.d/next*, and
*CHANGES.md*. It never tags, pushes, or publishes.

Artifact builds can stamp a different Python version without touching
*Cargo.toml*: `PYPI_VERSION=1.2.5.dev3 mise run py-sync-version` writes it to
every Python manifest and extras pin, then refreshes *uv.lock*. The value must
be the PEP 440 spelling of the workspace version, optionally followed by
`.devN`; local labels such as `+sha` are rejected. Do not commit such a
stamped version. `bump-execute` clears any inherited `PYPI_VERSION` so a
local bump always synchronizes to the version Cargo actually applied.

Run the bump on a clean working tree: `cargo-release` refuses to proceed when
there are uncommitted changes, and the bump amends its own commit.

Before bumping, make sure your local Git signing setup can sign both commits
and tags:

~~~~ sh
git config user.signingkey
git config commit.gpgsign
git config tag.gpgSign
~~~~

Preview the bump first; this runs `cargo-release` in dry-run mode and changes
nothing. It also reports that the applied bump will advance Sacho to the new
workspace version:

~~~~ sh
mise run bump -- 1.2.5
~~~~

Inspect the planned workspace and intra-workspace dependency updates.  When the
preview is correct, create the signed bump commit, which covers the Rust crates
and every published JavaScript package under *packages/* and the core Python
distribution together:

~~~~ sh
mise run bump-execute -- 1.2.5
~~~~

Push the bump commit normally:

~~~~ sh
git push
~~~~

[PEP 440]: https://peps.python.org/pep-0440/

### Tagging a release

Review the fragments and compile the current version into a dated release
section before tagging:

~~~~ sh
mise exec -- sacho preview
mise exec -- sacho release
~~~~

Commit *CHANGES.md*, the removed fragments, and the removed *changes.d/next*
together. The GitHub Release body is cut from that frozen `Version x.y.z`
section. The workspace version stays unchanged until the next cycle's bump.

A release does not bump anything; it tags the version that
`workspace.package.version` already holds.  Create and push a signed tag with
that exact version:

~~~~ sh
git tag -s 1.2.4 -m "Release 1.2.4"
git push origin 1.2.4
~~~~

The *main.yaml* workflow verifies that the tag is signed and matches
`workspace.package.version`, runs the full CI gate, verifies locked crate
packages, publishes all crates to crates.io and all packages to npm, JSR and
PyPI, builds release binaries, and creates the GitHub release. The Python
upload uses the tested artifact ID and pinned manifest digest; the Release
attaches those same 31 files plus *python-manifest.json* and
*python-SHA256SUMS*.

If a new workspace crate or JavaScript package is added, seed its name on the
relevant registry (crates.io, npm, or JSR) once before relying on trusted
publishing for it.  Trusted publishing is used for normal release and
development publishes after the package already exists.

After the release succeeds, start the next development cycle immediately by
bumping to the next version:

~~~~ sh
mise run bump-execute -- 1.2.5
git push
~~~~

### npm CLI artifacts and publication

The seven _packages/cli\*_ templates are intentionally excluded from the pnpm
library workspace. The npm CLI pack job stages copies under *target/npm-cli/*,
reuses the six existing CLI archives and shared completion bundle, and runs
`npm pack --ignore-scripts` once. Each platform package includes both
`gukhanmun` and `gukhanmun-mkdict`, with their hashes and executable modes
validated independently. Each executable package includes the software and data
notices. The parent package (`@gukhanmun/cli`) contains all five completion
files.

`mise run npm-cli-unit` checks the launcher, packaging and publication helpers,
including version stamping in an isolated directory. `mise run check` includes
these checks. The native installation tests run as separate workflow jobs:
all six OS/architecture pairs, Linux on glibc and native-architecture Alpine,
and the Node.js 22 user floor. They install the original packed tarballs
through a temporary local registry with normalized npm metadata, without
install scripts, dependency rewrites or public-registry fallback.

The candidate is one immutable artifact retained for 90 days. Its ID and
manifest digest bind the native tests and publisher to the same seven
tarballs, hashes, source commit and workflow run. The dedicated
`publish-npm-cli` job waits for those tests and publishes only those bytes.
The existing JavaScript library publisher does not pack or publish CLI
templates.

Eligible upstream `main` and `X.Y-maintenance` pushes publish under `dev`;
matching signed stable tags publish under `latest`. PRs, forks and other
branches build and test without npm publication credentials. Full
`X.Y.Z-dev.N+sha` versions remain in tarballs, exact optional pins, completion
metadata and native version output; npm registry identity and dist-tags drop
the `+sha` suffix. The manifest records the full commit independently.

The publisher uses a GitHub-hosted runner, pinned Node.js/npm tooling,
`id-token: write` and explicit provenance. It accepts no registry-token
fallback. Before uploading the parent, it requires all six platform packages
to be visible with matching integrity and provenance. It checks selected and
nonselected tags, installs through the real registry, and audits signatures in
a temporary local project. These checks cannot be replaced by local tarball
tests.

#### One-time registry bootstrap

All seven npm packages need an initial functioning package and their own
trusted publisher configuration. This is a separate maintainer operation, not
part of normal CI:

1.  Publish development candidates explicitly under `dev`. On first
    publication, npm can also assign `latest` to that development version and
    create a `0.0.0-stage` placeholder. This bootstrap state is accepted:
    installs without a tag or version, such as `npm install @gukhanmun/cli`,
    may select the initial development version until the first stable release.
    Later development publications must leave the existing `latest` tag
    unchanged.
2.  Download one successfully tested, upstream development candidate. Verify its
    manifest and original artifact identity. With short-lived operator
    authentication, publish those exact functioning tarballs under `dev`,
    platform packages first and parent last. Do not seed dummy launchers or
    independently rebuilt binaries. Verify the public tarball hashes and that
    `dev` points to the published version; `latest` may point to it as well.
    Allow for registry processing and cached responses when checking visibility,
    then revoke the temporary credential.
3.  Configure each package for owner `dahlia`, repository `gukhanmun`, workflow
    filename `main.yaml`, with no environment. Explicitly allow direct
    `npm publish`: new configurations otherwise allow staging only. This
    workflow does not use `npm dist-tag` or need its separate permission.
4.  Push a new upstream source revision for the first normal OIDC publication
    within two days of creating the configurations, before they expire.
    Do not rerun the manually seeded version: its provenance may not meet the
    normal publisher's checks. Missing packages or incorrect OIDC settings fail
    with setup instructions, never a token fallback.

See [npm's Trusted Publishing documentation] for current tooling requirements
and settings. No npm account or registry setting is changed by local tests.

#### Partial publication and recovery

Publication is parent-last, not atomic. Rerun only the failed publisher against
its original artifact ID and manifest digest. Never rebuild or rerun all jobs
after an upload. An existing version is skipped only when its original
integrity and provenance match; conflicts or expired artifacts require a new
version. Receipts record the attempt and partial result. Anomalous dist-tags on
already-published versions require operator investigation; CI does not repair
them automatically.

One noncancelling concurrency group serializes both channels. GitHub can replace
a pending job with a newer pending job; rerun a cancelled release explicitly
when needed. Stale development runs and lower stable patches in the same
release line are rejected before writes. As in the existing JavaScript
publisher, a signed maintenance release can move `latest` to a lower release
line. Development publication must leave `latest` unchanged.

[npm's Trusted Publishing documentation]: https://docs.npmjs.com/trusted-publishers/

### Python publication and bootstrap

Python publication follows the existing eligible pushes: `main` and
`X.Y-maintenance` in `dahlia/gukhanmun` publish development versions, and
matching signed `X.Y.Z` tags publish stable versions. PRs, forks and feature
branches build and validate artifacts without Python publishing credentials.
Python versions use [PEP 440], never the Rust/npm `+sha` suffix; the source SHA
is retained in the manifest and native build records. The core package's
extras pin all six data distributions to exactly that Python version. Bump the
workspace to the next version immediately after a stable release: `X.Y.Z.devN`
sorts before `X.Y.Z`, so development builds of the released base version would
not be preferred over that stable release.

The `publish-python` job waits for the full gate, semantic matrix and complete
artifact validation. It installs no Rust and rebuilds nothing. Only that Python
job gets `id-token: write`; it uses [Trusted Publishing] with no long-lived
PyPI token fallback. Both the release JSON and [Simple index] inventories must
confirm file hashes before publication is complete. Data files become visible
before the native package is uploaded. Development publication creates no
GitHub Release.

All seven PyPI projects must exist and carry normal Trusted Publishers before
the first CI upload. Pending publishers cannot bootstrap seven projects at
once: their identity is unique per owner/repository/workflow/environment, and
one account can hold at most three pending registrations. Use this one-time
seed procedure instead:

1.  Merge the Python version-stamping change into `main` first. Start from a
    clean, disposable checkout of `main` at the source commit to release. Seed
    shortly before the first normal release, because pip can select a
    development version when no stable candidate satisfies a request.

2.  Stamp the workspace version plus `.dev0` in every Python manifest and extra
    pin. For workspace `0.4.0`, use the commands below. Choose a matching native
    target if the host is not Linux x86\_64; Linux builds require Docker or
    Podman as described above. Do not commit the stamped manifests or lockfile.

    ~~~~ sh
    PYPI_VERSION=0.4.0.dev0 mise run py-sync-version
    mise run py-sdist-test
    mise run py-dist -- linux-x86_64
    ~~~~

3.  Stage one audited native CPython wheel, the six data wheels and all seven
    sdists in a fresh directory. The seed needs 14 files, not the full release
    matrix. Replace `linux-x86_64` in the paths below with the target you built.
    Use only files from that same stamped checkout:

    ~~~~ sh
    seed_dir=$(mktemp -d)
    cp target/python/release/linux-x86_64/wheels/*-cp311-abi3-*.whl "$seed_dir/"
    cp target/python/dictionaries/gukhanmun-*/*.whl "$seed_dir/"
    cp target/python/sdist/*.tar.gz target/python/dictionaries/gukhanmun-*/*.tar.gz "$seed_dir/"
    ~~~~

4.  Create a temporary account-scoped PyPI API token; project-scoped tokens
    cannot create the missing projects. Supply it as `UV_PUBLISH_TOKEN` without
    putting it in shell history, repository files or logs. Perform the single
    local seed upload, then immediately revoke the token:

    ~~~~ sh
    mise exec --no-deps -- uv --no-config publish --trusted-publishing never "$seed_dir/"*
    ~~~~

5.  Register a normal Trusted Publisher on each project: `gukhanmun`,
    `gukhanmun-stdict-fst`, `gukhanmun-stdict-cdb`, `gukhanmun-opendict-fst`,
    `gukhanmun-opendict-cdb`, `gukhanmun-loanwords-fst` and
    `gukhanmun-loanwords-cdb`. Use owner `dahlia`, repository `gukhanmun`,
    workflow filename `main.yaml` and no environment. Confirm all seven
    registrations in PyPI before relying on CI publication. Normal publishers
    can share this identity across existing projects.

The seed has its own `.dev0` version; workflow run numbers are positive, so
normal development uploads use different versions. The normal uploader refuses
`.dev0`. Until bootstrap is complete, an eligible CI upload fails with setup
instructions rather than silently skipping publication. No-environment Trusted
Publishing trusts the repository/workflow identity: the ref checks in code and
YAML do not protect against a collaborator who can change that workflow. Tag
verification preserves the existing signer policy and adds source-commit
binding; it does not require a particular signer or branch ancestry.

[Trusted Publishing]: https://docs.pypi.org/trusted-publishers/using-a-publisher/
[Simple index]: https://packaging.python.org/en/latest/specifications/simple-repository-api/

### Partial Python publication and recovery

PyPI uploads are not atomic. Each attempt writes *python-publish-receipt.json*
and a step summary with completed, missing and conflicting files. The receipt
records the original artifact ID, manifest digest, run, commit and attempt;
receipt artifacts have attempt-specific names. A runner cancelled abruptly
may not write a final receipt, so the next attempt always reads PyPI again.

Rerun the failed upload job against its original successful `python-dists`
artifact. Do not rerun successful build jobs or select **Re-run all jobs** after
any upload. A rebuild can produce different bytes under the same filenames;
only an existing file with the same SHA-256 and size is skipped. Any conflict
fails before further uploads. An upload response lost in transit counts as
success only after the registry confirms identical content.

Recovery uses the original artifact ID and manifest digest. If that artifact
has expired, publish a new version; never accept a rebuilt hash as a duplicate.
PyPI cannot reuse deleted filenames. A conflicting or deleted stable filename
therefore requires a new patch version. The normal pipeline deliberately has no
arbitrary-run recovery input or overwrite switch.

Ongoing development releases consume PyPI storage. Remove obsolete development
releases or request a quota increase when needed, remembering that removal
does not make their filenames reusable.

The credential-free `python-publish-check` job can be rerun independently by
its job ID to check recovery downloads. Its next receipt must retain the same
artifact ID and manifest hash while recording a newer current attempt. Its
offline validation does not request PyPI credentials or contact the registry.

### Standalone Python CLI distribution

The independent `gukhanmun-cli` PyPI project contains both existing release
executables, `gukhanmun` and `gukhanmun-mkdict`, a shared standard-library
launcher and the five shared completion scripts for `gukhanmun`.
It has no dependency on the `gukhanmun` Python library or its data
distributions. It ships six `py3-none` wheels, not extension wheels or an
sdist. The existing binding/data release remains a separate set of 31 files.

*scripts/python/cli\_artifacts.py* defines the CLI matrix. The two Linux wheels
use compressed manylinux 2.28/musllinux 1.2 platform tags for statically linked
musl executables; macOS wheels use macOS 11 tags, and Windows wheels use
`win_amd64`/`win_arm64`. Native release checks inspect linkage and architecture,
install outside the checkout without Rust or other Gukhanmun distributions,
and execute the same wheel on the minimum/current CPython and available
PyPy interpreters. Each interpreter and installer also builds FST and CDB
dictionaries with the installed `gukhanmun-mkdict` and loads them with the
installed converter. Both executables have their own hashes and platform audits.
Linux checks also execute inside both digest-pinned container
images, glibc and musl. Those containers show userland compatibility only;
they do not show that the executable runs on an old Linux kernel.
Windows checks include native console control events through
the installed launchers.

To build and test a native CLI wheel locally, install the interpreters from
*mise.python.toml*, then run:

~~~~ sh
mise run py-cli-local -- --target linux-x86_64
mise run py-cli-libc-test -- --target linux-x86_64 --dir target/python/cli-local
~~~~

Choose the matching target from `scripts/python/cli_artifacts.py matrix`.
Linux builds need a musl C compiler as well as the Rust target because the
CLI includes native C dependencies. `mise run py-cli-native -- <rust-target>`
installs the Rust target and builds both existing executables, but does not
provision that C compiler. The libc tests use Docker or Podman with the same
engine and SELinux volume settings as `py-dist`. The local command never
stamps source versions or publishes. Add
`--shell bash --shell zsh --shell fish --shell nushell` when those shells are
installed; Windows uses `--shell powershell`. `py-cli-prepare` and
`py-cli-test` repeat the installer and host checks without rebuilding.

The `cli-dists` bundle contains only the six CLI wheels and their own build,
audit and execution evidence, with a separate manifest and checksums. The
credential-free `cli-publish-check` job checks the bundle against its original
immutable artifact ID and manifest digest. The `publish-python-cli` upload job
uses that same bundle, obtains an OIDC token only in that job, and never
rebuilds. Eligible events, signed-tag verification and Python version spelling
follow the existing Python release policy. The CLI's native `--version` retains
the Rust development spelling; wheel metadata uses PEP 440 version strings, and
provenance binds both versions to the source commit. Version bumps synchronize
all eight Python manifests without adding CLI extras to the core package.

#### Bootstrap the CLI project

Before the first eligible CI upload, register a [pending Trusted Publisher] for
project `gukhanmun-cli`, owner `dahlia`, repository `gukhanmun`, workflow
*main.yaml*, and no environment. The first authorized OIDC publication creates
the project and converts the pending publisher into a normal publisher.
No manual seed upload or API token is needed. A pending publisher does not
reserve the project name.

Only the CLI uploader permits an absent project and treats missing registry
inventories as empty. Artifact validation, event authorization, conflict checks
and confirmation through both registry inventories still apply. The existing
seven library/data projects retain their separate bootstrap procedure.
Until a suitable stable release exists, ordinary tool installation can select
a development release.

#### CLI publication recovery and retention

Rerun only the failed `publish-python-cli` job with the original `cli-dists`
artifact ID and manifest digest. Do not rerun successful build jobs or select
**Re-run all jobs** after any wheel was uploaded. Attempt-specific
*cli-publish-receipt.json* records submitted, completed, missing and conflicting
files. Both PyPI's release JSON and Simple index must confirm matching content;
a lost upload response alone is not proof of success. A hash or size conflict
stops publication before further uploads.

CI retains the immutable bundle for 90 days. If it expires, publish a new
version rather than rebuilding files under old names. Deleted PyPI filenames
cannot be reused; a conflicting or deleted stable filename requires a new
patch release. Monthly, review obsolete CLI development releases and retain
the newest ten plus every version involved in active recovery. Remove older
development releases manually when safe, or request more PyPI storage. No
automated deletion runs in the workflow.

[pending Trusted Publisher]: https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/
