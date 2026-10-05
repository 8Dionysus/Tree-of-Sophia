# TOS-D-0070 Declared source validation scope

## Index Metadata

- Decision ID: TOS-D-0070
- Original date: 2026-10-05
- Surface classes: corpus/contract, scripts/validation, docs/route-law
- ToS layers: contracts, source-witnesses, doctrine, docs
- Tree classes: corpus
- Guard families: source-first authority, exact-byte provenance, bounded execution, compatibility
- Posture: accepted

## Context

The implementation owner selected this extension under the Operator-authorized
Rust migration and the repository agent mandate. This records that owner
decision; it does not attribute the specific profile choice to the Operator.

The historical corpus validator performs a conservative whole-source audit.
Its default Foundation route requires the repository's synthetic laboratory
and Goldset districts. A selected corpus or historical source closure does
not necessarily include those unrelated research packets. The existing
lab-only mode is explicitly ineligible for admission. TOS-D-0069 requires
validation to declare exact inputs and dependency scopes; it does not make
all future corpora copies of the current repository.

## Decision

Add an explicit software-owned validation profile catalog at
`ToS/doctrine/semantic-interchange/source-validation-profiles.v1.json`.
Native admission selects a declared profile with `--validation-profile ID`.
Omission remains `full-audit`; its existing required Labs and Goldsets and
whole-audit meaning remain intact.

The `selected-source-closure` profile runs the existing Records,
bibliography, rights, review, reference, dependency-closure, Discovery and
Closure owners over authenticated selected membership. It executes the
actual scheduled schema requests under the same candidate fence and binds
local completed work to the declared queue. Unselected whole-repository
Labs and Goldsets have explicit absent district reports, not fabricated
successful evaluations. This remains true when Labs or Goldset files are
present in selected membership: ordinary Records/reference checks may cover
those bytes, but their district audit is not claimed. The report binds its scope. Exact catalog bytes
and selected profile ID participate in validator identity and receipts. Existing admission consumers require the exact selected
validator hash, so a scoped result cannot satisfy a whole-audit validator pin.
The selected scope does not require the historical whole-tree private/handoff,
server-plan coverage, topology, derivation or chronology demonstration batches.
Discovery traverses authenticated selected family members and provenance streams;
Closure validates their declared Claims, endpoints, backlinks and dependencies.
A selected record's required target or contract remains mandatory even when the
historical demonstration packet is outside this scope. Full audit retains its
original batch requirements.

Unknown profiles, unsupported scope dialects and missing required selected
inputs refuse admission.

## Options Considered

- Keep only whole audit: preserves its meaning but leaves generic selected
  corpus admission without the scoped owner adapter it needs.
- Skip absent laboratory directories: rejected because filesystem absence
  cannot declare scope or turn required missing material into success.
- Reuse lab-only mode: rejected because its weaker evaluation does not
  execute the corpus admission invariants.
- Force all current research packets into each corpus: rejected because it
  changes selected corpus membership and couples independent corpora to
  unrelated synthetic evidence.
- Explicit authenticated scope: accepted; retains each route's actual claims.

## Rationale

One authored catalog owns profile selection. Supported scope dialects map
to existing owner kernels rather than corpus names or manually copied path
lists. The declaration is compiled with the validator software, so a held
historical candidate does not need newly inserted grammar bytes. Binding its
exact digest prevents a receipt from silently changing scope. Validation
continues to prove declared mechanics only; source assessment, rights, canon
and publication retain their stronger owner routes.

## Consequences

Selected corpus admission can omit unrelated whole-repository districts only
through an explicit declared profile. Missing required dependencies remain
failures. Existing ordinary commands do not silently accept the new flag,
and the full-audit default does not become a selected audit. Runtime success
and capacity acceptance still require actual measured consumer evidence;
this decision record is weaker than current contracts and implementation.

## Source Surfaces

- `ToS/doctrine/semantic-interchange/source-validation-profiles.v1.json`
- `ToS/doctrine/semantic-interchange/README.md`
- `rust/crates/tos-validation/src/source_foundation_default_rules.rs`
- `rust/crates/tos-command/src/source_foundation_cli.rs`
- `rust/crates/tos-command/src/source_foundation_admission_identity.rs`
- `rust/crates/tos-command/src/source_foundation_admission.rs`
- `rust/crates/tos-command/src/source_foundation_orchestrator.rs`
- `rust/crates/tos-command/src/source_foundation_rule_diagnostics.rs`

## Validation

Regenerate decision indexes with `scripts/generate_decision_indexes.py`,
then run its `--check` mode and `scripts/validate_decision_records.py`.
Parser and identity regressions cover explicit profile selection, unknown
and conflicting flags, declaration identity and scope-dependent validator
identity. Genuine native admission and missing-reference controls remain
required runtime evidence, distinct from source and decision-index checks.
