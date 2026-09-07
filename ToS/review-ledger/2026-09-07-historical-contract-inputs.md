# Historical contract inputs — 2026-09-07

## Scope and owner review

Reviewed the source-foundation validator, its regression controls, the exact
retained Corpus schema, Corpus doctrine and TOS-D-0052 against parent
`eb2ec3b640c14bcf4204415db7bc81b1f9f559db` in the Foundation v1 task.
Current schemas own current record validation; historical bytes own only the
recorded input reference. This resolves the 19 schema-input diagnostics noted
in the preceding native-identity review, not that review's access/UI gaps.

The retained 5,748 bytes have SHA-256
`2f319b7bb1fe146d42422685e5d3c727aa2cde539919c18ff6d6ac4f9b1a6019`
and equal `corpus-record.schema.json` at
`afc87a39cd2398738c56f72dcb21509661dd2832`. The active schema and original
source/provenance records were not changed by this repair.

Checklist: source traceability, current/historical distinction, immutable
identity, explicit limits and ToS owner boundaries **yes**. New interpretation,
rights, consent, translation, counterpart, canon and publication decisions
**not applicable**. No assertion of execution authenticity follows from byte
availability. The validator neither obtains Git/network data nor falls back
for ordinary evidence; absent active schemas remain failures.

## Verification

- The focused regression first failed because the resolver was absent, then
  passed for current and retained inputs. Negative controls cover corrupted
  bytes, wrong original schema ID, source/non-schema substitution, missing
  active files, symlinks, malformed JSON, path escapes and the 1 MiB bound.
- `python -m unittest discover -s tests -p test_source_witness_foundation.py`:
  98 tests, 37.708 seconds, passed with one skip. That check requires private
  local payloads and skips when those bytes are unavailable; it does not waive
  other validation failures or imply those payloads were inspected.
- `python scripts/validate_source_witness_foundation.py`: passed after the
  doctrine change; optional local payload mode checks bytes that are present.
- Decision-record validation and generated index currentness passed.

These are local mechanical checks, not source truth, CI, merge, runtime,
semantic assessment or Foundation v1 acceptance. The next access-owner work
must still resolve multiple graph carriers for one identity and keep default
semantic focus from expanding through a shared technical maker hub.

## Recovery

Reverting the reader restores the earlier historical-input mismatch; it does
not require rewriting an event or rolling back the active language contract.
Retained source bytes are not disposable runtime cache. General source-record
history, source assessment and deployment keep their existing owner routes.
