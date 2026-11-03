# ToS Validation Topology

This directory owns the repository-level validation topology for
`Tree-of-Sophia`.

## Role

Validation lanes name the checks that protect ToS routes. They do not create
philosophical authority. Source witnesses, doctrine, canon, mechanics packages,
decision records, generated read models, tests, and local eval ports keep their
own owner surfaces.

## Surfaces

| Surface | Role |
| --- | --- |
| `../../VALIDATION.md` | on-demand human selector for named internal lanes and district validation routes |
| `validation_lanes.json` | executable command authority for named validation lanes |

Repeated argv live once in `validation_lanes.json` under `shared_commands`.
A sequence step selects either inline `command` or a direct `command_ref`;
its label, deadline and position remain part of that sequence. Every selected
sequence executes all its ordered steps. Unknown or ambiguous references refuse.

The `release_check` test suffix may declare `software_test_group` as `access`,
`query` or `command` for CI package cohorts. Once annotated, every test must
have a known group and all three groups must be present; checks cannot carry
that tag. `tos-release-check --phase tests --test-group NAME` validates the
whole suffix before selecting a cohort from the same manifest snapshot.
Ungrouped `all`, `checks` and `tests` retain their authored commands and order.
The release procedure owns the required CI matrix and aggregate gate.

The `source_witness_foundation` lane runs the native full audit and catalog
parity through `tos-native-owner-command foundation`. Run it inside the
ToS private-tmpfs owner launcher, which supplies the sealed
`TOS_STAGE_TICKET_FD` and matching `TOS_STAGE_ROOT`. Select the owner-issued
protected invocation explicitly as
`TOS_NATIVE_FOUNDATION_INVOCATION=/absolute/path/to/invocation.json`; it binds
the installed command and schema-worker digests plus the finite source, CPU,
stage, state and output budgets. The lane forwards that sealed FD only to the
native foundation command, preserving the launcher's existing ticket checks.
Without both the invocation selection and issuer ticket, `--run
source_witness_foundation` refuses before the audit starts. `--check` only
validates the authored lane manifest and does not need either input.

Root and district `VALIDATION.md` files are human maps, not inherited prompt
cards or second machine manifests. They may select a lane or preserve an
external-owner procedure; exact internal sequence membership and order remain
here in `validation_lanes.json`.

## Boundary Routes

- Source-home checks route to `ToS/source_home.manifest.json`, the nearest
  `ToS/**/AGENTS.md`, and `ToS/VALIDATION.md` after the source branch is known.
- Mechanics checks route to `mechanics/topology.json` and the package README's
  operation map.
- Mechanics-local contract tests route to the owning mechanic lane before
  broader repo tests.
- Generated parity checks route from source surface to builder to generated
  artifact to validator.
- Local eval pressure routes to `evals/`, while proof authority stays with
  `aoa-evals`.

## Validation

Use root `VALIDATION.md` to inspect or run a named lane. The
`validation_authority` sequence owns manifest self-check; the `release_check`
sequence owns standalone software checks and `tos-release-check` remains
its entrypoint. `software_browser` checks browser behavior on fixtures after `tos-release-check` builds the assets.
The former full aggregate is available through an explicit
`integration_snapshot_audit` selection, independently of software merge. Blocking lane posture applies inside
the selected owner operation, under TOS-D-0062.
