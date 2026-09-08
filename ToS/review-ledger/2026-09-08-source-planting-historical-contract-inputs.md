# Source-planting historical contract inputs — 2026-09-08

## Scope and owner review

Reviewed the source-planting schema input boundary in the registry normalization
and first-planting task against parent
`29c26280992f1397dd3a6ff12cd42f384727a5bc`. The active schema now permits the
existing tier-2 and tier-3 atlas table routes; recorded earlier provenance inputs
must continue to resolve their original contract bytes.

Under [TOS-D-0052](../../docs/decisions/TOS-D-0052-historical-contract-input-bytes.md),
retained the exact 6,068 bytes of
`ToS/contracts/philosophy-source-planting.schema.json` from that parent in
`ToS/contracts/history/3d37a53816e779c7bc65a15bf51b1520bb7c2ef389b1d4ea54473f487906df28.json`.
The filename is their SHA-256, and their `$id` identifies the original active
schema path. Thirty existing provenance events reference this digest. No event
was restamped and no source, responsibility claim, relation or planting record
was changed by this repair.

Source traceability, immutable historical bytes, current/historical distinction,
explicit evidence limits and ToS owner boundaries: **yes**. New interpretation,
translation, rights, consent, canon and publication decisions: **not applicable**.
A retained input provides byte availability; it does not authenticate the
original execution or admit a source or philosophical claim. Current planting
records retain active-schema and actual atlas-membership checks.

## Verification

- Exact retained bytes equal `git show` for the named parent and original path;
  SHA-256 and original schema identity match.
- Existing focused resolver regression passed: current and retained inputs,
  corrupted bytes, wrong schema identity, ordinary input substitution, missing
  active schema, symlinks, malformed JSON, path escapes and the size bound.
- `python scripts/validate_source_witness_foundation.py` passed in optional local
  payload mode; present bytes were fixity-checked.

These are local mechanical checks. CI, merge, publication, semantic assessment
and first-planting acceptance remain separate. The next owner is the current
ToS first-planting task, which retains responsibility for source acquisition,
rights, prepared branch targets and reviewed structural admission.
