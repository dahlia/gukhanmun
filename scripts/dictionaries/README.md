Dictionary data updates
=======================

The weekly workflow checks the authenticated JSON download listings of the
*Standard Korean Dictionary* (標準國語大辭典) and the *Open Korean Dictionary*
(우리말샘). It runs on Wednesdays and accepts a manual trigger. It opens a
reviewable data PR against the newest numeric `*.*-maintenance` line, never
`main`. Selection fails before login when that line lacks interface version 1,
staging-aware extraction tasks, metadata integration, or Sacho. Backport the
infrastructure to that maintenance line before enabling collection; the updater
does not choose an older line instead.


Local collection and regeneration
---------------------------------

Provide `NIKL_USERNAME` and `NIKL_PASSWORD` in the environment, then collect to
a new directory outside the checkout:

~~~~ sh
mise run dicts-collect -- /tmp/dictionary-source
mise run dicts-update -- /tmp/dictionary-source
~~~~

`mise run dicts-collect-local -- /tmp/dictionary-source` explicitly loads local
account configuration for collection. It does not publish anything. The update
command validates archive bytes again, uses this checkout's extractors and
curated decisions, and stages both dictionaries before replacing files. Any
failure during application or changelog generation restores the previous files.
Run `mise run ci` and inspect the diff before committing.

To inspect the newest eligible remote line locally, use
`mise run dicts-target -- dahlia` (or the name of your GitHub remote). A private
source-state file can be produced with
`mise run dicts-state -- /path/to/pinned-target /tmp/dictionary-state.json`.
The workflow uses that file to skip a download only when the committed source
identity, generated file hashes and extraction inputs still match. Missing
provenance forces collection. Open, unmerged proposals are downloaded again;
identical proposals do not receive another push. An input-code-only change
with identical source and TSVs does not open a provenance-only PR, so it can
cause repeat downloads until the next source update.


Source validation and editorial decisions
-----------------------------------------

The collector keeps account sessions in memory and restricts requests to the
sites' HTTPS origins, including the shared login host for the
*Open Korean Dictionary* (우리말샘). Its download listing needs an
`Accept-Language` header. A `200 OK` response is insufficient: unauthenticated
downloads may return login HTML.

Archive validation checks CRCs, JSON shapes, dates, contiguous shard filenames
and totals without extracting paths. The tested whole-dump format uses
5,000-record shards for the *Standard Korean Dictionary* (標準國語大辭典),
whose totals are global, and 50,000-record shards for the
*Open Korean Dictionary* (우리말샘), whose totals count raw rows per shard.
The latter includes identical duplicated records and its final filename
endpoint can differ from the row sum. Identical records sharing an ID are
counted separately in provenance; conflicting contents for one ID fail.
A missing terminal partial shard or a changed/ambiguous layout fails for
maintainer review, including exact-multiple terminal layouts. These checks
encode observed download conventions, not a guaranteed NIKL API contract.

A changed curated candidate signature, an unmatched annotation rule or a failing
behavioral assertion blocks publication. Inspect the source records and update
editorial decisions manually when justified. The updater never edits
*defaults.tsv* or *rules.tsv*. Failures include public extractor diagnostics in
the job summary, without account responses or cookies.

*metadata.json* supplies deterministic FST/CDB provenance. *source.json* also
records download identity, checksums, raw/unique/duplicate counts and output
statistics. Fixed-path Sacho fragments cover changed mappings; a source-only
update does not claim a change to dictionary behavior. Shipped snapshot notices
are regenerated with the data.


Publication and verification
----------------------------

Collection receives account secrets only for its collection step. Preparation
runs the pinned maintenance branch's code with no account or write credentials.
Only successful preparation and full `mise run ci` permit publication. Archives
and public artifacts expire after one day; no session material is uploaded.

The publisher accepts only allowlisted data, provenance, notices, fragments and
*CHANGES.md*. It checks file hashes, modes and the complete tested Git tree,
then rechecks the newest target and its head. It refuses branches containing
human commits and uses a pinned force-with-lease for bot-only updates. It never
executes code from an existing PR. Unchanged runs leave existing PRs alone;
cleanup of obsolete proposals is a maintainer task. A dedicated status records
full offline validation on the exact published commit, since PRs created with
`GITHUB_TOKEN` may not trigger the normal workflow.

Run `mise run dicts-test` for credential-free response/ZIP fixtures and real
local Git publication tests. These checks do not establish that a GitHub-hosted
runner can log in, or that repository settings allow automated PR creation.
Verify those after the workflow is deployed before relying on unattended runs.
