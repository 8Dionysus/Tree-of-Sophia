# Intellectual formations and initial Work creation, 2026-09-07

## Source and executable boundary

Entity registry v14 adds IntellectualFormation and its source-described
IntellectualSchool, IntellectualTradition and IntellectualMovement profiles.
They are historical research referents, not organizations, timeless doctrine
objects or atlas navigation categories. Existing atlas identities retain their
meaning. The composed schema requires a scope, identity criterion, formation
account and a kind-specific inquiry-lineage, transmission or orientation
account, with explicit wording languages. Seven properties expose shared and
kind-specific content/scope through the existing semantic property-ID contract.

Relation registry v13 adds four non-transitive reified predicates: intellectual
association, school-in-tradition, movement-reworks-tradition and formation
articulation in an intellectual object. They enforce concrete endpoints and
required statement, language/script, basis, scope and historical-time wording.
They neither infer membership nor universal endorsement, influence or identity.
Scholarly report and semantic interpretation remain different assertion layers.
Unknown date limits remain prose here, not interval algebra. Three Claim
properties expose basis, intellectual scope and time wording.

The existing source-metadata commands and reader carry all three new profiles;
there is no per-profile executable branch or new runtime. Creation, exact
source-copy forms, correction and replay retain the common source-owner
boundaries. This continues the source-description rationale in
[TOS-D-0053](../../docs/decisions/TOS-D-0053-source-described-conceptions.md).

## Real source material

The immutable [source-reading input](2026-09-07-intellectual-formations-source-reading.md)
records the exact sections read in the SEP Stoicism and Justus Lipsius entries
and the University of Vienna's German Circle article. URLs are section
locators, not captured bytes or frozen textual anchors. Original ancient or
Latin documents and physical editions were not inspected in this slice.

Owner commands created seven provisional subjects: Zeno, Lipsius, De constantia,
the early Athenian Stoa, a Stoic transmission tradition, Lipsian Neostoicism
and logical empiricism. Seven uncertain/unreviewed Claims connect them and the
existing Vienna Circle community; the school-to-tradition classification is
explicitly a research interpretation. The existing `authored_by` predicate
connects the Work and Lipsius. Twenty-eight source-bound forms retain Russian
names/descriptions/statements and labelled English, German or Latin name
variants. These are research descriptions, not certified translations.

Records, retained command inputs, provenance and receipts are under
`ToS/source-witnesses/agents/`, `works/justus-lipsius/de-constantia/`,
`intellectual-formations/` and `relations/formation-research/`. The maker is
the actual Foundation agent under the existing operator task authority;
this work does not self-grant assessment competence or historical admission.

## Actual Work-creation gap and bounded repair

The first creation run stopped after the two Agents: native corpus creation
accepted only Agent, Place and Organization. The Work was not retyped as a
Document or hand-written around the owner command. The existing corpus
creation operation now accepts an initial provisional Work with an explicitly
empty `expression_claim_refs` list, as required by the corpus schema. This
means no supplied expression assertions, not that no realization exists.

Initial Work creation admits identity metadata only. Other outgoing link
fields, nonempty expression refs and realization/publication fields fail.
The existing Nietzsche Work source home retains its stronger authorship and
chronology closure and refuses this initial standalone operation. No Expression,
Edition, Item, Artifact, date or responsibility Claim is invented. The actual
authorship Claim was created separately by the existing claims command.

The new Work boundary test first failed on unsupported Work creation and then
passed with the bounded implementation. Existing native creation tests now
exercise four kinds. Retried real Agent creation reused retained requests and
receipts; it did not replace already-created subjects or their provenance.

## Observed verification

- Source catalog/foundation, present-byte fixity, graph and corpus-index
  generation, parity and validators passed.
- Positive/negative formation tests cover required content/scope/languages,
  inheritance, invalid endpoints, wrong profile identities, unknown nested
  fields, reified non-transitive relations, dispute and negation.
- The shared creation/correction test passed across 30 declared profiles;
  full source-command suite: 37 tests, 128.029 s.
