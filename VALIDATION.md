# Tree of Sophia validation

This is the on-demand human route for repository validation. It is not an
inherited agent card and does not author philosophical meaning.

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
| source witnesses, provenance, rights, or corpus mechanics | `source_witness_foundation` |
| philosophy atlas or graph workbench | `philosophy_topology` |
| mechanics topology or package-local checks | `mechanics_topology`, then `mechanics_local` |
| canon or candidate intake | `canon_contracts` or `intake_contracts` |
| generated readers | `generated_parity` and the affected export lane |
| public entry or local KAG provider | `public_entry` or `local_kag_provider` |
| owner-local statistics | `local_stats_port` |
| cross-family documentation | `cross_corpus_documentation` |
| release-visible change | `release` through `scripts/release_check.py` |

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

Execute the complete repository gate:

```bash
python scripts/release_check.py
```

Run the narrowest relevant route first. A green command proves only its named
mechanical contract; source review, rights, semantics, canon, external-owner
acceptance, CI, publication, and runtime health remain separate claims.
