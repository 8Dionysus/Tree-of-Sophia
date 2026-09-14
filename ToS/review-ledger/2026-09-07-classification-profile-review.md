# Classification facets: source, command and reader review

Date: 2026-09-07 local / 2026-09-08 UTC. Reviewer: Codex (agent/model), within
the foundation-v1 assignment. This is bounded structural and source-return
review, not calibrated historical assessment or admission.

## Changed owner surfaces

Entity registry 21, relation registry 20 and the classification Claim schema
add four independent facets: genre, content form, communication medium and
carrier medium. They reuse the shared structured-value reader; no Python
predicate dispatch, new backend or profile-specific UI was added. The abstract
value family shares term/language/script, basis and scope filter definitions.
The existing File receives a declared MIME property from source manifests.

Source type, classification, atlas navigation category, contextual role and
intellectual membership remain distinct. Genre/form overlap is permissible;
neither changes subject identity. Carrier classification applies to Artifact
or Item, not Work or File. A declared MIME type is technical metadata, not a
payload inspection result, genre or access permission. Unknown extensions are
retained without extracting hidden identity, time or executable instructions.

Four actual Claims and four qualified Russian statement forms were created
through separately delegated v3 source-owner commands under
[`relations/independent-classifications/`](../source-witnesses/relations/independent-classifications/).
The first create plus exact retry took 9.518 s and returned the same receipt;
all forms were ready and admission remained false. No existing source subject
or earlier decision was rewritten. The frozen
[source-reading note](2026-09-07-classification-source-reading.md) separates
fresh Jastrow/Clay and CDLI reading from the letter's recorded earlier reading.
The unsuccessful present eKGWB fetch is not represented as source inspection.

## Observed verification

The classification contract test checks all four facets, subject/range
rejection, mandatory qualified values, invalid language and blank-term
rejection, source preservation, inert extension fields, independent equal
values, negation context, source-language display, inherited property filters
and reverse focus. Atlas genre/medium entries retain navigation status.
The File-property test excludes similarly named Work metadata and preserves
unknown file format. The shared structured-value command test exercised v3
authority, exact retries, correction, form materialization and retained prior
bytes on its synthetic survival fixture; it does not assert a real survival
or classification judgment.

Observed completed runs: 79 bibliographic graph tests in 249.284 s, including
the new facet and term-language/script controls; 62 access knowledge-contract
tests in 31.317 s; the focused structured-value command test in 13.551 s.
Source-foundation, bibliographic-graph and corpus-index validators passed.
Source catalog/graph parity and documentation currentness were checked after
regeneration. A later exact creation retry after the additive registry update
again preserved the four Claims, forms and original receipt (1.398 s).

A separate bounded read-only helper review found no actionable boundary or
source-overclaim defects. It inspected the source schemas, registries, reader
law, four Claims and cited source-reading context, but ran no tests and did not
verify fresh eKGWB/manuscript content, competence, admission or runtime. This
peer review does not create independent historical evidence or proof authority.

An actual common-reader run returned all four exact values and enclosing
Claims, their unreviewed posture and complete qualified RU statements. Eight
forward/reverse focus calls preserved one focus vertex each, with all carrier
IDs accounted for. A value's scene vertex correctly has null persistent
`entity_id`; its carrier is addressable without inventing a new persistent
subject. An initial probe incorrectly expected a persistent ID on every
vertex; only that assertion was corrected, not the product.

Measured snapshot before the final language-filter/documentation updates:
`54f9fcdbfb3b08653712f8a2e73819f96eb0df6b5c74919635f3541aec1a6ace`.
Cold common-graph load: 44.473 s; eight focus calls: 0.580, 1.960, 0.574,
0.394, 0.456, 0.466, 0.675 and 0.534 s. Returned packets contained 4–17 nodes
and 3–23 relations. MIME filtering selected 15 `application/pdf` File nodes,
not Works. Peak process RSS: 1,284,056 KiB. Concurrent checks were running;
these observations are not isolated performance budgets, scaling proof or UI
smoothness acceptance.

For a fresh read-only reproduction, first refresh the source catalog,
bibliographic graph and corpus index through their owned builders, then run
from the repository root:

```bash
PYTHONPATH=access/src python - <<'PY'
import json
from pathlib import Path
from tos_access.core import ToSAccessCore
from tos_access.knowledge import execute_knowledge_lens, select_human_forms
root = Path.cwd()
core = ToSAccessCore.discover(root)
graph = core.knowledge_graph()
path = root / 'ToS/source-witnesses/relations/independent-classifications/source-claims.jsonl'
for claim in map(json.loads, path.read_text().splitlines()):
    node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
    assert node['attributes']['source_claim'] == claim
    form = select_human_forms(node, 'ru')['roles']['statement']['packet']
    assert form['display_text'] == claim['qualifiers']['statement']
    assert form['admission'] is None
    literal = next(n for n in graph['nodes'] if n['type_id'] == 'tos.entity.' + claim['object']['kind']
                   and n['attributes'].get('value') == claim['object'])
    spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'classification',
        'node_query': {'filters': [{'property_id': 'tos.property.classification-term',
                                   'op': 'eq', 'value': claim['object']['term']}]},
        'relation_query': {'enabled': False}, 'detail': 'full'}
    assert literal['id'] in {n['id'] for n in execute_knowledge_lens(graph, spec)['nodes']}
    for focus, target in ((claim['subject_ref'], literal['entity_id']),
                          (literal['entity_id'], claim['subject_ref'])):
        packet = core.knowledge_focus(focus, depth=2, node_limit=200, relation_limit=400)
        assert {target, claim['claim_id']} <= {n['entity_id'] for n in packet['nodes']}
        vertex = next(v for v in packet['scene']['vertices'] if v['id'] == packet['scene']['focus_vertex_id'])
        assert vertex['node_ids']
print('four classifications and bidirectional focus verified; no admission')
PY
```

## Review disposition and remaining scope

Applicable checklist judgments: source traceability, layer separation,
identity stability, qualification retention, multilingual source authority,
bounded growth and no stronger-owner authority transfer pass this structural
review. Canon, lived witness, legal clearance and publication changes are not
applicable. No newly observed fact justifies a separate registry of candidates
or promotion to proof/memory authority. The named source-assessment route owns
the next meaningful content judgment; these preliminary Claims are not an
unowned queue or a mandatory human-signature step.

No fresh UI interaction, Worker/D1 packet, independent historical competence,
full language-form coverage, CI, merge, release or deployment is certified by
this change. B06 has executable connected coverage, not whole-foundation
completion. The existing UI's composition and movement were untouched.

Rollback is a source-aware successor/revert and normal derived rebuild, not
deletion of new Claims, their exact creation inputs or any later assessments.
No existing item payload or original source bytes were modified.
