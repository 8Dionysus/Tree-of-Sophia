# AGENTS.md

This file applies to durable ToS decision rationale under `docs/decisions/`.

## Conditional route

Start with the repository root `AGENTS.md`, `docs/AGENTS.md`, and this card.
Open `README.md`, `CHARTER.md`, or `BOUNDARIES.md` only when their human
orientation changes the decision task. Then open the source, intake, tree,
example, generated, or script surface the decision describes.

## Local role

`docs/decisions/` is the durable ToS decision rationale lane.

Decision notes explain why a route, topology, boundary, validator, export posture, or source-discipline choice was made.
Current node contracts, source witnesses, authored tree surfaces, roadmaps,
review checklists, mechanics surfaces, Zarathustra route surfaces, and
generated export payloads keep their own owner routes.

## Authority

Decision notes are weaker than the current source, tree, and doctrine surfaces they describe.

Use this lane to preserve rationale when a future agent would otherwise need to rediscover:

- why a ToS route exists
- why a layer boundary was chosen
- why a downstream export seam is bounded
- why a validator or index exists
- why an option was rejected or deferred

Use the owning surface itself for current meaning:

- source authority stays in `ToS/source-witnesses/`
- golden Zarathustra route surfaces stay in `ToS/zarathustra/`
- non-authoritative research scaffolds stay in `ToS/research-packets/`
- candidate structure stays in `ToS/candidate-intake/`
- canonical authored meaning stays in `ToS/canon/`
- public compatibility stays in `ToS/public-compatibility/`
- derived export stays in `ToS/derived-exports/`
- operation mechanics stay in `mechanics/`
- doctrine stays in `ToS/doctrine/`
- review law and repeatable operation docs stay in `mechanics/`

Generated indexes under `docs/decisions/indexes/` are lookup read models only.
Keep `modeled_surfaces` in `docs/decisions/indexes/index_contract.yaml` as a
top-level list of normalized repo-relative paths under `docs/decisions/`; do
not use it for root non-record Markdown.

## Record shape

New decision records use:

- canonical filename prefix `TOS-D-####`
- full path shape `docs/decisions/TOS-D-####-short-slug.md`
- an `## Index Metadata` block with `Decision ID`, `Original date`, `Surface classes`, `ToS layers`, `Tree classes`, `Guard families`, and `Posture`

Existing record numbers stay stable. If a decision changes, add a new
superseding record and say what it supersedes.

## Boundary Routes

- Current contracts, source surfaces, canon, roadmap, runtime, and KAG substrate
  keep their own authority; decisions explain why routes exist.
- New source, intake, tree, export, or generated behavior routes to its owning
  surface before or alongside the rationale note.
- Cross-repo symmetry is translated into ToS source-first layer language.
- Generated indexes are regenerated from decision metadata.

## Validation

Select the decision route in [`docs/VALIDATION.md`](../VALIDATION.md). The
decision index builder and record validator remain the owner procedure; also
select the owning route for the source, tree, export, or script surface the
decision describes.
