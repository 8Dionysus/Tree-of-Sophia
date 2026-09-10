# Document catalogue attribution grammar review

## Scope and result

Reviewed against `73ac445273cd583d7e39c49bedc8d956ae8ab346`: relation registry
42 → 43, navigation template 2 → 3, the separate Document catalogue date reader,
two existing-reader Place relations and the new date-only command delegations.
TOS-D-0063 records this accepted boundary. No real Claim, Place, source grant,
archive catalogue content or historical date was created or modified.

The synthetic end-to-end test copies an existing Letter's metadata, then makes
explicitly synthetic catalogue attributions. Its `03. 06.1886` test wording is
not an observation of the archive card. The actual card's independently reported
`3.6.1886` must remain its own exact source wording, not be unified with the
Sommer-attributed historical date. A real record heading such as `Eintrag` may
be a source field; the declared field role must not invent absent provider
labels. A selected substring requires its surrounding locator/context in the
qualified statement or retained qualifiers.

## Boundary review

Under the source review checklist:

- Yes: source traceability, full Claim/evidence context and authored-versus-derived
  distinction remain explicit; a catalogue attribution is not an event occurrence.
- Yes: Document/Letter, temporal value, Place identity and historical situation
  remain distinct. Historical domains and old reader/grant meanings are unchanged.
- Yes: exact source language/wording and nullable calendar/year numbering survive.
  A normalized value is not a source citation, an inferred calendar or admission.
- Yes: source-copy statements retain the whole Claim; the new short-wording
  adapter is nonstandalone navigation, not an invented substantive HumanForm.
- Yes: creation, correction, retained versions and replay preserve exact scoped
  authority. Discovery is grant-free and does not authorize the new reader.
- Yes: independent attributions and uncertainty remain possible; no identity,
  rights, publication, assessment or canon boundary is crossed.
- Not applicable: canon/public mirrors, lived witnesses, counterpart mapping,
  compost, calibration, branch growth and golden-kernel promotion are unchanged.

Date comparison checks the exact current profile, predicate/schema/layer,
Document subject, Claim version/content revision, same source file/line and
canonical Claim/value/literal binding before comparing equal-role envelopes.
Unknown calendar/year numbering yields undetermined; different roles cannot
become a historical comparison. This still validates declared metadata, not
the catalogue's truth or the correctness of a human's source reading.

## Verification

- New owner-command end-to-end test: passed, including two explicit date
  envelopes, source-null calendar/numbering, wrong domain/field/evidence/wording,
  old create/revise grants, descriptor opt-in, exact profile/digest/literal
  failures, statement HumanForm, retained predecessor and replay. Latest run:
  27.369 seconds, 72.8 MiB service peak, zero swap.
- Existing historical temporal creation/correction test: passed.
- Source-command discovery: 6 tests passed, no source/config reads required.
- Temporal comparison API: 15 tests passed, including existing transport and
  exact snapshot/version behavior.
- Source/access Claim navigation: 7 + 7 tests passed.
- Exact-base semantic registry transition: passed with no violations.
- Decision indexes regenerated; parity, decision records, source-home and
  whitespace checks passed.

These are 37 focused tests across the listed commands, not the full corpus or
release suite. An initial new-test assertion expected a scalar title instead
of the existing localized title map; it was corrected to inspect `default`
before the passing complete end-to-end runs. No source contract was weakened.

## Remaining owner

Foundation integration owns combined projections, registry/context presentation,
actual HTTP consumer inspection and any later exact grants. The separate
catalogue-Claim draft must retain its actual source spelling and unresolved
Naumburg identity until its owner admits that identity. No permission is granted
by this review. Full corpus/KAG parity, CI, merge, deployment, source assessment
and whole-Foundation acceptance remain unclaimed.
