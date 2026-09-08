# Inquiry profiles and real JGB reader review, 2026-09-07

## Scope and owner changes

Entity registry v11 adds source-described Aspect, PhilosophicalCategory,
Problem, ProblemFamily, Question, Position, Distinction and Opposition through
the existing semantic metadata reader. The new `thought-topic-record` schema
requires a profile-specific account, explicit scope/continuity and wording
languages. Opposition is a Distinction subtype and inherits its differentiation
property; there is no second current property ID for that same criterion.
Ten content properties execute through the existing property-ID filters.

Relation registry v10 adds ten specific reified predicates for problem
grouping, questions, proposed answers, stances, aspects, categories and the
two contextual term roles of a distinction. The existing thought-expression
and attribution predicates admit these eight profiles without broadening
their meanings. No `Thing → Thing`, global acyclicity, exhaustive-family rule
or two-term completeness rule substitutes for their declared semantics.

No writer or access Python branch was added. Source/create/revise/form,
query/catalog and predecessor retention reuse the existing interfaces.
Tests extend the existing graph and source-command owners. The declared
source-profile rationale remains [TOS-D-0053](../../docs/decisions/TOS-D-0053-source-described-conceptions.md);
this change does not silently retype any canonical or atlas subject.

## Real source use

The [source reading](2026-09-07-jgb-inquiry-source-reading.md) records a fresh
complete read of JGB 19 and 21, its exact retained representation and limits.
Shared owner commands created ten provisional subjects, 21 uncertain,
unreviewed Claims and 51 source-bound forms (30 subject forms and 21 Claim
statements). The source records and creation packages live in
`ToS/source-witnesses/semantic-descriptions/`; the Claims and their creation
history live in `ToS/source-witnesses/relations/jgb-inquiry-research/`.

The two problems remain distinct within their research family. Questions
and the cross-passage distinction are researcher formulations, not fabricated
verbatim authorial questions. The causal-category account describes the
passage's treatment of cause/effect without imposing it as core ontology.
The position is the bounded double refusal in JGB 21, not a timeless account
of Nietzsche. The proposed positive answer to the reversal question remains
the earlier explicitly hypothetical thesis, connected to its addressed
objection. No historical reader is invented as its holder.

Source-copy forms preserve the exact declared subject/version and complete
semantic scope/content, or complete Claim context. Their `ready` state does
not provide substantive assessment or admission; every new creation receipt
leaves admission false. Existing source bytes, rights and historical decisions
were not rewritten. This is a research extension, not canon or publication.

## Verification and corrections during review

- Registry/schema closure and comparison with parent registries passed.
- The new positive/negative inquiry contract passed: required accounts,
  concrete endpoints, question-versus-answer distinction, inherited Opposition
  endpoints, explicit basis and both reading directions.
- Source-command suite: 36 tests passed in 59.465 s. The final focused
  creation/correction case then passed in 32.643 s across the two concept
  profiles, four reasoning profiles and eight inquiry profiles. It also checks
  inherited property queries, exact previous bytes, unchanged scope/ID,
  uninterpreted fields, source-bound context and idempotent receipt replay.
- Access knowledge suite: 58 tests passed in 22.696 s. The property-ID and
  inquiry checks passed again after removing the redundant subtype property.
- Graph contract suite: 64 tests passed in 111.314 s with the real data.
  Graph generation and exact parity passed again after the property cleanup.
- Source-foundation validation passed, including present-byte fixity; corpus
  index validation passed before final review-document regeneration.

The first new test accidentally reused its creation-receipt variable for a
query result. This test error was corrected and replay checks rerun. An early
graph test run overlapped regeneration and correctly failed exact parity;
the quiescent reruns passed. Neither failure was bypassed or converted into
acceptance. The field/property checks are structural; no synthetic case is
presented as historical evidence or an interpretation-quality score.

## Actual complete-reader probe

The local read-only probe used `ToSAccessCore.discover('.')`, not a synthetic
graph, at normalized snapshot
`d5da5030d08321d20028d4110e1cff37616a436d7f32f6cd5e9c6fdeb7b8deeb`
(40,082 carrier nodes / 59,685 relations). All ten records survived exactly in
both `source-claims` and `source-navigation` carriers. These are two carriers
of the same `entity_id`, not two newly created subjects. All 21 Claims survived
exactly. Russian name/hover/statement selections returned their source wording
with mandatory context and no admission. Every new content property selected
its real subject, including the inherited Distinction property on Opposition;
compact results omitted full attributes.

Depth-two focus, bounded to 100 nodes / 200 relations:

| Focus identity | Required reach checked | Nodes / relations | Local seconds |
| --- | --- | --- | --- |
| `tos.problem-family.agency-responsibility-jgb` | both distinct problems | 6 / 7 | 0.229 |
| `tos.question.reversal-after-rejection-jgb21` | parent problem and hypothetical proposed answer | 6 / 7 | 0.219 |
| `tos.opposition.metaphysical-and-mechanistic-freedom-jgb21` | both specified conceptions | 8 / 10 | 0.222 |

Environment: Python 3.14.7, Linux 7.1.13-200.fc44.x86_64, x86_64. First graph
call in this process: 16.648 s. The entire probe, including preservation,
three focus calls and all property selections: wall 20.717 s, user CPU 20.074 s,
system CPU 0.624 s, peak RSS 1,225,788 KiB. This is one process-cold local
observation with existing OS caches, not a filesystem-cold run, hosted result,
scaling result or p95. Cold cost remains substantial. Adding this review to
the repository index will itself change the normalized snapshot.

Reproduce a source-preserving focused read without any writes:

```bash
PYTHONPATH=access/src python - <<'PY'
from tos_access.core import ToSAccessCore
c = ToSAccessCore.discover('.')
for identity in ('tos.problem-family.agency-responsibility-jgb',
                 'tos.question.reversal-after-rejection-jgb21',
                 'tos.opposition.metaphysical-and-mechanistic-freedom-jgb21'):
    result = c.knowledge_focus(identity, depth=2, node_limit=100, relation_limit=200)
    print(identity, [(n['entity_id'], n['type_id']) for n in result['nodes']])
PY
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python -m unittest discover -s access/tests -p test_knowledge_contract.py
```

## Remaining limits

Source-visible competence-backed assessment, scoped admission, exact lexical
occurrences, full bilingual descriptions and actual UI interaction remain
unproved for these new records. A Russian source form is not a verified
translation of the English semantic account. New profiles require no special
screen in the backend contract, but that does not verify UI consumption or
smoothness. CI, merge, release, deployment and complete Foundation v1 are not
claimed. Source/assessment and access owners retain these concrete next steps;
no new mandatory per-record human queue is introduced.
