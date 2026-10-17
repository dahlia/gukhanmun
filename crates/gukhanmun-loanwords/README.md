Bundled loanword examples
=========================

`gukhanmun-loanwords` embeds the National Institute of Korean Language's
loanword spelling examples as an offline FST dictionary. The source snapshot,
explicit editorial decisions and audit report live under *data/*.
The runtime package excludes the full source snapshot and bulky audit report.

~~~~ rust
assert_eq!(gukhanmun_loanwords::dictionary().lookup("香港").unwrap().unwrap().reading(), "홍콩");
~~~~

Use `Builder::bundled_loanwords()` in the umbrella crate or `--loanwords`
in the CLI. Both Korean presets leave this dictionary disabled by default.
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
`mise run loanwords-update-local` variant loads the key from the ignored *.env*
file. Never put the key in a command argument, source snapshot or report.

Once *loanwords.yaml* exists on *main*, the workflow runs on Sundays at
03:17 UTC and can also be started manually. Store the key as the repository
secret `NIKL_LOANWORD_API_KEY`. The collection job has read-only repository
permissions; a separate job receives the write token and applies only validated
data and changelog paths. Failed collection preserves the pinned data.

When collection succeeds and the source changed, the workflow opens or
refreshes a PR from the *automation/loanword-data* branch. The
workflow runs offline CI before publishing. Repeated identical source/editorial
content does not refresh an existing PR, and regenerated data that already
matches *main* closes an obsolete bot PR. Each affected package's changelog
section uses the fixed *loanword-data-update.md* fragment. Updates preserve
introductory notes and replace one source-update bullet in that file, recording
the current snapshot fingerprint and source count. `sacho sync --force`,
`sacho fmt` and `sacho check` materialize and verify the changelog.

New or stale editorial conflicts produce a draft PR containing a separate
pending audit and a failing `loanword-data-update` status. The current
snapshot, runtime dictionary and changelog remain intact. Resolve decisions on
a separate maintainer branch and rerun the updater. The updater refuses to
replace a bot branch containing human commits. Move those changes to a separate
branch, then delete the bot branch before retrying. It also refuses stale
results if *main* advances during collection.

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
