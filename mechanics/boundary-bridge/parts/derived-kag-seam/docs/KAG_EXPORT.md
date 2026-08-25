# ToS KAG Export

This document records the current source-owned tiny KAG export posture for Tree
of Sophia.

The export is deliberately narrow.
It exposes one bounded source-node capsule for downstream KAG consumers without
replacing ToS-authored authority.

## Current pilot

The current pilot stays on the Zarathustra prologue route only:

- one exported object: `tos.source.thus-spoke-zarathustra.prologue`
- one canonical authored source node: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- one public compatibility entry surface: `ToS/public-compatibility/source_node.example.json`
- two supporting doctrine surfaces:
  - `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`
  - `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- one compact consumer export: `ToS/derived-exports/kag_export.min.json`

## Core rule

The export is a source-owned guide surface, not a new authority layer.

It may expose a bounded question, summaries, interpretation-layer handles, and
current route refs for downstream consumption, but authored ToS authority
remains in the canonical tree node and its supporting ToS docs, while the public
entry surface remains a compatibility mirror for the current tiny-entry seam.

## Current files

- `ToS/derived-exports/kag_export.json`
- `ToS/derived-exports/kag_export.min.json`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/prepare_tree_kag_owner_pair.py`

If you edit supporting surfaces in `docs/`, `ToS/public-compatibility/`, `ToS/derived-exports/`, `ToS/contracts/`, or `scripts/`, also follow the nested `AGENTS.md` in that directory.

## Current verification

For the current bounded export seam without regeneration, use:

Current verification is owned by
`mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`
and the `public_entry` sequence in `docs/validation/validation_lanes.json`.

`python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` checks generated export parity and
payload structure for this seam.
The release lane pairs it with canon, intake, route-card, and public-entry
validators when broader route assurance is needed.
`python -m unittest discover -s tests` strengthens repo-local contract and
schema coverage around that same bounded route.

## Regeneration

If you change export inputs or generation logic, use:

Regeneration is owned by
`mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py`
after the public-mirror owner has synchronized its inputs. The nearest
part-local `AGENTS.md` and the validation-lane manifest own the operator route.

## Tree owner-preparation boundary

The Tree owner-preparation adapter is a read-only witness for a future
cross-owner KAG pair. It records the canonical node, public mirror, supporting
surfaces, derived exports, local family manifest, source-index rows, producer
scripts, and execution environment with exact candidate seals. It classifies
materialized inputs separately from existence-only checks.

The current export builder reads the public compatibility mirror. The adapter
also requires observed byte/blob parity with the canonical node, but does not
claim that the current builder depends on canonical bytes. Its confined reads
reject symlinks, parent-directory escapes, unsupported Git modes, aliases,
stale index rows, missing `signs.digest`, and candidate/toolchain drift. Every
compatibility path declared by the local family manifest uses that same typed
relative-path, root-confined, regular-file, non-symlink reader; malformed
compatibility structures fail closed before classification.

The preparation packet keeps reproducibility evidence publication-safe. A small
allowlist preserves deterministic locale/interpreter controls, path-like
environment values are hash-only, and every `GIT_*` value is represented only
by presence, length, and a digest. Raw Git configuration or command values are
never written to the packet.

Run the bounded read-only fixed-point observation with `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/prepare_tree_kag_owner_pair.py --index-reuse`.

An exact sealed preparation uses `--check` with the candidate, producer, and
environment values shown by the unsealed JSON packet. If the external `aoa-kag`
procedure/schema is unavailable, the packet remains
`blocked_external_kag_contract` and the command exits `3`; this route emits no
receipt, semantic pair, admission, runtime claim, or owner acceptance.

The complete local family is regenerated through the accepted `aoa-kag`
provider-owned portable-family route before a Tree checkpoint is considered
current. That route refreshes the full portable family; `--index-reuse` remains
a bounded selected-row observation and is not a substitute for regeneration.
The provider-owned compatibility assembly route may materialize the declared
seven-file v2 view into a caller-selected directory. The Tree packet validates
each declared path when present and records an absent on-demand view as a
partial snapshot; these generated views never become authored source authority.
