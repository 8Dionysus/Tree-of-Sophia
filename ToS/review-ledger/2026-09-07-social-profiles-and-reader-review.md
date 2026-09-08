# Social bodies, relationships and native reader forms, 2026-09-07

## Scope and source ownership

Entity registry v13 adds SocialGroup, Community and InstitutionalBody as
source-described corpus subjects. Community inherits SocialGroup; both and
InstitutionalBody descend from Organization, not SemanticObject or an atlas
navigation category. Their shared schema requires an identity criterion,
scope, wording languages and a profile-specific account. An institution is
not its place, and a community is not automatically an intellectual movement.
Existing atlas institution/school/tradition/movement identities are unchanged.
Four content properties, scope and identity-criterion properties are exposed
through the shared property-ID query contract.

Relation registry v12 adds eight source-attributed predicates: membership,
learning from a person, study and teaching at an institution, collaboration,
correspondence, friendship and conflict. Concrete endpoint constraints,
explicit statements, relationship basis, social scope and a historical-time
note are enforced. The last three fields are also queryable Claim properties.
All eight predicates are non-transitive. Symmetric relationship readings do
not create a second independent Claim, reciprocal self-identification or
evidence of intellectual influence. A time note is not normalized date algebra.

Creation, correction, source-copy forms and retained predecessor/replay
behavior use the existing owner commands. No profile-specific writer or access
dispatch branch was introduced. The source-description extension rationale
continues [TOS-D-0053](../../docs/decisions/TOS-D-0053-source-described-conceptions.md),
without retyping an existing atlas object or treating social bodies as thoughts.

## Real material and limits of reading

The immutable [source-reading note](2026-09-07-social-source-reading.md) records
four complete German institutional article readings and their limits. The
articles were not captured as local bytes; their current URLs are locators,
not exact-text or fixity claims. Reported chronological inconsistencies were
not imported as resolved dates. New Russian wording is research metadata,
not a certified translation or publication of article bodies.

Owner commands created six people, two universities, the Leipzig philological
society and the Vienna Circle community: ten provisional records, 12 uncertain,
unreviewed Claims and 42 source-bound forms (30 subject forms, 12 statements).
Nietzsche's existing identity was retained without rewriting its record.
The Claims separately represent seven Nietzsche relationships, three Circle
memberships and two founding collaborations. Neither membership nor teaching
becomes intellectual agreement, causality or a closed list of participants.

Records, exact creation requests and public-safe provenance remain under
`ToS/source-witnesses/agents/`, `social-bodies/` and
`relations/social-research/`. The maker is the actual Foundation agent under
the existing operator task authority. No assessment competence or historical
admission was self-granted; source-copy form readiness is not assessment.

## Reader defect found by the real integration probe

The initial full-reader probe failed on Ritschl's hover form. The source-claim
carrier already retained native corpus forms, but source navigation loaded
forms only for registry-declared profiles. Since the ordinary focus could
choose the navigation carrier, a valid Agent form disappeared there.

`scripts/tos_corpus_index_common.py` now uses the existing metadata-form loader
for native corpus records as well. It verifies their corpus schema, catalog
record identity/type and canonical source digest before associating forms.
Links and physical artifacts are not silently assigned this corpus adapter.
New source bytes with an old catalog fail; after catalog regeneration, old
forms remain explicitly stale instead of being rewritten or hidden.

The added permanent contract test exercises Agent, Place, Organization and
Work through both carriers and the ordinary focus selection. It verifies
exact records, form equality, unknown language qualifiers, absent admission,
catalog drift refusal and preserved stale forms. The test first reproduced
the missing form, then passed after the reader fix. This protects a shared
consumer boundary, not an incidental count in the real corpus.

## Validation observed

- Entity/relation closure and backward compatibility with parent registries
  passed; 116 types and 125 relation definitions were checked.
- Positive/negative social contracts passed, including required account and
  basis fields, inheritance, wrong endpoints, disputed and negated Claims.
- The focused source creation/correction test passed across 27 declared
  profiles in 78.810 s; full source-command suite: 36 tests, 110.577 s.
