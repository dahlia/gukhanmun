Bundled loanword examples
=========================

`gukhanmun-loanwords` embeds the National Institute of Korean Language's
loanword spelling examples as an offline FST dictionary. The source snapshot,
explicit editorial decisions and audit report live under *data/*.
The runtime package excludes the full source snapshot and bulky audit report.

~~~~ rust
assert_eq!(gukhanmun_loanwords::dictionary().lookup("香港").unwrap().unwrap().reading(), "홍콩");
~~~~

The umbrella crate and CLI enable this dictionary by default for `ko-kr`,
while `ko-kp` leaves it disabled. Use `Builder::no_bundled_loanwords()` or
`--no-loanwords` to disable it, and `Builder::bundled_loanwords()` or
`--loanwords` to enable it for `ko-kp`.
User dictionaries take priority, then loanwords, then the preset dictionaries.
JavaScript callers load `@gukhanmun/loanwords-fst` or
`@gukhanmun/loanwords-cdb` after user dictionaries and before the preset
dictionaries.

See *ATTRIBUTION.md* and *LICENSE* for source permissions, which are separate
from the GPLv3-or-later software terms. Ordinary builds need no API key.


Regeneration and scheduled updates
----------------------------------

Run `mise run loanwords-check` to verify the pinned snapshot, decisions, TSV,
provenance and report without contacting the API.
`mise run loanwords-regenerate` regenerates those artifacts offline after an
editorial change. For a live update, set `NIKL_LOANWORD_API_KEY` in your
environment and run `mise run loanwords-update`. The explicit
`mise run loanwords-update-local` variant loads the key from the git-ignored
*.env* file. Never put the key in a command argument, source snapshot or report.

Once *loanwords.yaml* exists on *main*, the workflow runs on Sundays at
03:17 UTC and can also be started manually from *main*. Store the key as the
repository secret `NIKL_LOANWORD_API_KEY`. Only the collector built from the
workflow's pinned *main* commit receives it. One complete source snapshot,
checked for credential leaks, feeds every target. Separate read-only jobs
build and extract each target offline without the key; a publishing job
receives the write token and applies only validated data and changelog paths.
Target build caches are not shared with *main*. Failed collection preserves
the pinned data.

Data updates target the maintenance branch of the newest stable release
series that already shipped this dictionary, comparing version numbers
numerically. A release tag such as *0.10.1* must contain this dictionary and be
an ancestor of *0.10-maintenance*. Both annotated and lightweight stable tags
count; prereleases do not. Before the dictionary's first stable release,
updates target *main*. After shipment, a missing or incompatible maintenance
branch fails visibly instead of falling back to an older series or *main*.
Release tags and maintenance branches must be controlled by maintainers.

The workflow also prepares a separate *main* PR from the same source using
*main*'s own decisions and changelog, so later releases retain the update.
Each target has its own bot head: *automation/loanword-data* for *main* and,
for the 0.10 series, *automation/loanword-data-0.10-maintenance*. Open bot PRs
for older series produce a warning for manual cleanup. No update PR is merged
automatically.

Each target runs offline CI with serialized tasks and tests before publication.
The artifact records its complete tested Git tree. The publisher reconstructs
that tree from the pinned target base and verifies it before assigning a status.
An existing bot commit is reused only when its entire tree matches and its sole
parent is that base. Identical updates on an unchanged base do not refresh the
PR; data that already matches a target closes its obsolete bot PR.
Each affected package's changelog uses the fixed *loanword-data-update.md*
fragment. Updates preserve introductory notes and replace one source-update
bullet, recording the current snapshot fingerprint and source count.
The target's own `sacho sync --force`, `sacho fmt` and `sacho check` materialize
and verify its changelog. Generated changelogs are never copied between
branches.

New or stale editorial conflicts produce a draft PR containing a separate
pending audit and a failing `loanword-data-update` status. The current
snapshot, runtime dictionary and changelog remain intact. Resolve decisions on
a separate maintainer branch and rerun the updater. The updater refuses to
replace a bot branch containing human commits. Move those changes to a separate
branch, then delete the bot branch before retrying. It also refuses stale
results if the target advances or the selected release series changes during
collection. An advance of *main* alone does not block the maintenance PR.
Each matrix target completes independently; a failure makes the workflow fail
visibly, and another scheduled or manual run retries it. A busy *main* may need
a manual rerun once it settles.

The repository's Actions settings must allow workflows to create pull requests.
For PRs created with `GITHUB_TOKEN`, [GitHub requires a maintainer]
to select
*Approve workflows to run* before checks start;
GitHub App tokens can support fully unattended downstream checks instead.
The automation does not approve reviews or merge. Scheduled workflows may be
disabled by GitHub after prolonged repository inactivity; re-enable the
workflow if necessary. A successful local API call does not prove that
GitHub-hosted runners can reach the API. Verify that with an actual workflow
run.

[GitHub requires a maintainer]: https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow
