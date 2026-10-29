# AGENTS.md

This card applies to owner-local statistical questions in `stats/`.

## Role and routes

ToS owns measurement questions over its authored objects and declared
populations. Shared measurement grammar and cross-owner composition remain
with `aoa-stats`; philosophical meaning stays with the authored tree.

Read `stats/port.manifest.json`, the selected packet and its population's source
owner. [README](README.md) defines the current reference measurement,
`Tree-of-Sophia/table-i-prepared-dossier-route-ratio`, including its unique
Table I rows, prepared-dossier routes and unknown-versus-zero rules.
When packet shape changes, use the [embedded portable format](../rust/crates/tos-ops-mechanics-plan/schemas/README.md).

## Boundaries

Declare each measurement's population, unit, derivation and evidence revision.
Distinguish an observed zero from missing, unknown or invalid input according
to that measurement's contract. A reference snapshot retains its observation
identity and time; it is not live state. Route coverage alone does not assess
source adequacy, philosophical value or canon status.

## Validation and closeout

Inspect the selected population, derivation and packet, then use
[VALIDATION](VALIDATION.md) for supported ToS-native validation/export checks.
A new measurement must fit that supported format or extend its owner contract.
Consumers check composition and admission in their own environments.
Report the question, exact population and revision, checks, uncertainty and
packet/export posture.
