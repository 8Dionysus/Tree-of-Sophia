# TOS-D-0043 Spark And Legacy Scaffolding Retirement

## Index Metadata

- Decision ID: TOS-D-0043
- Original date: 2026-09-04
- Surface classes: agent route, mechanics/topology, docs/route-law, scripts/validation, legacy/provenance
- ToS layers: mechanics, docs, scripts, tests, agent-lane
- Tree classes: support, event, state
- Guard families: owner boundary, source/history preservation, validator restraint
- Posture: accepted retirement record; current active routes remain authoritative

## Decision

Retire all listed Spark and legacy scaffolding roots from the current owner
surface. Historical bytes remain recoverable in git; this note records
immutable baseline links and does not recreate compatibility stubs. Active
validators and route maps must describe only current package surfaces.

## Baseline historical links

Baseline commit: [`4871641840c08709534dc8529c74da82422573c5`](https://github.com/8Dionysus/Tree-of-Sophia/commit/4871641840c08709534dc8529c74da82422573c5).

| Retired root | Baseline tree | Full historical tree link |
| --- | --- | --- |
| `.agents/spark/` | `7ad759962ef48f8dec2f9d156e79062e436c6a9f` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/.agents/spark) |
| `mechanics/agon/legacy/` | `51684b1edf1ca4f285be698868312e18d1701efe` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/agon/legacy) |
| `mechanics/audit/legacy/` | `497f27cb36bfc2ac14a6499ac74fff991963c848` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/audit/legacy) |
| `mechanics/boundary-bridge/legacy/` | `662c63b341931ae3ac7933644f8c7a8be68d6333` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/boundary-bridge/legacy) |
| `mechanics/distillation/legacy/` | `85a4ae9fc4537f9bc1f4477c02b5227901afe732` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/distillation/legacy) |
| `mechanics/experience/legacy/` | `67817e48cf9a624ffe0d6b4eb380e59682f0a6e4` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/experience/legacy) |
| `mechanics/growth-cycle/legacy/` | `3d9bcdd9007578f5f5011610042a8ff5dcd6c79d` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/growth-cycle/legacy) |
| `mechanics/questbook/legacy/` | `d7bf0696fb5e97af01e6e20046e393d710c2f8c8` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/questbook/legacy) |
| `mechanics/source-witnessing/legacy/` | `510e798c349c571ab58f5cac943e9f17ec062f45` | [tree](https://github.com/8Dionysus/Tree-of-Sophia/tree/4871641840c08709534dc8529c74da82422573c5/mechanics/source-witnessing/legacy) |

Each root is retired because it is archive-only scaffolding with no current
owner route. Recovery is by the immutable commit/tree links above or the
exact path/blob inventory in `surface-retirement-20260904/baseline.json`.

## Consequences

No runtime, memo, `.aoa`, remote, tag, or merge state is changed by this record. Generated indexes are read models and must be regenerated from this authored note. ToS meaning and active mechanics remain with their current owner surfaces.