- Full graph suite: 68 tests, 181.221 s.
- Access knowledge suite: 58 tests, 26.646 s.
- Corpus-index suite: 9 tests, 27.307 s.

Synthetic tests constrain contracts, not historical truth. No failed check was
converted into an assessment or an automatic positive review.

## Complete-reader observation

The read-only integration probe used the ordinary `ToSAccessCore` over snapshot
`d0ff1e520d5b956438bbb7e7c34333d95ea1eee5820b7df5c63788c0e3f02234`
(40,478 carrier nodes; 60,244 relations). Every new subject was exactly equal
to its source record in both source-navigation and source-claims carriers,
with one shared entity ID and the expected Russian/variant-language forms.
Scope/content and absent admission survived in both carriers. Every Claim
retained its full statement and whole-Claim context, separate from its endpoints.
Eight body-property selections and 21 Claim-property selections passed through
semantic IDs; compact results omitted full attributes. Formation ancestry did
not include Organization, SemanticObject or NavigationObject.

Depth-two focus was bounded to 100 nodes and 200 relations:

| Focus | Required reach | Nodes / relations | Seconds |
| --- | --- | --- | --- |
| Early Athenian Stoa | Zeno and Stoic tradition | 6 / 7 | 0.282 |
| Stoic tradition | Stoa and Lipsian Neostoicism | 6 / 7 | 0.253 |
| Lipsius | De constantia and Neostoicism | 6 / 7 | 0.257 |
| De constantia | Lipsius and Neostoicism | 6 / 7 | 0.271 |
| Logical empiricism | existing Vienna Circle | 4 / 4 | 1.111 |

Python 3.14.7, Linux 7.1.13-200.fc44.x86_64, x86_64: process-cold graph call
20.800 s; whole probe wall 29.258 s, user CPU 28.511 s, system CPU 0.707 s;
peak RSS 1,260,700 KiB. OS caches were warm and another graph test process was
running. This is not isolated-host, filesystem-cold, percentile, scaling, D1 or
UI evidence. Cold cost remains a Foundation problem. Indexing this note will
change the snapshot; the measurement does not describe the later final HEAD.

Reproduce an ordinary bounded read without mutation:

```bash
PYTHONPATH=access/src python - <<'PY'
from tos_access.core import ToSAccessCore
from tos_access.knowledge import select_human_forms
core = ToSAccessCore.discover('.')
for identity in ('tos.intellectual-school.early-athenian-stoa',
                 'tos.work.justus-lipsius.de-constantia',
                 'tos.intellectual-movement.logical-empiricism'):
    result = core.knowledge_focus(identity, depth=2, node_limit=100, relation_limit=200)
    center = next(n for n in result['nodes'] if n['id'] == result['focus']['node_id'])
    print(identity, select_human_forms(center, 'ru'))
    print([(n['entity_id'], n['type_id']) for n in result['nodes']])
PY
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python -m unittest tests.test_source_witness_bibliographic_graph
python -m unittest discover -s access/tests -p test_knowledge_contract.py
python -m unittest tests.test_tos_corpus_index
```

## Review and remaining owner work

Checklist: source traceability, authored/derived distinction, distinct Claim
identity, context/lineage, language-neutral IDs, public-safe provenance,
plurality and explicit uncertainty: **yes**. Assessment competence bypass,
fabricated human review, automatic causality or admission: **no**.
Canon/public mirrors, compost, calibration, gold, tiny-entry and lived-witness
changes: **not applicable**. The source-description and operation boundaries
are explicit; no UI, runtime, rights or publication owner was taken over.

Substantive historical assessment, richer multilingual forms, normalized
time, shared bibliographic transactions and actual UI interaction remain
incomplete. Source/assessment, growth and access retain their respective next
steps; no mandatory human signature per record is introduced. Reverting the
reader/registry change and rebuilding projections need not delete new source
records, creation history or forms. CI, merge, release, deployment and complete
Foundation v1 are not claimed.
