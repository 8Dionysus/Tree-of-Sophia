# AGENTS.md

This file applies to durable ToS decision rationale under `docs/decisions/`.

## Read first

Before editing decision records here, read:
1. the repository root `AGENTS.md`
2. `docs/AGENTS.md`
3. `README.md`
4. `CHARTER.md` and `BOUNDARIES.md`
5. the source, intake, tree, example, generated, or script surface the decision describes

## Local role

`docs/decisions/` is the durable ToS decision rationale lane.

Decision notes explain why a route, topology, boundary, validator, export posture, or source-discipline choice was made.
They do not own the current node contract, source witness, authored tree surface, roadmap, review checklist, or generated export payload.

## Authority

Decision notes are weaker than the current source, tree, and doctrine surfaces they describe.

Use this lane to preserve rationale when a future agent would otherwise need to rediscover:

- why a ToS route exists
- why a layer boundary was chosen
- why a downstream export seam is bounded
- why a validator or index exists
- why an option was rejected or deferred

Use the owning surface itself for current meaning:

- source authority stays in `sources/`
- candidate structure stays in `intake/`
- canonical authored meaning stays in `tree/`
- public compatibility stays in `examples/`
- derived export stays in `generated/`
- doctrine, review law, and public route docs stay in their current `docs/` files

Generated indexes under `docs/decisions/indexes/` are lookup read models only.
Keep `modeled_surfaces` in `docs/decisions/indexes/index_contract.yaml` as a
top-level list of normalized repo-relative paths under `docs/decisions/`; do
not use it for root non-record Markdown.

## Record shape

New decision records use:

- canonical filename prefix `TOS-D-####`
- full path shape `docs/decisions/TOS-D-####-short-slug.md`
- an `## Index Metadata` block with `Decision ID`, `Original date`, `Surface classes`, `ToS layers`, `Tree classes`, `Guard families`, and `Posture`

Do not renumber existing records. If a decision changes, add a new superseding record and say what it supersedes.

## Hard no

Do not:

- use a decision note to bypass a stronger current contract
- turn decisions into source canon, node law, relation law, roadmap authority, runtime control, or KAG substrate truth
- hide new source, intake, tree, export, or generated behavior inside a rationale-only note
- copy sibling repository mechanics when ToS needs source-first layer language
- hand-edit generated indexes

## Validation

After adding or editing decision metadata, run:

```bash
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```

Also run the owning validator for the source, tree, route, export, or script surface the decision describes.
