# Branch Growth Cycle

## Operating Card

| Field | Route |
| --- | --- |
| role | distinguish deepen-node, create-node, and form-branch moves |
| input | source pressure, review state, branch need |
| output | growth route or return-to-review |
| owner | `mechanics/growth-cycle/parts/branch-growth-cycle/` |
| next route | `ToS/philosophy/` or `ToS/canon/` after review |
| tools | `mechanics/growth-cycle/parts/branch-growth-cycle/docs/GROWTH_STRUCTURE.md`, `ToS/philosophy/philosophy.manifest.json`, `mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py` |
| check | `python scripts/validate_philosophy_topology.py` |

## Assessment policy application

`scripts/knowledge_assessment.py` applies the source-owned
`ToS/doctrine/KNOWLEDGE_ASSESSMENT.md` law to authenticated owner inputs without
network, model calls or source writes. It distinguishes substantive assessment
from current admission. The caller supplies trusted policy, grants, competence,
current exact records and complete bounded subject history; submitted prose
cannot provide its own authority. This pure engine is not yet a durable command
adapter or proof of agent competence. Local invariant checks belong to
`mechanics/growth-cycle/tests/test_knowledge_assessment.py` and the existing
`mechanics_local` discovery lane.

## Source-owner journal

`scripts/assessment_journal.py` implements immutable source-owned assessment
batches and an atomic per-subject head pointer under an explicitly configured
owner directory. `ToS/contracts/knowledge-assessment-batch.schema.json` owns
their shape. The parent directory must already exist. A hash partitions storage;
it does not replace the subject's ToS ID. No corpus assertions are copied into
a second database. Original assessment rationale and refs remain in the owned
batches; derived current admission can be rebuilt.

`append(engine, context, reviews, command_id=..., expected_revision=..., now=...)`
requires authenticated bindings and an agreed source snapshot from the command
owner. It records a valid assessment even when the judgment rejects, disputes
or defers use. It rejects the entire new batch on qualification failure or a
stale expected head. Replaying the exact command returns its old receipt and
fresh current admission separately. `inspect` materializes one subject's
complete committed history; it is not a corpus-wide scan or a public endpoint.

Unix locking serializes writers with a bounded wait (five seconds by default);
`JournalBusy` means retry, not discard or restart the live writer. Immutable blobs are fsynced before atomic head
publication; interrupted unreferenced blobs are not active history and are not
silently deleted. Missing/corrupt committed data fails closed. Committed
supersession stays effective even after the old grant expires or is revoked.
Tests cover these mechanics with synthetic records, not OS power-loss hardware
proof, trusted runtime identity, semantic quality or a deployed growth API.

Materialization currently bounds one history at 1,024 assessment events and
each batch at 1 MiB; it refuses truncation. Larger histories need a source-owned
checkpoint/archive reader, not deletion of history. Orphan retention, actual
source-adapter binding, cross-object transactions and research/UI integration
remain foundation work; this journal alone does not close the Growth profile.

## Human-form materialization

`scripts/human_forms.py` renders the source-owned
`ToS/doctrine/HUMAN_FORMS.md` contract. `materialize_form` takes an immutable
form record, trusted `FormScope`, exact access-filtered source records and an
owner-admitted template set. `SourceBinding` identifies a whole JSON field, not
an executable expression. Required context comes from the source owner, not
the proposed wording. Freeform rendering requires the existing assessment
engine and authenticated reviews/history against the current form and source
snapshot; it cannot use a submitted positive status as permission.

This pure operation writes nothing and does not call a model. The result's
wording and context are a single read contract; `standalone_reading: false`
forbids consuming the string as an unqualified assertion. The current output
budget is 64 KiB with explicit refusal, not semantic truncation. Missing,
restricted, stale and assessment-required states have no emitted wording.
Rendering mechanics do not prove an adapter's authentication, a template's
semantic quality, an agent's real-language competence or UI consumption.
`mechanics/growth-cycle/tests/test_human_forms.py` protects these boundaries.
