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


AI usage
--------

If you use AI tools (such as Claude Code, GitHub Copilot, Cursor, etc.) while
contributing, you must disclose this in your pull request description and
commit messages.  See <AI\_POLICY.md> for the complete policy.
