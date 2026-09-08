# Tree consumer pin for the bounded segmented KAG family

## Index Metadata

- Decision ID: TOS-D-0060
- Original date: 2026-09-08
- Surface classes: kag/provider, release/compatibility, validation, GitHub workflow
- ToS layers: derived-export, docs, owner-handoff
- Tree classes: provider edge, generated read model, source return
- Guard families: exact identity, bounded reads, migration rollback, currentness
- Posture: accepted for the staged provider route

## Context

The current ToS portable family is a v3 compatibility carrier. The complete
source family now exceeds the v3/v4 48 MiB materialisation ceiling, so raising
that ceiling or silently replacing the old reader would weaken the owner
boundary. `aoa-kag` provides a separately versioned segmented v1 family whose
segments are independently addressed and whose compatibility assembly remains
explicit.

## Decision

Tree-of-Sophia consumes the segmented family only through the source-owned pin
at [`kag/provider-pin.json`](../../kag/provider-pin.json):

- provider source revision:
  `d9b00bc456ea95dd8447311331ee83ba51afa023`;
- family schema:
  `aoa-repo-local-kag-segmented-family-v1`;
- schema handle:
  `aoa-kag:schemas/repo-local-kag-segmented-family.schema.json`;
- local consumer adapter: `scripts/validate_local_kag_provider.py`.

The adapter validates the control manifest, source-index identity, every
segment digest, record count, record key, and byte budget without assembling
the complete family. It retains the v3 path as a compatibility fallback; v3
and v4 consumers do not implicitly read segmented output. Migration and
rollback remain explicit provider selection by manifest digest, with the last
good manifest retained.

The workflow action pin and the generated family receipt must name the same
provider revision and exact family digest. This source route does not activate
an external registry, MCP resource, runtime graph, deployment, or semantic
authority.

## Options Considered

- Raise the existing 48 MiB v3/v4 ceiling. Rejected: it removes the bounded
  compatibility invariant and still materialises the complete corpus.
- Keep generating only the v3 family. Rejected: the current source family
  cannot be admitted under its declared global owner budget.
- Introduce an explicit segmented provider route with a source-owned pin.
  Chosen: each read stays bounded while the logical corpus can grow under a
  separately declared ceiling.

## Consequences

Consumers must select the pinned segmented provider deliberately and use a
bounded segment reader. Complete compatibility assembly is an explicit,
budgeted operation and is not part of the local provider validation claim.
The old family remains a rollback carrier until a downstream consumer has
accepted the new route; its existence does not make it current.

## Source Surfaces

- `kag/provider-pin.json`
- `kag/manifest.json`
- `scripts/validate_local_kag_provider.py`
- `.github/workflows/repo-validation.yml`
- `docs/RELEASING.md`
- `kag/indexes/index_family.manifest.json`
- `kag/receipts/index_family_budget/<family-digest>.json`

## Validation

Run `python scripts/validate_local_kag_provider.py`, the focused provider
tests, the source-family validator from the pinned `aoa-kag` revision, and the
affected release/owner lanes. Local validation proves source and generated
integrity only; CI, merge, runtime activation, and external consumer
acceptance remain separate claims.
