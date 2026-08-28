# Exact External Artifact Identity Is Provenance

## Index Metadata

- Decision ID: TOS-D-0034
- Original date: 2026-08-27
- Surface classes: scripts/validation, research-packet, provenance, docs/route-law
- ToS layers: scripts, research-packets, docs
- Tree classes: source
- Guard families: validator restraint, provenance preservation, active naming
- Posture: accepted

## Context

The Table I research intake binds each local DOCX by its exact relative path,
size, and SHA-256 digest.  One supplied A48 filename contains the retired word
`seed` immediately before its `.docx` suffix.  The active-naming guard correctly
rejects that combination when it is an active repository path or identifier,
but the intake manifest and its derived projections quote the filename as
external artifact identity.

Renaming the supplied file or normalizing the recorded path would change the
section fingerprint and weaken the evidence chain.  Excluding whole intake or
projection surfaces would also hide unrelated active-name regressions.

## Decision

Admit the complete A48 DOCX filename as one exact, content-only quoted external
artifact identity.  The exception applies only after the whole filename
matches.  It does not admit the shortened token-plus-suffix fragment, variants
of the filename, or any repository filesystem path.

The filename remains evidence, not an active ToS route or ID.  The A48 owner
route remains the `cross_chronology_frontier` branch established by
TOS-D-0033.  Its reviewed RU/EN display title names the Andes and Rapa Nui as
separate comparative frontier cases instead of copying the filename into
active owner vocabulary.  The intake manifest remains the authority for the
artifact path, size, digest, capture posture, and claim limits.

## Options Considered

- Rename the local DOCX and recompute the intake fingerprint.  Rejected because
  it mutates supplied evidence and breaks the requested frozen fingerprint.
- Store an encoded or shortened filename.  Rejected because it makes provenance
  less inspectable and still distorts the supplied identity.
- Exclude the manifest and generated projections from active-name validation.
  Rejected because the exception would become surface-wide instead of exact.
- Allow the shortened token-plus-suffix fragment globally.  Rejected because
  unrelated active references would then bypass the route-law guard.

## Consequences

The intake can preserve exact external provenance while the retired route term
remains forbidden in active paths and identifiers.  Any future external name
with retired vocabulary requires its own reviewed exact identity and negative
near-miss tests; this decision is not a general provenance bypass.

Validator success proves only naming mechanics.  It does not authenticate the
DOCX, establish authorship or origin, accept its semantic claims, or promote
the A48 dossier beyond manual pre-canon review.

## Source Surfaces

- `scripts/validate_active_naming.py`
- `tests/test_validate_active_naming.py`
- `ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-intake.manifest.json`
- `ToS/philosophy/atlas/multilingual/content-labels.json`
- `docs/decisions/TOS-D-0021-domain-vocabulary-and-active-route-naming.md`
- `docs/decisions/TOS-D-0033-non-era-philosophy-frontier-route.md`

## Validation

Run the focused active-naming tests and validator, regenerate and validate the
decision indexes, then run the broad repository release lane.  Preserve the
tracked Table I section fingerprints throughout regeneration.
