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

Document every new public Rust API with rustdoc comments. Public documentation
should explain the API's role, important invariants, and where it fits in the
pipeline so that `mise run doc` stays useful as an API review gate.

When adding a feature, changing existing behaviour, or fixing a bug, add an
entry to *CHANGES.md* under the current development version heading.
Documentation-only changes (edits to *docs/*, prose in Markdown files, or
code comments) do not need a *CHANGES.md* entry.


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
    “Rendering modes”).
 -  Wrap inline code in backticks (`mise run check`, `--flag`).
 -  Use the official spelling of proper nouns exactly.  If unsure, verify on
    the official website (e.g., Node.js, not NodeJS or Node).
 -  Write “hangul” and “hanja” in lowercase; they are common nouns, not proper
    nouns.  Ethnic and language names are proper nouns and take an initial
    capital: Korean, Sino-Korean, Chinese.


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
    `Serialize` on the public types — internal renames inside `gukhanmun-core`
    do not churn `.snap` files automatically.

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
Assisted-by: Claude Code:claude-opus-4-8
Assisted-by: Codex:gpt-5.5
~~~~


Release process
---------------

This repository keeps the workspace version set to the next planned release,
not the last released version.  Branch builds therefore publish development
versions such as `1.2.4-dev.456+<sha>` while `1.2.4` is in development.  After
releasing `1.2.4`, immediately bump the workspace to `1.2.5` before landing
new work.

Version bumps use `cargo-release` through `mise`.  The bump command updates the
shared workspace version, updates intra-workspace dependency version
requirements, and creates a signed commit.  It does not create tags, push, or
publish to crates.io.

Before releasing, make sure your local Git signing setup can sign both commits
and tags:

~~~~ sh
git config user.signingkey
git config commit.gpgsign
git config tag.gpgSign
~~~~

### Bumping the next development version

Preview the bump first:

~~~~ sh
mise run bump -- 1.2.5
~~~~

Inspect the planned workspace and intra-workspace dependency updates.  When the
preview is correct, create the signed bump commit:

~~~~ sh
mise run bump-execute -- 1.2.5
~~~~

Push the bump commit normally:

~~~~ sh
git push
~~~~

### Tagging a release

Prepare *CHANGES.md* so it contains a `Version x.y.z` section for the release.
The GitHub Release body is cut from that section.

When the current workspace version is ready to release, create and push a
signed tag with the exact same version:

~~~~ sh
git tag -s 1.2.4 -m "Release 1.2.4"
git push origin 1.2.4
~~~~

The *main.yaml* workflow verifies that the tag is signed and matches
`workspace.package.version`, runs the full CI gate, verifies locked crate
packages, publishes all crates to crates.io, builds release binaries, and
creates the GitHub release.

If a new workspace crate is added, seed that crate name on crates.io once
before relying on GitHub Actions trusted publishing for it.  Trusted publishing
is used for normal release and development publishes after the crate already
exists on crates.io.

After the release succeeds, start the next development cycle immediately:

~~~~ sh
mise run bump -- 1.2.5
mise run bump-execute -- 1.2.5
git push
~~~~
