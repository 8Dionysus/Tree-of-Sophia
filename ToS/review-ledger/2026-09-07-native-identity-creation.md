# Native identity creation and retained creation receipts

Date: 2026-09-07 UTC. Reviewer: `model:codex`, source/Growth implementation
and boundary inspection only. This is a checkpoint, not substantive identity
assessment, admission, publication clearance or Foundation v1 completion.

## Owner changes

`mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py`
now supports separately delegated `source.create` for the existing native
Agent, Place and Organization records. It reuses the Corpus schema, metadata
field catalog, source-copy forms, global source lock, no-replace transaction
and six-file serialization capture. No role subclasses, new Person schema,
claim inference or admission authority were added.

Creation rejects pre-reviewed identities/labels/identifiers, link-field
attributions, source/path/type substitution and unsupported schema fields.
Permitted language qualifications retain unknown values. Metadata and declared
Claim forms now both participate in allocated form-ID collision checks and
the prepared dependency digest. A regression reproduced the missed Claim-form
collision before the shared scan was corrected.

Creation retry formerly accepted changed receipt fields and changed initial
file bytes. The regression reproduced 22 missing refusals across uncaptured
historical and captured native packages. The shared retry now verifies receipt
shape, principal/authority, original source/request/configuration/dependency
bindings, non-admission, file closure and byte digests. Valid later form versions
retain the original source-copy forms. Historical source corrections reuse the
existing archive reader and validate the complete retained source lineage;
they do not restore old bytes over the current source. This remains unsigned
same-account storage verification, not authenticated execution provenance.

The catalog entry schema now accepts the declared `source-claims.jsonl`
carrier and requires its `source_schema_ref`. The existing exact catalog
regression now enumerates that carrier and validates every projected entry;
unknown carrier names and missing declared schema routes are negative cases.

## Real source use

The [Naumann Agent](../source-witnesses/agents/constantin-georg-naumann/agent.json)
was created through `source.create`, with two exact source-copy forms and a
captured request/environment/provenance/receipt. Its canonical record digest
is `c601f723c3f82419d87ef365abb2979726b178e339cd3a9c83630f0b203f207a`.
It is provisional, not the existing Naumann company record. No birth/death
dates, external authority ID or reviewed equivalence were added.

The [addressee Claim](../source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl)
was separately created through `claims.create`, followed by its separately
delegated statement form. Claim v1 digest:
`166d57a5decfa28a68f6a2a0b3213cf6454648a52dc567e3872a59de0024063f`.
The form is a full Russian statement with the entire Claim as context,
`ready` mechanically, `standalone_reading: false`, admission null.

Both creations replayed exactly; Claim replay also succeeded after the form
write. The previous letter and its three initial Claims were not rewritten.
The retained [source-reading note](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md)
is the bounded research basis, not a newly captured independent witness.
Addressee identification does not establish delivery, reading, agreement,
person/company equivalence or historical influence. No image payload was
acquired or republished, and no substantive assessment was submitted.

## Verification and limits

- `python -m unittest discover -s mechanics/growth-cycle/tests`: 127 passed
  (56.958 s in the final broad run before the catalog-only correction).
- `python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py`:
  58 passed (59.417 s); source/graph/reader mechanics, not source truth.
- The focused exact-source Claim catalog test passed (0.106 s), covering all
  205 catalog Claims after adding the declared-stream schema route.
- Validation-lane, script-topology and test-topology modules passed 10, 10
  and 6 tests respectively. Nested cards and the 16-route harness passed
  after rebuilding their stale currentness companion from source.
- Catalog, bibliographic graph and corpus index were rebuilt through their
  builders. Catalog parity, graph validator, corpus-index validator and
  source-home validator passed.
- The bibliographic graph grew from 721 to 724 nodes and 1,344 to 1,349 edges.
  Every previous edge and all 199 previous Claim traces are unchanged.
  One existing maker node legitimately gained a source reference; the other
  previous nodes are unchanged. Counts alone do not prove acceptance.

The full source-foundation validator is **not green**. After the catalog fix,
19 responsibility-Claim diagnostics remain for historical inputs of
`ToS/contracts/corpus-record.schema.json`. Its current bytes are unchanged
from this checkpoint's parent. The preserved input digest
`2f319b7bb1fe146d42422685e5d3c727aa2cde539919c18ff6d6ac4f9b1a6019`
matches the exact schema bytes at `afc87a39c`; the current schema digest is
`baccf59bc5c6ffd1560848610ac812751d1e5182c89658814eeb2eedde4b6db1`.
The next source/provenance-owner change must make historical input-byte
resolution explicit without restamping old events or weakening current
record validation. This note does not waive that gate.

Actual `ToSAccessCore` loading and form selection were exercised. Restricting
focus to `source-claims` returns both endpoints from the person and letter
centers (6 and 14 nodes at depth 2). The unrestricted reader still exposes
distinct source-navigation and source-claims carriers for the same entity ID:
unrestricted person/letter focus may omit the other endpoint, while Claim
focus can expand through shared maker provenance to its 200-node limit.
These are unresolved access-owner issues, not successful end-user acceptance.
One local full graph load measured 15.039 s; it is neither an indexed-query
latency result nor a scaling budget proof. UI gestures and Worker/D1 parity
for these new source records were not exercised in this checkpoint.

## Manual checklist and next ownership

Yes: source traceability, authored/derived separation, person/organization/role
separation, stable IDs, exact old source retention, visible uncertainty,
language-neutral subject identity, no ungranted assessment and no neighboring
runtime authority. Not applicable: canon promotion, counterpart/compost,
lived-witness handling, translation-quality admission, tiny-entry changes and
golden-kernel acceptance. There is no claim of a human signature or of model
competence established by the tests.

Recovery preserves source packages and receipts; correcting descriptions or
removing derived delivery does not erase their history. Code and generated
reader rollback are separate from source retirement. No old source, payload,
archive or shared runtime was deleted. Host capacity was reserved before the
derived rebuilds; this is storage accounting, not artifact/publication admission.

Native general record revision, multi-subject bibliographic/artifact creation,
the physical letter carrier, values/time/roles and thought profiles,
substantive nonself assessment, migration/scaling and integrated UI/CI remain
in the full operator goal. Immediate follow-through belongs to source-owner
historical-input resolution and access-owner cross-projection focus. No PR,
merge, deployment or published runtime claim is made by this checkpoint.
