# Partial Table II Medieval-Worlds Route

## Index Metadata

- Decision ID: TOS-D-0036
- Original date: 2026-08-29
- Surface classes: philosophy/topology, research-packet, provenance, scripts/validation, docs/route-law
- ToS layers: philosophy, research-packets, scripts, docs
- Tree classes: era, region, tradition, dossier
- Guard families: chronology boundary, identity quarantine, partial intake, provenance preservation, manual review, source-first authority
- Posture: accepted

## Context

The operator-local Table II packet contains fifty `T2-*` DOCX files for a
fifty-eight-row master table. Forty-nine supplied documents identify their
master rows. The supplied `T2-26` document instead identifies Ethiopian
Ge'ez ecclesiastical writing, while the canonical Table II row `T2-26` is
nominalism, logical analysis, and the fourteenth century. Rows `T2-51` through
`T2-58` have no supplied dossier.

The package also spans regions and traditions whose local chronologies do not
share one civilizational periodization. A stable route is needed without
turning a convenient Table II navigation label into a historical or canon
claim. The DOCX files remain non-authoritative research material; their exact
fixity and extraction coverage motivate this decision but do not author it.

## Decision

Plant the forty-nine master-aligned dossiers under a bounded chronological
navigation container at `ToS/philosophy/eras/medieval-worlds/`, then explicit
region and tradition homes. `medieval-worlds` means the Table II navigation
window only. It does not assert that Byzantine, Islamic, Jewish, Latin,
South Asian, Tibetan, East Asian, African, or Southeast Asian histories share
one essence, boundary, or synchronized chronology.

Keep the supplied `T2-26` file in the tracked intake and coverage accounting
with posture `blocked_master_identity_mismatch`, but emit no dossier index
row, branch, proposed node, relation, source backlog, term, transmission, or
graph projection under that false identity. Keep `T2-51` through `T2-58`
explicitly missing. A later corrected artifact may change those states only
through the route map and a regenerated intake, not by silently reassigning
the supplied bytes.

Table II rows with master status `B` or `C`, or confidence at most `3`, require
manual review. Mechanics may extract and route them as pre-canon candidates;
they may not convert the master confidence into source, semantic, doctrine, or
canon authority.

## Options Considered

- Require all fifty-eight dossiers before any planting. Rejected because it
  hides forty-nine usable, explicitly bounded routes and makes absence less
  inspectable.
- Plant all fifty supplied files by filename id. Rejected because `T2-26`
  would acquire a demonstrably false master identity.
- Reassign the supplied Ethiopian file to `T2-07`. Rejected because that would
  rewrite the external packet, invent a merge judgment, and erase exact
  artifact identity.
- Create region-first branches without a Table II chronological container.
  Rejected because it loses the current table window and makes later
  table-specific review harder to inspect.
- Use a bounded `medieval-worlds` navigation container with explicit partial,
  quarantine, and review states. Accepted.

## Rationale

This route preserves the owner chain: the master-table rows own Table II
identity, the route map owns current tree placement, the intake and coverage
surfaces own exact local-artifact accounting, and manual review owns any
historical or semantic promotion. Explicit absence and quarantine are more
truthful than either all-or-nothing intake or filename-driven identity.

The chosen container keeps regeneration deterministic while acknowledging its
limited scope. Region/tradition children carry the useful topology; the era
label carries only table navigation and is prohibited from becoming a claim
of global historical uniformity.

## Consequences

Prepared-dossier mechanics must support multiple table packages, partial
packages, quarantined inputs, package-local proposed-node and relation files,
and aggregate atlas projections without letting a Table I rerun erase Table II.
Validators must prove that quarantined material produces no semantic output,
that missing rows remain visible, and that manual-review rules follow master
status and confidence.

The first Table II planting is therefore `49 / 58` at the master-row level and
`49 / 50` at the supplied-artifact admission level. Neither ratio is a source
quality, historical coverage, review, or canon score. A corrected T2-26 or new
T2-51–T2-58 dossier requires exact new fixity and owner review.

## Source Surfaces

- `ToS/philosophy/AGENTS.md`
- `ToS/philosophy/atlas/master-tables/table-ii/rows.jsonl`
- `ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json`
- `ToS/research-packets/deep-research/philosophy/packet-contract.md`
- `ToS/research-packets/deep-research/philosophy/dossiers/table-ii-docx-intake.manifest.json`
- `ToS/research-packets/deep-research/philosophy/dossiers/table-ii-docx-extraction-coverage.json`

## Validation

Run the decision-index generator/check and decision validator, the prepared
dossier pipeline tests, philosophy topology and projection validators, then
the broad repository release lane. Green mechanics prove only deterministic
partial routing, quarantine, extraction accounting, and projection parity;
they do not review the DOCX claims or promote them to source or canon.
