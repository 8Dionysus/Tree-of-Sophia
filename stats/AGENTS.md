# AGENTS.md

Route card for owner-local statistical questions in `Tree-of-Sophia`.
Read the root `AGENTS.md` first.

## Applies To

Everything under `stats/`.

## Role

This directory owns statistical questions over Tree of Sophia objects whose
meaning remains with the authored tree. Shared measurement grammar and
cross-owner composition remain owned by `aoa-stats`.

This port measures declared populations; it does not own philosophical meaning.

## Read Before Editing

1. Root `AGENTS.md`, `CHARTER.md`, `BOUNDARIES.md`, and `ROADMAP.md`.
2. `ToS/philosophy/AGENTS.md` and `ToS/philosophy/atlas/README.md`.
3. The Table I manifest, rows, and prepared-dossier route map.
4. `stats/README.md` and `stats/port.manifest.json`.
5. The central measurement and packet contracts under `aoa-stats/stats/`.

## Boundaries

- The population is the complete non-empty set of unique Table I atlas rows.
- The numerator contains only unique explicit Table I prepared-dossier routes
  whose row identity belongs to that population and whose branch path exists.
- A valid population with no routes is an observed zero.
- Malformed, empty, duplicate, unsupported, out-of-population, or missing-path
  input is unknown, not zero.
- The reference packet is weaker than the atlas rows, route map, branch homes,
  dossier contents, source witnesses, review packets, and canon.
- Route coverage does not measure dossier quality, source adequacy,
  philosophical value, branch maturity, graph readiness, or canon status.

## Validation

Inspect the atlas population, route map, branch paths, and packet first. The
port validator requires a compatible `aoa-stats` checkout through
`AOA_STATS_ROOT`, `.deps/aoa-stats`, or the sibling `../aoa-stats` path; CI
supplies its pinned checkout explicitly, and an unavailable central validator
is a failed check. Then run:

```bash
python scripts/validate_local_stats_port.py
python -m unittest tests.test_local_stats_port
```

Use the root route for repository-wide validation.

## Closeout

Report the Table I question, exact row and route populations, manual positive
and negative cases, packet posture, central validation, and repository
validation.
