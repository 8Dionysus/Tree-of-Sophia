# AGENTS.md

This card applies to the stand bonsai under `bonsai/`.

## Role

A bonsai is a public stand's self-contained mini-Tree: the navigation,
chronology, places and lens settings one stand reads, kept in the Tree's own
shapes so that the stand can experiment freely and still hand what it finds to
the Tree. Each `bonsai/<stand>/` has a manifest and its modules; `bonsai/lib/`
holds the shared readers, the Tree-contract check and the export. Stands under
`access/web/src/public-research/` consume bonsai modules and do not author them.

## Boundary

- A bonsai proposes for its stand. It never writes into `ToS/` and
  admits nothing; its export is a packet of candidate records and claims for
  the Tree's own source-owner routes, every claim unreviewed.
- Every dating is a Tree `historicalDate` value — a date, an interval, an
  order relative to another dated situation, or an explicit unknown — with its
  certainty and the source's own wording, in astronomical years. A display
  fallback is never stored as data; a gap stays a gap.
- Every dating and place names its source and how firmly it stands.
- Modules refer to the bonsai's own IDs. The graft map in each bonsai's
  `identities.json` says which Tree identity each one stands for and whether
  the Tree already holds it.
- The Tree grows independently. The manifest records the Tree revision and
  contract digests of the graft; drift is reported, and the export is checked
  against the contracts in the current checkout.
- A module that saved personal graphs depend on keeps its material IDs when it
  moves or changes, or its stand declares how the earlier graph upgrades.

## Change and verify

[README](README.md) explains module shapes, the check and the export. Select
the bonsai route in [VALIDATION.md](../VALIDATION.md): the stands' tests carry
each bonsai's module, Tree-contract and export checks. Card or routing changes
also select `route_docs`.
