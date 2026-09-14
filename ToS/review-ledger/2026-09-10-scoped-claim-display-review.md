# Scoped Claim display fields and retained-form integrity

Date: 2026-09-10 UTC. Reviewer: the Foundation implementation agent, under the
operator's Foundation v1 implementation mandate. Source baseline:
`952d85f2be7851added9a145a503658241fb1a25` on the native-structure worktree.
This is source/command review, not a substantive assessment of historical
Claims, independent model consensus, final-union acceptance, or publication.

## Sources and change

- `ToS/doctrine/HUMAN_FORMS.md` owns the exact-field and mandatory-context law.
- `ToS/contracts/claim-display-fields.schema.json` bounds opt-in authored
  name/caption/hover fields and requires the complete statement.
- `scripts/source_record_profiles.py` binds the recognized contract into
  source dependencies; `scripts/source_witness_human_forms.py` copies complete
  fields with the whole exact Claim as context.
- The growth-cycle source command, Claim revision and private Claim adapters
  enforce field delegation on preparation, raw application and retry.
- TOS-D-0064 records the accepted boundary choice; its six decision indexes
  were regenerated. The script inventory records the display contract and
  the previously omitted `scripts/source_document_catalogue.py` helper.

## Source-visible review

The repository review checklist was applied to the affected boundaries:

- Source traceability, authored/derived separation, language identity,
  context retention and ToS ownership: **yes**. No statement clipping, UI
  heuristic, endpoint name or navigation descriptor becomes authored wording.
- Historical identity and version lineage: **yes**. Revisions retain earlier
  Claim/form bytes and creation evidence. Integrity reconstruction uses the
  historical actor, exact archived request and retained result references.
- Delegation and revocation: **yes**. Existing public v1, private and compound
  grants remain statement-only. Public v2 names permitted fields explicitly;
  public correction opt-in does not acquire date/value/layer authority.
  The exact predecessor remains in scope when a form changes its role.
- Replay separation: **yes**. A later independent successor does not widen the
  historical operation being retried. Corrupted result references cannot be
  substituted for the archived request's reconstructed results. Historical
  integrity is not a new materialization or a check under today's write grant.
- Unknown versions and uncertainty: **yes**. Unknown fields remain inert and
  preserved; recognized malformed fields fail closed. Mechanical readiness
  does not assess a shortening or grant use, rights, canon or publication.
- Gold, lived witness, counterpart, tiny-entry and canon changes:
  **not applicable**; none is introduced by this change.

## Executed validation

On the stable final code, the command below completed successfully:

```sh
PYTHONDONTWRITEBYTECODE=1 python -m pytest -q -x -p no:cacheprovider \
  mechanics/growth-cycle/tests/test_source_claim_commands.py \
  -k 'correction or revision or claim_display'
```

Result: **17 passed, 23 deselected, 54 subtests passed**, 420.84 seconds reported
by pytest. Host-admitted unit `tos-root-claim-history-diagnostic-20260910.service`
terminated with exit 0 on 2026-09-10 at 14:00 UTC: 422.123 seconds service time,
413.273 seconds CPU, 138.3 MiB memory peak, zero swap peak. An earlier run
returned exit 1 while the implementation was still changing; its stdout was
not retained. Its cause is not asserted or hidden by the successful rerun.

The tests include exact full context and graph form selection, unknown/invalid
extensions, language/length limits, old-grant refusal, raw-apply refusal,
current revocation, result/predecessor integrity, late independent successors,
source/form history, concurrent corrections and interrupted publication.
Synthetic claims remain explicitly synthetic.

Decision-index generation/check, decision-record validation, mechanics topology,
source-home and whitespace checks passed before this note. The earlier full
cross-corpus documentation check failed: it exposed the missing helper inventory
(now repaired), a missing historical OCR review note in this partial union,
stale derived documentation/agent/KAG companions and absent pinned KAG checkout.
These are not a green full gate. Integration owns restoring the original OCR
note from its existing commit and rebuilding companions from the final union.

Legacy/compound compatibility and discovery subsequently passed:

```sh
PYTHONDONTWRITEBYTECODE=1 python -m pytest -q -x -p no:cacheprovider \
  mechanics/growth-cycle/tests/test_source_claim_commands.py \
  mechanics/growth-cycle/tests/test_source_commands.py \
  mechanics/growth-cycle/tests/test_source_command_discovery.py \
  -k 'claim_forms_use_shared or claim_form_refusals or complete_creation_reaches or invalid_sources_claims_forms or discovery'
```

Result: **10 passed, 77 deselected, 124 subtests passed**, 59.80 seconds reported
by pytest. Unit `tos-root-form-compat-20260910.service` exited 0 at 14:06 UTC:
61.780 seconds service time, 59.426 seconds CPU, 146.1 MiB memory peak, zero swap.
Its first launch was refused before execution by storage preflight. The other
task's owner corrected its own future-write accounting; the retry was admitted
normally. No permission gate was bypassed or other task's files removed.

The full private-adapter module is **not yet verified**. Its initial unittest
process exited 120 at 14:05:22 UTC, after 1088.645 seconds service time and
1079.689 seconds CPU, with 85.6 MiB memory and 4.1 MiB swap peaks. The launcher
returned 143 without preserving traceback. The child failure is authoritative;
default success fields after the transient unit was collected are not evidence
of success. A diagnostic rerun with captured output is required; the cause is
not attributed to the code or the launcher without that evidence.

A subsequent captured run of the same relative test path returned **22 passed,
25 subtests passed** in 1123.92 seconds, but its launch directory was the
canonical checkout, **not this changed worktree**. This is an execution-location
error, not verification of the change, and that green result is excluded here.
The original failed launch did use the changed worktree. The corrected full
run, `tos-native-structure-private-integrity-20260910.service`, had both its live
process cwd and systemd `WorkingDirectory` independently checked against the
native-structure worktree. It also ended without captured stdout: its launcher
returned 143 and the journal recorded pytest exit 1 after 1199.060 seconds,
1184.954 seconds CPU, 127.7 MiB memory and 6.4 MiB swap peaks. All five captured
source/test hashes were unchanged. An external termination or pipe problem is
a hypothesis, not an established cause; no assertion traceback was recovered.
The same 26 collected tests are now being run in disjoint short groups with
pytest XML reports. The first three pure integrity tests passed on this exact
worktree at 14:56 UTC (3 passed in 0.16 seconds; XML: 3 tests, no errors,
failures or skips). They cover private grammar isolation, exact historical
reconstruction without current write authority, and corrupted result,
predecessor and retention refusal. Source/test hashes remained unchanged.
The other 23 tests and the combined result remain pending. The local source
commit preserves this frozen candidate; it does not close this private gate.

## Remaining boundary and next owner

Actual source wording for letter 705, scoped creation/correction/form operations,
compact delivery budgets, Python/Worker parity and real UI consumption remain
separate work. Complete Claim JSON must remain an inspection payload, not an
unbounded compact carrier. No production deployment, final CI, merge, whole
corpus migration or Foundation v1 completion is claimed here.
