# Confidential source Claim growth and exact retained history

Date: 2026-09-08. Reviewer: source-owner agent under the continuing Foundation
v1 operator mandate. Scope: protected source commands and their shared reader,
not linguistic acceptance, publication, canon or Foundation completion.

## Owner change and inspection

`mechanics/growth-cycle/parts/branch-growth-cycle/README.md` specifies the
explicit `tos_local_owner_claim_command_v1` delegation. Its adapter,
`source_owner_claim_commands.py` in that part's `scripts/`, implements bounded
`claims.create`, `claim.revise` and common form changes through the existing
source command envelope. The public access plane remains read-only and cannot
discover the private store through this route.

The Claim/body/endpoint/form grammar remains with the existing source owners.
`scripts/source_owner_claim_profiles.py` now grounds an in-memory candidate
using the same exact typed source selections as stored-source reading. It does
not invent a source file, accept caller-authored endpoint bodies or create a
second relation registry. Candidate and stored-reader modes cannot be mixed;
their immutable snapshots retain distinct mode and input bindings.

The writer reuses the protected profile writer's account/context checks,
public-then-private locks, flat packages, mode-0700 directories, mode-0600
files, byte-bound archives and atomic rename/exchange. Claim history uses the
existing `tos_claim_revision_history_v1` and reconstructs the exact source
stream. Creation uses the common Claim receipt and records the actual writer
among its serialization software inputs. A correction actor need not be the
original maker and cannot rewrite that maker or the first form creator.

Root inspection covered the entire new writer, shared candidate reader and
archive callbacks, source dispatch, identity inventory, configuration/operation
scope, native closure, form materialization, all new tests and the public
refusal boundary. A bounded helper implemented the candidate/archive seam,
extended adversarial tests and independently inspected the writer. Its report
was review input; the source and failing reproductions were inspected by root.

The review found and corrected these concrete gaps:

- Quoted anchors needed their own evidence grant rather than inheriting a
  general source selection.
- A later Claim's relation-selection mismatch had to fail before the first
  Claim's private source read. All shapes, relation selections and grants are
  preflighted across the batch. Rights checks still precede each selected
  representation read; this does not claim an all-or-nothing read transaction.
- Internally consistent current forms or a v1 label did not prove exact
  creation. Every inspect/update now checks the retained request, creation
  closure, initial Claim stream, configuration bytes and initial forms against
  the original receipt, preserving legitimate later form history separately.
- Creation, Claim revision and sibling form changes must not reuse one command
  identity. Retried results also recheck current source/rights/context and the
  exact current package after response materialization.
- Slicing an implementation list omitted the actual new writer from
  serialization provenance. An explicit named software tuple replaces it.

## Verification

All executable test fixtures are synthetic. They do not read real private
source text, establish rights or competence, or produce real assessments.

The two final review reproductions first failed as expected: appending a
newline to the retained configuration was accepted by describe/prepare-revise,
and actual writer provenance was absent. After correction both passed (2 tests,
31.181 s). The final complete writer run passed all 17 tests in 466.784 s.
It includes two-Claim sibling preservation, exact creation/revision/form replay,
unknown-field retention, scope and identity refusals, corrupt archive/creation
refusal, distinct correction actor, late delegation/context revocation and real
subprocess loss on both sides of atomic exchange. The subprocess exits with
`os._exit(73)`; recovery does not rely on a normal Python exception unwinding.

Other completed affected checks:

- Shared private Claim reader: 29 tests passed.
- Private profile commands and archive callback: 30 tests passed; the focused
  immutable archive reader control also passed separately.
- Original public source commands: 39 passed in 373.921 s; public Claim commands:
  26 passed in 257.975 s.
- Private Claim assessment: 11 passed in 92.246 s; existing owner-local v4
  assessment: 21 passed in 88.853 s.
- Script and test topology: 16 passed in 1.615 s.
- Corpus-index and cross-corpus documentation tests: 44 passed in 32.104 s.
  Route-card validation covered 56 cards. The corpus index and documentation
  currentness companions were rebuilt and validated through their owners.

Reproduce the changed boundaries with:

```bash
python -m unittest tests.test_source_owner_claim_profiles -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_owner_claim_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_owner_profile_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_claim_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_owner_local_claim_assessment.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_owner_local_assessment.py -v
python -m unittest tests.test_script_topology tests.test_test_topology -v
```

## Manual boundary checklist and remaining work

Source traceability, separate Claim/object identity, explicit authority,
unchanged source layers, retained versions, complete qualified human forms,
uncertainty and private/public separation: yes within the inspected scope.
The writer returns `local_only`, `publication_authorized: false` and
`grants_admission: false`; no reviewer identity or competence is fabricated.
Source copying, mechanical readiness, substantive assessment and scoped use
remain separate. Canon, translation acceptance, lived witness and stronger
AoA runtime/proof/memory authority changes: not applicable to this slice.

Initial alternatives are confined to this selected batch. Only declared and
understood semantic/identity relation profiles are writable here; structured
values, other assertion layers and endpoint-identity changes do not acquire
permission by resemblance. There is no cross-store global uniqueness proof,
indexed constant-cost writer or hostile same-UID isolation. Existing bounded
metadata inventory and history ceilings remain explicit. Interrupted staging
is not a new source or permission to remove committed archives.

No real private Claim has been created by these synthetic checks. The next
source-owner step is the existing real DTA packet, separate Occurrences and
lexical-form relation, followed by independently selected v4 source-visible
assessment. This review does not establish substantive quality, execution
competence, admission, UI consumption, CI, merge, deployment or published
runtime health. Companion parity is a source-navigation check, not private
content publication or substantive assessment.
