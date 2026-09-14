# Reception profiles: contract and local-reader review

Date: 2026-09-07. Reviewer: Codex (agent/model), within the foundation-v1
implementation assignment. This review concerns source/reader boundaries and
observed behavior, not calibrated historical or linguistic acceptance.

## Owner change and judgment

Entity registry 20 and relation registry 19 declare reception, historical
canon formation, forgetting, rediscovery and legacy through the existing
source-profile extension contract. The new reception schema requires receiving
context and kind-specific evidential qualifications. All five are historical
referents; historical canonization is not a ToS assessment event. Targets and
carriers remain reified scholarly-report Claims, not direct truth edges.

The source-reading account and its limitations are frozen in
[`2026-09-07-reception-source-reading.md`](2026-09-07-reception-source-reading.md).
Two actual source-described histories now connect the existing Pennsylvania
tablet, provisional Old Babylonian transmission cluster and 1920 scholarly
Work. Three Claims share one source origin. No modern site, date, textual
reconstruction, independent corroboration, quality approval or admission is
inferred. The other three profiles have synthetic contract coverage, not
fabricated historical instances.

The applicable review-checklist items pass this bounded structural review:
source return; source/interpretation/projection separation; historical and
receiving context; stable identities; multilingual forms without language-split
objects; preserved uncertainty; no AoA runtime authority imported into ToS;
no fabricated human review or self-issued competence. Canon, lived witness,
rights clearance and tiny-entry changes are not applicable. Broader plurality,
historical assessment quality and v1 completeness are not certified by this
review.

## Executed local behavior

The existing source-owner commands created two native records with RU/EN names
and RU notes, three Claims and their complete RU statement forms. Exact retries
returned the original creation receipts. The tracked request, serialization
provenance, environment and receipt files live beside the new sources under
`source-witnesses/history/gilgamesh-reception/` and
`source-witnesses/relations/gilgamesh-reception/`. They prove the recorded
serialization/transaction, not upstream model execution or semantic quality.

Content review also narrowed one short statement: its 1914 qualifier could
previously date both purchase and identification. `claim.revise` attached the
explicit year to the reported purchase without independently dating Poebel's
identification. The selected Claim retains its ID and advances to version 2
(`sha256:0c836c9f9b886acea58094af87444dc0324d92c2d9726a1c8cefd0676d50c98c`).
The corrected statement form is rebound; eleven predecessor package files are
retained and digest-verified, and the other two Claim rows remain byte-identical.
An exact correction retry returned the same receipt. The operation and checks
took 10.911 s; neither grants assessment or admission. The original source-reading
note and creation inputs remain unchanged.

A real local reader probe inspected both graph carriers, exact source fields,
language selection, full semantic-content and Claim context, inherited
property filters and six forward/reverse focus calls. Each selected focus has
one scene vertex retaining its carrier IDs. Artifact, ancient cluster and
modern book remain three identities. New forms retain null admission; Claims
retain unreviewed posture. Date words do not become normalized time keys.

Observed common-reader snapshot:
`228c19cb7f69e473765270820b2b121ab91c513cbc9803907fadd79228a60004`.
It predates the wording correction, this review note and later documentation regeneration. Cold graph
load was 26.563 s; six focus calls took 0.419, 0.419, 1.898, 0.446, 0.463 and
0.463 s, returning 5–23 nodes and 4–38 relations. Peak process RSS was
1,282,524 KiB. This is one local run with concurrent validation, not an isolated
performance benchmark, latency acceptance or growth-scale proof.

For a fresh, read-only check of the actual packet, run from the repository root
(the two derived graph carriers must first match current sources):

```bash
PYTHONPATH=access/src python - <<'PY'
import json
from pathlib import Path
from tos_access.core import ToSAccessCore
from tos_access.knowledge import select_human_forms
root = Path.cwd()
core = ToSAccessCore.discover(root)
graph = core.knowledge_graph()
source = root / 'ToS/source-witnesses/relations/gilgamesh-reception/source-claims.jsonl'
for claim in map(json.loads, source.read_text().splitlines()):
    node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
    assert node['attributes']['source_claim'] == claim
    packet = select_human_forms(node, 'ru')['roles']['statement']['packet']
    assert packet['display_text'] == claim['qualifiers']['statement']
    assert packet['admission'] is None
    for focus, target in ((claim['subject_ref'], claim['object']),
                          (claim['object'], claim['subject_ref'])):
        result = core.knowledge_focus(focus, depth=2, node_limit=200, relation_limit=400)
        assert {target, claim['claim_id']} <= {n['entity_id'] for n in result['nodes']}
        assert len([v for v in result['scene']['vertices'] if v['entity_id'] == focus]) == 1
        print(focus, len(result['nodes']), len(result['relations']))
PY
```

Reproduce the public checks with:

```bash
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python -m unittest discover -s access/tests -p test_knowledge_contract.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python scripts/build_source_witness_catalog.py --check
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
python scripts/build_tos_corpus_index.py --check
python scripts/validate_tos_corpus_index.py
```

The whole source-command module is listed for reproduction; this pass ran its
focused `HistoricalCreationTests.test_semantic_description_creation_and_correction_preserve_referent_and_scope`
method, covering all five new profiles alongside existing kinds (187.013 s,
pass). Access knowledge-contract tests: 61 passed in 37.990 s. The full graph
module passed 78 tests in 252.030 s before the data-only wording correction.
Its first run had correctly rejected a stale graph when regeneration and the
first test overlapped; the successful rerun used a stable snapshot, with no
weakened check. Source-foundation, bibliographic-graph and corpus-index validators
passed. Documentation currentness and mechanics topology also passed after
their source-owned regeneration.

After the data-only correction, the reception contract and verified-projection
query tests both passed (11.180 s). The real reader probe passed again at
`e1857c6d1fd6a246af9c38c4bf086ea4b648508d6916b344e54456590d9abe38`:
cold load 28.725 s, focus 0.381–1.727 s, peak RSS 1,282,640 KiB. Original creation
replay preserved the corrected version, and the source, graph, corpus and
documentation validators passed again. These observations still precede this
final note update; they are not performance or historical acceptance.

## Rollback, remaining work and next owner

An old closed-schema reader cannot consume these new profiles. Rebuild the
catalog and graph companions from source with the declared schema versions;
do not hand-edit generated nodes. Restore a prior compatible reader snapshot
for rollback without erasing new source identities, requests, provenance or
decision history. No external artifact was admitted, published or deployed.

R03 remains partial: all-five real-material coverage, a dedicated reception
timeline, calibrated content and wording assessment, actual UI interaction,
D1 consumption of this particular packet, scale acceptance and CI/landing are
not proved here. Qualified source-assessment binding remains the next owner
step for these preliminary histories; there is no mandatory per-record human
signature. The overall foundation goal is not complete.
