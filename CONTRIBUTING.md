Contributing
============

This repository uses [mise] as the single entry point for development tools and
commands.

Install the configured tools:

~~~~ sh
mise install
~~~~

List available project commands:

~~~~ sh
mise tasks
~~~~

Refer to [*DESIGN.md*](./DESIGN.en.md) when you need the overall design,
architecture, or roadmap context for a change.

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

Avoid relying on globally installed Rust tools or ad hoc command variants when
working on this repository. Add or update a mise task when the project needs a
new repeated development command.

Document every new public Rust API with rustdoc comments. Public documentation
should explain the API's role, important invariants, and where it fits in the
pipeline so that `mise run doc` stays useful as an API review gate.

[mise]: https://mise.jdx.dev/


Testing
-------

The test suite is laid out along the four axes documented in
[*DESIGN.md*](./DESIGN.en.md):

 -  **Regression fixtures** live under `tests/fixtures/` at the workspace root
    and are consumed by `cargo test -p gukhanmun --test fixtures`.  Each
    fixture is an `<stem>.input.<ext>` / `<stem>.expected.<ext>` pair with an
    optional `<stem>.toml` sidecar describing preset, dictionary records,
    homophone window, recovery policy, or assertion kind.  Sidecar fields are
    parsed by `crates/gukhanmun/tests/common/mod.rs`.

 -  **Property-based tests** live in `crates/gukhanmun-core/tests/properties.rs`
    and share generators with `crates/gukhanmun-core/tests/common/mod.rs`
    (`arb_hangul_only_string`, `arb_mixed_script_chunks`).  Existing
    case-driven assertions in `core_mvp.rs` are not replaced; new properties
    should pull from `common::*` so the generator surface stays consistent.

 -  **Snapshot tests** use `insta` and live in
    `crates/gukhanmun-core/tests/snapshots.rs`.  The recorded shape is the
    test-layer projection `common::tokens_to_snapshot_value`, not a derived
    `Serialize` on the public types — internal renames inside `gukhanmun-core`
    do not churn `.snap` files automatically.

 -  **CommonMark conformance** lives under `tests/fixtures/commonmark/` and
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
`html/initial-sound-raw.input.html` is therefore reported (and filtered)
as `html::initial_sound_raw`.

Bless mode tolerates a missing expected file (so the very first run works)
but only blesses `assertion.kind = "exact"`; `contains` fixtures must list
their needles in the sidecar.

### Updating an `insta` snapshot

`cargo-insta` is installed automatically by `mise install` through the
`postinstall` hook in `mise.toml`.  After editing engine behaviour, run
the interactive reviewer:

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
