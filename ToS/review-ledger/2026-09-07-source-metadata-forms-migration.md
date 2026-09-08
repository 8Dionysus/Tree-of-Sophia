# Source metadata forms: bounded migration

Date: 2026-09-07 UTC. Base: `9f9e177c14ed1fbd2b1d1d72e4814f21c0198082`.
This is migration and consumer evidence, not substantive assessment, complete
Foundation v1 acceptance, a new corpus registry, or publication authority.
Exact source baselines and per-file outputs are in the adjacent
[evidence record](2026-09-07-source-metadata-forms-migration.json).

## Source, operation and result

The operator's Foundation v1 commission authorizes the source-copy migration.
Host-owner accounting routes were separately applied and read back before the
source and graph reservations were acquired. No deployment, restart or cleanup
was performed. Each reservation was released after its operation terminated.

The existing `source_commands.py` entrypoint created 170 adjacent
`*.human-forms.json` sets. The preexisting Jenseits set is byte-identical. Each
new set contains exact source-copy bindings and one authenticated local-account
command receipt; it records no human signature, semantic judgment or admission.
Delegation was create-only and subject/identity/version scoped. Issued form
identities are retained in the source sets; they must not be regenerated after
source-field reordering or reinterpreted as hashes of immutable wording.

All 171 supported bibliographic records now supply 460 forms: 289 names and
171 hover notes. The covered kinds are 41 agents, 4 places, 11 organizations,
40 works, 28 expressions, 23 editions, 3 collections and 21 items. Whole source
strings, source ID/version/digest, identity posture and variant metadata remain
intact. No source metadata was inferred: 342 forms still have unspecified
language and 21 retain source `und`; 51 are `en`, 22 `ru` and 24 `de`.

Five object-link records are explicitly excluded from this corpus-record
adapter; they retain their separate object-link contract and reader. Other
ToS families are not claimed to have migrated. Empty, unsupported and unassessed
descriptive roles are not filled by this operation.

## Execution and comparison

A three-record pilot created nine forms. Exact replay changed no bytes. The
remaining run created 167 sets and replayed the pilot. A new invocation then
replayed all 170 commands, preserving all 171 output files and adding no second
receipt. Final sets occupy 1,149,258 bytes, below the 8 MiB source reservation.
The baseline comparison covers 182 unchanged files: source records, catalogs,
the catalog manifest and the original form set. It is not a claim to have
rehash-read all private payloads.

The first graph rebuild exposed a consumer gap: ten catalogued subjects with
no Claim were absent. The builder now includes every supported public-metadata
catalog identity, whether or not a Claim mentions it. The ten added graph
identities were already authored source objects, not newly invented subjects.
No edge or assertion was inferred from their names or notes. The final graph
has 690 nodes and 1,312 edges, including 171 identities and 193 unchanged
unreviewed Claim records. Existing nodes differ only in form-bearing fields;
Claim bodies, traces, edges and review/visibility counts remain unchanged.
The graph occupies 5,326,726 bytes, below the 24 MiB graph reservation.

The ordinary `ToSAccessCore` reader inspected all 171 subjects and returned all
460 ready source-copy forms without semantic admission. Focus and compact
LensSpec were exercised for Laws of Hammurapi, Jenseits and Agnete Wisti Lassen.
The first was previously absent. Compact packets retained full hover wording,
mandatory context and source-return information with cleared technical
attributes; the three selected node packets were 7,425, 7,186 and 9,793 bytes.

One local run measured 27.364 seconds for cold full normalization, 7.296 seconds
for all 171 inspections, 219–231 ms for individual focus and 27 ms for each
compact lens. These are observations during local validation, not controlled
benchmarks or accepted budgets. In particular the cold full build remains a
performance gap for the broader goal; source-copy migration does not solve it.

## Reproduce and recover

The source command protocol and configuration shape are documented in
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md`. Use `describe`,
`prepare` and `apply` with an independently selected current owner configuration.
For a new run, allocate only genuinely new form identities; for correction,
use `form.revise`, the current expected revision and retained predecessors.
Never recreate an existing subject's forms to disguise a correction or wipe
its receipts. The task-local frozen launch inputs are not a second authority.

```bash
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python scripts/build_source_witness_catalog.py --check
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python -m unittest discover -s access/tests -p test_knowledge_contract.py
PYTHONPATH=access/src python -m tos_access --root . knowledge node tos.work.akkadian-law.laws-of-hammurapi --relation-limit 0
```

Each source transaction is atomic for one subject, not the entire corpus.
After interruption, retry the identical request with its command ID and current
authority; expected-version conflicts require rediscovery, not overwrite.
Unchanged source inputs plus the retained receipt make replay distinguishable
from a second write. Local coordination lock files remain in place and are
excluded only by local Git info rules, not by a corpus-content ignore.

A derived reader may be switched to the base projection with its matching
reader contract. That rollback omits the new forms and the ten newly visible
subjects; it must leave all new source sets, source identities and receipts
intact. Rebuild from those sources to return to the current reader. No source
rollback, history deletion or runtime reader switch was performed here.

## Review and limits

Source return, source/derived separation, retained identity, language/context
preservation and non-inference of fact edges were reviewed. Assessment,
rights, personal consent and canon authority were not delegated by a rendering
success. Unspecified languages remain unspecified; source prose was not
substantively reevaluated. There is no new ontology or source schema.

Focused source-command, growth and access checks passed. The full
`source_witness_foundation` lane reached and passed the evidence-spine checks,
then failed at the existing private `local-content/transfer-source-passages`
file prohibition. Its contents were not read, moved or removed. This is a
separate source/storage-owner disposition, not a reason to claim the full lane
green. CI, merge, Worker/D1 materialization, UI interaction and smoothness,
production deployment, all-domain coverage and agent assessment quality are
not established by this record. Those requirements remain open in
[the foundation map](../doctrine/FOUNDATION_V1.md).
