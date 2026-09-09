# Assessed source-copy and parent-context consumer review

## Scope and source authority

This review covers the source-bound form materializer, its explicit local graph
snapshot adapter and the Python/Worker form-selection readers. The base is
`856f0669d6bb8ef21cad207774e512d3efdb5872`. No corpus source, private payload,
assessment journal, grant, rights record or historical form was changed.

Two concrete gaps were found by following the existing source-copy route:

- The assessed metadata adapter required an authored whole-subject binding,
  although the ordinary source-copy constructor correctly stored the narrower
  owner-declared field context. Its initial `context.omitted` refusal was
  preserved during diagnosis. The assessed owner now adds whole-subject context
  as `owner:subject`, without editing the form or removing any field guard.
- Both consumer readers rejected ready assessed source-copy snapshots because
  they recognized only freeform derivation. They now retain either production
  mode with its own exact form admission and the same no-publication/no-live-grant
  observation boundary. Template authority is unchanged.

Separate parent assessment remains part of the complete materialization, not
an endorsement inferred from a form. The snapshot adapter already retained
that member; it was not a data-loss bug there. The new observation marker makes
loss of just the required companion or marker an explicit consumer error.
Readers verify subject, policy/use, journal, withdrawal references and limits,
retain unknown admission details, and refuse over-budget output without
truncating qualifications. Positive, rejected, disputed and unreviewed parent
states remain distinct and do not determine the form's own admission.

## Mechanical evidence and limits

Focused Python/Worker parity tests cover both derivations, six parent states,
twenty malformed/context-loss cases, exact packet preservation and over-budget
inspection. A synthetic source-copy assessment over copied public metadata
also follows the actual public journal command, graph snapshot adapter and
Python selector: ordinary copy readiness does not open the assessed gate;
its own committed assessment does; grant revocation closes it again; stored
source/form bytes stay unchanged. These are executable mechanical fixtures,
not additional historical or linguistic judgments.

The pre-existing transport operation-set assertion omitted the already
implemented temporal-comparison operation; its expected set was synchronized
with the current owner contract. No operation was added by this repair.

Final source tests: `access/tests/test_knowledge_contract.py` plus the mechanics
`test_knowledge_assessment.py`, `test_human_forms.py` and
`test_native_text_layer_assessment.py`: **197 passed, 530 subtests, 64.10 s**.
Four focused source/assessed/Claim form tests in Worker `test/knowledge.test.ts`
passed, including Python parity; `npm run typecheck` passed. These Worker checks
use the local JavaScript reader, not a new D1 import or remote runtime. The
earlier context refusal and stale operation-set failure are not counted as
successful runs. Documentation currentness, source-home validation and the
documentation cross-corpus, verify-route and mechanics-route test modules passed.
Private v4/v5 inputs remain excluded from public graph assembly. This review does not
claim real confidential graph transport, D1 deployment, UI acceptance, general
language quality, CI, merge or completion of Foundation v1.

## Manual review

Source traceability, authored/derived separation, exact identity/history,
mandatory context, bounded delivery and assessment authority: **yes**. No
source-copy assessment becomes a rewrite of source wording or a parent verdict.
No model output selects its own grant; no new per-record human signature is
required by this repair. Canon, public compatibility, lived witness, counterpart
mapping, compost and operational sibling ownership changes: **not applicable**.

The additive snapshot member and stricter transport validation require compatible
readers; stripping it is not a migration. Reverting the consumer code cannot
erase any source or journal. The integration owner takes the reviewed commit
and refreshes only affected generated companions before its own CI/landing
checks. Broader forms coverage remains with ToS and access, not this review.
