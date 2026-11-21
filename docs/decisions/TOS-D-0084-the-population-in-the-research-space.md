# TOS-D-0084 The Population in the Research Space

## Index Metadata

- Decision ID: TOS-D-0084
- Original date: 2026-11-20
- Surface classes: access/web, bonsai
- ToS layers: derived projection
- Tree classes: none
- Guard families: bounded downstream seams, review separation
- Posture: accepted by explicit operator instruction

## Context

TOS-D-0082 gave the Population bonsai a light page of its own at
`/population/`, outside the research space the other stands share, since its
material is people rather than texts. The operator asked for the space instead:
lenses and spatial views fit thousands of people as well as they fit works.

Two facts shaped how. The master tables of the research name, for each of the
190 batches, a research node with its title, macro-region and windows of time,
and TOS-D-0083 lets a bonsai publish what it grows from them. And the space
draws every star as a button on the page: measured headless on the Population's
tree, 3,466 stars cost 136 ms a frame (median), where the Aesthetics tree's 901
cost 37.7 ms.

## Decision

`/population/` opens the research space with the Population's own profile.
Its library holds the twelve regions the stand groups the research nodes under,
the 190 nodes with their titles from the master tables (in the stand's English
too), and the planted members; a region contains its nodes, and a node contains
every member whose register rows it holds. The space opens on the regions alone,
so that no person stands out at rest.

The tree is the banyan of the site's choice grown into a grove: the regions are
trunks round one ground, each region's nodes branch above it in three tiers by
the table of the research, and each member is a leaf at the node of its founding
row. A member whose rows stand in other nodes hangs a thread to each of them,
which says what the rows say and no lineage of ideas.

The sky may now draw material as dots, without a star of its own; the
Population's leaves are dots, and a member gets a star when it is chosen or
compared. A member is found by any of its names, in any script, and an old ID
merged into another leads to it. A member's card shows its names, its years in
the words of the register or of the line's readers with their source and
footing, the verdict that is not given, and each register row with its node.

## Options Considered

- Keep the light page (TOS-D-0082). It reads well but gives the population no
  space: no lenses, no tree, no view of the research's own structure.
- Every member as a star, as the other stands draw theirs. Over budget by four
  times on the tree.
- Members only of the chosen node. Light, but the whole population is never seen
  at once, and every lens would need its own way to unfold.
- Members as dots on the sky (chosen). The whole population stays in view in
  every lens; the tree costs 202 stars and 3,273 dots, 13.9 ms a frame.

## Rationale

The space is where the Tree's stands are read, and the Population's material
has a structure of its own to show: regions, research nodes in time, and the
people whose rows the nodes hold. Drawing the many as dots keeps every member
reachable without making the page carry a button for each, and keeps the cost
where the other stands' trees already stand. Size, brightness and the start
view say nothing of anyone's weight: every member is one dot, and the space
opens on the regions.

## Consequences

- The light page and its model (`access/web/src/population/`) are gone; TOS-D-0082
  remains the record of the bonsai itself.
- The sky's dots are shared code but opt-in: a view without points draws none.
- The space reads the corpus through Antiquity's selection, as the Aesthetics
  does, and counts none of its works as the Population's own.
- A reader's saved graph is still keyed by the library's fingerprint, so a new
  planting does not carry it over; the member IDs it holds stay valid through
  the redirects.
- Further lenses (time, the register's roles, proximity) follow on the same
  library.

## Source Surfaces

- `access/web/population/index.html`
- `access/web/src/public-research/population-library.mjs`, `population-tree.mjs`,
  `population-card.mjs`
- `access/web/src/public-research/space-library.mjs`, `public-space.mjs`,
  `space-legibility.mjs`, `research-index.mjs`, `bonsai.mjs`, `app.mjs`,
  `corpus-work-count.mjs`, `locales/*.json`
- `access/web/constructor/sky.mjs`
- `bonsai/review/population/nodes.py`, `plant.mjs`, `inputs/node-titles.en.json`
- `bonsai/lib/population.mjs`, `bonsai/README.md`

## Validation

`node bonsai/lib/cli.mjs check population`; vitest of the space and the
constructor (`src/public-research/`, `constructor/`); headless frame
measurements of the Population's tree beside the Aesthetics tree.
