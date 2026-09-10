# Physical part composition: source and implementation review

Date: 2026-09-10. Reviewer: `agent:codex-tos-foundation`.
Scope: successor to the [intellectual/corpus structure route](2026-09-10-scoped-member-structure-review.md),
not closure of all Foundation v1 requirements.

## Owner change and assessment boundary

`physical_part_composition` reuses the bounded scoped-member value and its
local order checks. Both subject and referenced members must be physical
Artifacts. Digital Item/File, Work, intellectual fragment, scholarly composite
and research corpus are not permitted substitutes. Its literal value, Claim,
whole and component keep distinct identities. Competing Claims remain separate;
acyclicity and total-order constraints apply within one scoped Claim, never
to all historical accounts combined. Physical attachment, restoration,
spatial placement and completeness are not implied by this relation.

The [exact source reading](2026-09-10-oim-a00645-physical-component-source-reading.md)
grounds the real [OIM A00645 Artifact](../source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json)
and [partial, unordered composition Claim](../source-witnesses/relations/oim-a00645-physical-composition/source-claims.jsonl).
The old ensemble and the separately bound reading/discovery/rights inputs
remain unchanged. `source.create`, `claims.create` and `form.create` all
executed through their existing separate exact delegations; each exact retry
returned the same receipt without another publication. No human signature,
source admission or interpretation assessment was manufactured. Native
serialization events remain distinct from catalog capture and source reading.

The real retained HTML exposed an overly narrow v2 Artifact fingerprint:
`captured` previously could only be false. V2 now describes retained as well
as unretained bytes. A true value requires an exact independently bound
discovery snapshot and acquisition account (URL, hash and byte size).
This check never fetches a remote page, reads private descendants or grants
publication; false text/semantic/graph/canon/publication authority is retained.
V1, its August fingerprints and all original bytes remain unchanged. The
private HTML is not emitted in source, graph or public request/receipt files.

Checklist: source return, distinct layers, preserved history, qualified
uncertainty, stable identities, source/derived separation and owner boundaries
are **yes**. New canon, personal/lived witness, counterpart, calibration,
translation, practice lineage and tiny-entry changes are **not-applicable**.
This review inspects the new mechanism and the stated source reading; it is
not an independent museum examination or an agent-assessment admission event.

## Verification

The existing Artifact creation suite passed 8 tests in 26.878 s, including
new retained-snapshot binding and publication-authority negatives. Native
creation, exact retry, selected correction, old-grant refusal, stale inputs,
private paths, partial capture and interrupted publication remain covered.
The scoped structure suite passed 6 tests in 0.814 s, including cross-kind
physical-part rejection, wrong literal kind, cycles, self-parts, duplicates,
out-of-scope precedence and incomplete total orders.
The complete affected bibliographic-topology module then passed 23 tests in
1.500 s. The corpus index was rebuilt and its validator passed.

Source catalog and bibliography graph were regenerated from authored inputs.
The source-witness foundation validator passed. The actual union reader used
revision `182bd7e95d66134b126fb49a3a161a82547bfbd7788d30d9663bd7ba6b5237f3`
(42,272 nodes / 62,262 relations), returned the entire exact Artifact and
Claim, the source-copy name and qualified EN statement, and the complete
distinct composition value. Both endpoints exposed the Claim at depth two:
9 nodes / 8 relations for the ensemble, 7 / 6 for the component. Named
coverage/order properties found the partial unordered value. Source inputs
and retained private snapshot hashes were unchanged after the reader run.

Observed seconds: cold union 38.654222; warm union 0.000418; Claim inspection
0.339858; endpoint focus 0.391575 / 0.371218; Artifact inspection 0.000550;
property lens 1.789796. Peak memory was 1.4 GiB with no swap reported for this
run. These shared-host observations are not an isolated benchmark or a passed
latency budget. Command apply-plus-retry durations were 10.078 s for Artifact,
18.812 s for Claim and 5.885 s for the Claim form. Initial use of a raw external
URL as Claim evidence was correctly refused; the submitted Claim instead
returns to tracked exact source records and the reading note containing that
URL. No source-rule exception was added.

## Remaining integration

Source-home and documentation-family currentness checks passed after
regeneration. Registry transition against exact predecessor
`82e7e28156c5ae7bfb406cb30271e8b1e4561eff` passed with no violations.
Combined KAG currentness,
Worker/D1, UI consumption, CI and landing remain the integration owner's
obligation. This note does not claim those checks, deployment, physical
reconciliation, source-text publication or whole-goal completion. Collection
member order remains a separate source-contract task; this physical relation
does not duplicate or replace `contains_work` membership.
