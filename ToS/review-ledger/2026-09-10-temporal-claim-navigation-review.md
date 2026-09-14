# Typed temporal Claim navigation review

## Scope and result

Reviewed against `e4b9fd12fa11d51dd0850fee0b288df55a10fa99` the version-2 opt-in
historical-time source-wording adapter in the existing Claim-navigation template.
The relation registry moves from version 41 to 42; the reader's six-slot syntax
and nonstandalone purpose remain unchanged. The initial identity-only decision
is retained in TOS-D-0057 with a dated successor link; TOS-D-0067 (branch-local TOS-D-0062 when reviewed) records only
this explicit eligibility extension.

The real `tos.claim.jenseits-1886-commission.date` now contributes its exact
German source string `03. 06.1886` to the navigation field catalogue. It does not
contribute the separately normalized `1886-06-03` or an invented translation.
Its source Claim, scholarly-report attribution, reported/unreviewed posture,
qualifications, evidence and original normalized temporal value are unchanged.
The descriptor stays nonstandalone; compact reading remains incomplete and no
HumanForm or supplied source title is created.

## Boundary review

Source traceability, authored-versus-derived distinction, literal-versus-identity
separation, full Claim context, language authority, and the source/access boundary
remain intact under the review checklist. The builder and independent access
verifier bind source wording to the full Claim/version/digest, exact literal ID,
source file/line and canonical whole-value digest. A boolean cannot substitute
for a numeric value inside extensions. An arbitrary display label or normalized
date cannot replace the source wording.

The adapter requires the existing historical-temporal profile and understood
temporal range. Dates, intervals, relative order and unknown dates preserve the
same exact-wording rule. No calendar, year numbering, precision, historical
ordering, credibility or admission is inferred. Unsupported types, missing
wording, wrong range, ambiguous literal and stale bindings fail closed; version 1
without the opt-in keeps its previous refusal. Registry/schema validation rejects
unknown, duplicate, empty or non-array adapters and a version-1 opt-in.

## Verification and remaining owner

- Focused source/access Claim-navigation tests: 14 passed plus 56 subtests,
  25.62 seconds; observed service peak 123.8 MiB, zero swap.
- Exact-base semantic registry transition against `e4b9fd12fa11d51dd0850fee0b288df55a10fa99`:
  passed.
- Owner decision indexes regenerated; index check and decision-record validator
  passed. The previous decision's original metadata and v1 rationale remain.

An initial focused run found a new test's unresolved graph-path constant; it was
corrected to the existing `GRAPH_PATH` constant before the complete passing run.
This was a test wiring error, not source data or a weakened assertion.

The integration owner must regenerate the combined bibliographic graph and other
dependent companions, preserve the normalization binding, and repeat actual
HTTP/scene inspection. Worker/D1 carries the resulting normalized descriptor; no
new independent rendering path was introduced or newly claimed verified here.
Full corpus parity, release/KAG regeneration, CI, merge, deployment and whole
Foundation acceptance remain separate from these focused checks.
