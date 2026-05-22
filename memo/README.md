# Tree-of-Sophia Memo Port

This is the tree-local memory port for `Tree-of-Sophia`.

Use it for candidates, receipts, exports, and local notes that should be visible
to future agents without making `Tree-of-Sophia` the central memory authority.

| Path | Use |
|---|---|
| `PORT.yaml` | tree-local port contract |
| `INDEX.md` / `index.min.json` | generated local read model over packets |
| `candidates/` | proposed memory claims with evidence refs |
| `receipts/` | accept, reject, validate, or forward traces |
| `exports/` | reviewed-intake packets for `aoa-memo` |
| `local/` | tree-local memory notes that should remain local |

Default write mode: `write_candidate_only`.
