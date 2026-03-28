# Zarathustra Relation Residue Review

Date: 2026-03-28

## Scope

This note records why the expanded route-local canonical relation pack for
`thus-spoke-zarathustra/prologue-1` does not promote every row from
`intake/.../edges.csv`.

## Outcome

The canonical relation pack under `tree/relations/` now promotes 125 edges
whose two endpoints are already canonical ToS surfaces.

The remaining 3 rows stay in `intake/edges.csv` with visible blockers:

- 3 `deferred_literal`

## Reason

This deferment is boundary discipline, not silent rejection.

- `deferred_literal` rows still depend on literal helper surfaces that remain
  intake-only.

The expanded relation pass therefore keeps canon honest by promoting every
fully-settled edge and leaving only literal helper residue visibly
deferred.
