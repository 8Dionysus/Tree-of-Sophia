# Boundary Bridge Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | ToS material crosses into KAG, public compatibility, or sibling handoff |
| input | source-owned export, public mirror, derived read model |
| output | bounded bridge seam and owner split |
| owner | `mechanics/boundary-bridge/` |
| stronger route | `ToS/` remains authored truth; sibling repos own their layers |
| next route | [Derived KAG Seam](parts/derived-kag-seam/README.md) or [Public Mirror Sync](parts/public-mirror-sync/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --public-mirror-validate`; `tos-ops-mechanics-plan --kag-source-export-verify --kag-export EXPORT` |

## Operations

Public mirror parity remains the local source/canon compatibility check.
The derived KAG seam is optional and separately selected; local source review,
mirror parity and software validation do not require a live KAG owner.
Selecting that seam requires its exact export inputs and downstream-owner checks.

Each part owns one bridge operation route.


| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Derived KAG Seam](parts/derived-kag-seam/README.md) | keep KAG/public handoff bounded and downstream | `ToS/` authored truth and downstream KAG owner |
| [Public Mirror Sync](parts/public-mirror-sync/README.md) | keep public compatibility mirrors aligned with canon without making them canon | `ToS/canon/` and source review |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `derived-kag-seam`: [KAG_EXPORT](parts/derived-kag-seam/docs/KAG_EXPORT.md)

## History

Immutable Git history preserves moved-path accounting from the former
`ToS/doctrine/KAG_EXPORT.md` placement. Current derived KAG seam work starts
from `parts/derived-kag-seam/`.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/boundary-bridge/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Boundary Bridge is active as the ToS derived-seam guard. It lets ToS hand
bounded read models outward without moving authority out of the tree.

### Current Contour

- Keep the derived KAG seam as a bounded downstream handoff.
- Keep public mirror sync aligned with source and canon routes before public
  examples widen.
- Keep public compatibility, generated exports, runtime projections, and KAG
  consumers visibly weaker than ToS source, doctrine, branch, and canon
  surfaces.
- Keep bridge material in the owning part only when it is a repeatable seam,
  not source-authored meaning.

### Next Work

- Clarify seam posture when ToS adds new graph views, corpus indexes, public
  compatibility mirrors, or runtime-facing projections.
- Keep bridge growth paired with a stronger ToS source route and a downstream
  owner handoff.
- Keep `abyss-stack` visualization and `aoa-kag` substrate consumers separate:
  the stack displays, KAG consumes, ToS authors.

### When Time Comes

Time has come when: a second derived seam repeats enough that `derived-kag-seam`
or `public-mirror-sync` would hide its boundary.

Move: add a new bridge part with its source owner, downstream owner, and return
route named.

Guardrail: the bridge may transport read models; it may not transfer
philosophical authority.

### Out Of Scope

- Authority transfer.
- KAG-as-canon.
- Runtime activation.
- Sibling truth claims.
