# Tree of Sophia validation

This on-demand human route selects repository validation. Agent inheritance
follows `AGENTS.md`; philosophical meaning belongs to the authored ToS
sources.

## Authority

`docs/validation/validation_lanes.json` is the sole machine authority for
internal lane membership and command order. This file explains selection;
`scripts/validation_lanes.py` loads and executes the selected sequence.

## Select a route

| Changed surface | Start with |
| --- | --- |
| AGENTS cards or route topology | `route_docs` |
| model-facing skills or owner ports | `agent_surface` |
| source-home structure | `source_home` |
| semantic registry/profile changes | `semantic_registry_transition` with the exact baseline required by `docs/RELEASING.md` |
| source witnesses, provenance, rights, or corpus mechanics | `source_witness_foundation` |
| philosophy atlas or graph workbench | `philosophy_topology` |
| mechanics topology or package-local checks | `mechanics_topology`, then `mechanics_local` |
| canon or candidate intake | `canon_contracts` or `intake_contracts` |
| generated readers | `generated_parity` and the affected export lane |
| public entry or local KAG provider | `public_entry` or `local_kag_provider` |
| owner-local statistics | `local_stats_port` |
| cross-family documentation | `cross_corpus_documentation` |
| standalone software | `release` through `scripts/release_check.py`; `software_browser` for browser behavior after the software build |
| Rust workspace and crates | `rust_workspace` with the pinned toolchain, WASM target and matching wasm-bindgen CLI; it verifies only implemented Rust packages and generated WEB.1 Node host bindings |
| data or historical integration | select the affected owner operation in `docs/RELEASING.md`; no combined integration gate |

Use the nearest district `VALIDATION.md` when it names a narrower external
owner, mutation-bearing builder, or package-specific procedure.

## Run

Inspect an exact current sequence without executing it:

```bash
python scripts/validation_lanes.py --sequence route_docs
```

Execute one selected internal sequence:

```bash
python scripts/validation_lanes.py --run route_docs
```

Execute the full software contracts and fixture-based behavior route locally
(CI selects affected checks as described in `docs/RELEASING.md`):

```bash
python scripts/release_check.py
```

Run the narrowest relevant route first. A green command proves only its named
mechanical contract; source review, rights, semantics, canon, external-owner
acceptance, CI, publication, and runtime health remain separate claims.

Under TOS-D-0062, blocking owner lanes block their selected admission or artifact
publication. They are not universal merge obligations. KAG, stats, corpus
currentness and generated inventory parity do not belong to the software gate.
See `docs/RELEASING.md` for the separate data and integration operations.