- Access knowledge suite: 58 tests, 25.025 s.
- Full graph suite after the native reader fix: 67 tests, 160.795 s.
- Corpus index suite after that fix: 9 tests, 30.729 s.
- Source catalog/foundation validation, present-byte fixity, graph and corpus
  index generation, exact parity and validators passed before this note.

Early setup failures (missing social-bodies parent and an invalid optional
null confidence value) were corrected before successful commands. Confidence
was omitted, not fabricated as zero. Retained creation requests were replayed;
existing subjects and source-reading input were not silently replaced.
The new test's initial import typo was corrected before its behavioral red/green
run. No failure was converted into a positive assessment or bypassed guard.

## Complete-reader observation

The final read-only probe used `ToSAccessCore.discover('.')` and the complete
local graph, normalized snapshot
`494b98b547a13270ec6c5a56b46d9cac7ac3002f18bf4361aadfbe136d5e4fe4`
(40,394 carrier nodes and 60,141 relations). All ten records remained exactly
equal to their source in both source-navigation and source-claims carriers,
with one shared entity identity. Russian name/hover and German name selection
worked in both carriers, with complete declared scope/content and no admission.
All 12 Claims retained their exact statements and whole-Claim context.
Seven body-content field checks and 36 Claim-property checks selected the
actual records through property IDs; equal qualifiers may select multiple
Claims and do not assert identity. Compact results omitted full attributes.

Depth-two focus was bounded to 100 nodes and 200 relations:

| Focus | Required reach | Nodes / relations | Seconds |
| --- | --- | --- | --- |
| Ritschl-learning Claim | Nietzsche and Ritschl | 41 / 49 | 0.349 |
| Ritschl | Nietzsche | 4 / 4 | 0.306 |
| Basel university | Nietzsche | 4 / 4 | 0.284 |
| Leipzig society | Nietzsche | 4 / 4 | 1.157 |
| Vienna Circle | Schlick, Hahn and Neurath | 8 / 10 | 0.268 |
| Overbeck | Nietzsche | 5 / 7 | 0.258 |

Python 3.14.7, Linux 7.1.13-200.fc44.x86_64, x86_64: first graph call 22.545 s;
whole probe wall 33.414 s, user CPU 32.594 s, system CPU 0.786 s; peak RSS
1,255,744 KiB. This is one process-cold measurement with warm OS caches, not
p95, filesystem-cold, isolated-host, growth scaling, Cloudflare/D1 or UI evidence.
The cold cost is a remaining Foundation scaling problem. Indexing this note
will change the snapshot; these numbers do not describe the later final HEAD.

Reproduce a bounded actual read without mutation:

```bash
PYTHONPATH=access/src python - <<'PY'
from tos_access.core import ToSAccessCore
from tos_access.knowledge import select_human_forms
c = ToSAccessCore.discover('.')
for identity in ('tos.agent.friedrich-ritschl',
                 'tos.institutional-body.university-basel',
                 'tos.community.vienna-circle-schlick'):
    result = c.knowledge_focus(identity, depth=2, node_limit=100, relation_limit=200)
    center = next(n for n in result['nodes'] if n['id'] == result['focus']['node_id'])
    print(identity, select_human_forms(center, 'ru'))
    print([(n['entity_id'], n['type_id']) for n in result['nodes']])
PY
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python -m unittest tests.test_source_witness_bibliographic_graph
python -m unittest discover -s access/tests -p test_knowledge_contract.py
python -m unittest tests.test_tos_corpus_index
```

## Source-first review and remaining work

Checklist: source traceability, authored/derived separation, distinct Claim
identity, context preservation, public-safe provenance, explicit maker,
language-neutral subject IDs, plurality and honest uncertainty: **yes**.
No new substantive assessment or admission is asserted; competence bypass,
fabricated human review and automatic causality: **no**. Canon/public mirrors,
compost, calibration, gold, tiny-entry and lived-witness changes:
**not applicable**. No UI, runtime, rights or publication owner was taken over.

Intellectual schools/traditions/movements, normalized historical time,
independent competence-backed assessment, fuller multilingual descriptions and
actual UI interaction remain incomplete. Source/assessment and access owners
retain those steps; no per-record human-signature queue is introduced here.
The reader change can be reverted and its derived index rebuilt without
deleting authored records, creation history or forms. CI, merge, deployment,
release and complete Foundation v1 are not claimed.
