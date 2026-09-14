# Historical contract bytes

This directory retains exact earlier ToS schema bytes needed to resolve
recorded provenance inputs. It is not a schema registry or the current contract
source. Active schemas remain at their original `ToS/contracts/*.schema.json`
paths; declared profiles and current validators select those active paths.

Each retained file is named `<sha256-of-exact-bytes>.json`. Do not reformat or
edit it. A changed byte sequence has another digest and another file. The
historical `$id` still identifies its original schema path; it does not make
this snapshot an active schema or overwrite that path.

`validate_source_witness_foundation.py` can resolve a recorded schema input
through this directory only when the active schema exists, the retained
bytes match the exact recorded digest and their `$id` matches that active
schema's ToS URI. The historical file is bounded to 1 MiB; symlinks, path
escapes, wrong identities, missing bytes and malformed JSON are refused.
No Git history, network lookup, digest restamping or schema migration occurs
during validation. Ordinary source/evidence inputs still need current exact
bytes, and output/current-record validation remains unchanged.

The initial snapshot
`2f319b7bb1fe146d42422685e5d3c727aa2cde539919c18ff6d6ac4f9b1a6019.json`
is the 5,748-byte `corpus-record.schema.json` at commit
`afc87a39cd2398738c56f72dcb21509661dd2832`. It was restored byte-for-byte,
not reconstructed from a remembered schema, and resolves prior recorded
inputs without changing those events. This establishes byte availability,
not the truth or authenticity of their original execution.

Rationale: [TOS-D-0052](../../../docs/decisions/TOS-D-0052-historical-contract-input-bytes.md).
