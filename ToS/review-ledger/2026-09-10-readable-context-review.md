# Readable governing context review

## Scope and result

Reviewed the bounded source-owned presentation vocabulary and its optional
read-only access companion against the exact pre-change baseline
`641aee49f8f7a015464c17b47c2a4e1971aa5ee8`.

The entity registry's `context_presentation` and `HUMAN_FORMS.md` own labels,
explanations and classification. The graph schema owns the sidecar body;
`readable-context.v1.schema.json` is a thin reference to that definition.
The access compiler, catalog, contract bundle, normalization dependency digest,
and Python/Worker delivery carry this source contract. No source Claim,
HumanForm wording, translation, assessment, admission or permission was added.
The vocabulary is finite: unknown fields, source schemas and enum values keep
their raw values and explicit unclassified state.

Manual review followed the source/boundary checklist. Source traceability,
authored-versus-derived distinction, preservation of qualifications and
disagreement, language authority, and ToS/access ownership: **yes**. Canon,
rights, public publication, lived testimony and identity transitions: **not
applicable**; this change grants none of them. Labels for recorded acceptance,
visibility and identity posture explicitly describe source declarations,
not current grants or the truth of a proposition.

## Findings resolved

- Exact pointer/value checks use canonical JSON comparison, preserving false
  versus zero and true versus one, including nested values.
- Compact lens delivery omits the optional sidecar because its raw roots are
  absent. Full delivery preserves both. The selected HumanForm retains its own
  mandatory context in either mode; an absent sidecar is not absent context.
- Canonical JSON record hashes distinguish numeric representations that normal
  JavaScript JSON numbers collapse. Deduplicated `exact_materials` retains the
  existing canonical text and genuine raw origins within the same 32 KiB limit.
  Consumers must hash that text and read numeric values losslessly from it.
  This preserves the existing canonical representation, not literal source-file
  whitespace or exponent spelling. Original source bytes remain source-owned.
- Record bindings resolve exact material by record digest plus source pointer;
  assertion and supplementary materialization bindings resolve through their
  actual carrier origins. Source and graph digests were not redefined.
- Carriers without context bypass copying, restamping and the additional cache
  stage. Contextual carriers share their untouched nested inputs. A changed
  presentation vocabulary invalidates the derived stage across cache runs.
- Overflow or malformed bindings return no partial ready context or partial
  exact material. Both arrays are empty, with exact source roots and a reason.

The master review and a separate bounded source-review helper independently
checked the exact-material binding and numeric boundary. The latter also
probed all three binding kinds, forged origins and the 1024-byte refusal limit.
Those checks found no remaining blocker in this bounded contract. They do not
establish UI rendering or whole-foundation acceptance.

## Verification

The following focused checks passed on the changed tree:

- `PYTHONPATH=access/tests python -m unittest test_readable_context test_processing
  test_knowledge_contract.KnowledgeContractTests.test_completed_normalization_steps_are_reused_and_dependencies_invalidate
  test_knowledge_contract.KnowledgeContractTests.test_relation_view_membership_participates_in_endpoint_content_revision
  test_knowledge_contract.KnowledgeContractTests.test_source_form_selection_keeps_exact_wording_context_and_language_fallback`:
  31 tests, 4.856 seconds; observed transient service peak 38.4 MiB, zero swap.
- The existing Worker knowledge battery: 15 tests passed. The new readable
  context test separately verifies RU/EN full/compact Python/Worker/local-D1
  parity, direct inspection, exact catalog vocabulary, and synthetic numeric
  controls (`1`, `1.0`, unsafe integer, negative zero and exponent notation).
  Ordinary HTTP JSON normalization is distinguished from preserved exact text.
- Worker `npm run typecheck`; public contract-bundle resolution of the thin
  schema against a current complete sidecar; `git diff --check`.
- `TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT=641aee49f8f7a015464c17b47c2a4e1971aa5ee8
  python scripts/validate_semantic_registry_transition.py` and
  `python scripts/validate_tos_source_home.py`.

The real Freedom fixture uses its existing public metadata and all three current
source-owned materializations. Its full-build catalog revision and sidecar
vocabulary agree; original HumanForms remain unchanged. Synthetic numeric and
conflict variants are test controls, not new historical assertions.

## Bounded cost observation and remaining owners

A deterministic slice of the existing public
`ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
(SHA-256 `fb5840f9d535d469fc741c3df7a85525012887bc90b7dfd7eb6e1134153d3b40`)
selected the first 32 form-bearing carriers, 32 further Claim carriers, and
192 other carriers in source order. Of those 256 carriers, 181 needed no sidecar,
74 returned complete context, and one explicitly exceeded the shared limit.
Sidecars occupied 1,835,792 canonical JSON bytes; the largest was 32,586 bytes.
The carrier array grew from 1,942,506 to 3,779,798 bytes. Three in-memory attachment
passes took 0.559, 0.550 and 0.558 seconds (median 0.558); the observed service peak
was 66.9 MiB with zero swap. This measures attachment on that slice, not cold
corpus assembly, a persistent cache footprint or production latency acceptance.

Integration owns the addressed-update hook and combined normalization binding,
full-corpus validation and projection regeneration after merging parallel owner
changes. The UI owner owns lossless rendering and real interaction acceptance.
Full standalone/release/KAG regeneration, CI, merge, deployment and live hosted
behavior were not established by this bounded change. Reverting the derived
reader remains possible while retaining all original source records and forms.
