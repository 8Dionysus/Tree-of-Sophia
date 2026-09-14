# Historical generations and reader authority: implementation review

Reviewed on 2026-09-07 by `agent:codex-tos-foundation`, against the change
following `325393b8ad3195381f747f907d7f84255e2bbef1`.
This is an implementation and source-boundary review, not independent
historical assessment, admission, UI acceptance or Foundation v1 completion.

## Source work and meaning

Two HistoricalGeneration records under
`ToS/source-witnesses/history/generation98-research/` express the criteria
attributed to Azorín (1913) and Salinas (1934) in Fox's historiographical study.
Different identity criteria deliberately receive different IDs. They are not
two descriptions silently competing for one immutable criterion, two
Organizations, or intervals inferred from a shared name.

One Agent under `ToS/source-witnesses/agents/miguel-de-unamuno/` participates
in both through two qualified `generation_member` Claims under
`ToS/source-witnesses/relations/generation98-research/`. The dates describe
the proposed classifications, not membership dates. Both reports cite the
same Fox article; this does not constitute independent corroboration.
The [source-reading note](2026-09-07-generation98-source-reading.md) is frozen
command evidence, separate from this implementation review. Original Azorín
and Salinas texts were not independently inspected. No homogeneous beliefs,
institutional identity, influence or causal relationship were added.

All three subjects and two Claims were created through the existing ordinary
owner commands, with separately scoped trusted authority, prepared version
checks and exact idempotent retries. Source packages retain requests,
receipts, provenance and human forms. Each subject has Russian, English and
Spanish names plus a Russian hover description; each Claim has a Russian
statement with the full Claim as mandatory context. Identities remain
provisional, Claims unreviewed and forms unverified, without admission.
No new profile-specific implementation was necessary.

## Reader defect and correction

The real probe exposed an incorrect default: `_normalize_node` treated the
`source-claims` carrier as `canon` when no authority posture was recorded.
It now defaults to `canon` only for the canonical carrier and to
`derived-export` for all others, matching relation normalization. Explicit
source posture and actual review status remain intact. No source record,
review event or canon status is rewritten by this correction.

A new contract test first failed for all four carrier kinds: identity,
Claim, evidence and literal. It checks the corrected reader output, absence
of invented canon status, retained `unreviewed` status, input immutability,
unchanged canonical-carrier defaults and explicit source-posture preservation.
The normalization cache's existing processor digest includes this function's
AST and transitive helpers, so previous cached normalization is not reusable
under the changed processor. This is not a hot-runtime deployment claim.

## Real reader observation

The ordinary `ToSAccessCore` preserved exact source records in both
`source-claims` and `source-navigation`, all three name languages, Russian
hover and Claim statement forms, full qualification context and absent
admission. Both carrier families and Claims retained derived authority and
no canon status; Claims retained `unreviewed`. No numeric time semantics
were inferred from the cohort years.

Depth-2 focus with limits 100 nodes/200 relations returned 7 nodes/7 relations
for Unamuno and 5/4 for each cohort. The scene contained one person vertex
with both source-carrier IDs, not duplicate people. Both cohort → person
and person → cohorts navigation succeeded. An initial probe had incorrectly
expected one raw carrier; it was corrected to check the scene's actual
identity contract without changing product code to discard source records.

The post-fix observation used source snapshot
`8ce43c822089d7bb3a7681a48c6279d455450f007e5317279c38223fc6bbc319`:
cold construction 27.267 seconds, focus calls 0.332/0.310/0.295 seconds,
peak RSS 1,274,360 KiB. These are single local observations, not p95, budget
acceptance or an indexed-write performance claim. Later documentation
rebuilds may change the source snapshot.

## Validation observed

- `python -m unittest discover -s access/tests -p test_knowledge_contract.py`:
  all 59 tests passed in 31.733 seconds after the explicit-posture and
  nonempty canonical-carrier controls were added.
- The existing historical-context profile contract test passed in
  0.394 seconds, covering generation/period/Organization distinctions and
  the scoped membership predicate without changing those contracts.
- Source-foundation, source-claim graph and source-home validators passed
  on the new records. The corpus index rebuilt and validated successfully.
- The route-card validator initially found stale generated currentness after
  the access README changed. The owner builder refreshed its companion;
  validation then passed for all 56 cards.

Two command-selection mistakes (a pluralized test class and a reversed
documentation-validator filename) failed before running the requested check.
The declared test class and validation lane remain authoritative; neither
failure justified changing product behavior.

## Review and remaining owner work

Yes: source return, authored/derived separation, distinct record/subject/
Claim/form layers, stable shared person identity, language-bound forms,
attribution, uncertainty, contextual years, explicit next owner and no
automatic admission. Not applicable: canon modification, private payload,
lived witness, counterpart, calibration, tiny entry and release publication.

Independent assessment remains with the ToS source-review owner. The current
owner-routed reviewer preparation found no qualified model fit for the exact
runtime and required source-reading tools; no actor was launched and no
self-review was relabeled independent. A future reviewer must have appropriate
Spanish source-reading competence and current owner-approved tool/runtime
fit before the frozen reports can be assessed for scoped scholarly-report use.
Stronger historical membership claims require the underlying sources.

H04 remains partial: a real contested generation classification now exercises
the existing growth/reader contract, but contemporaneity and comparison of
transmission speed or reception lag are not established. Other source areas,
agent assessment, Worker/D1, UI interaction, growth cost, CI, merge and
deployment are not proved here. Rollback may replace the derived reader but
must preserve these source packages and their history. Source corrections
remain separately versioned owner commands.
