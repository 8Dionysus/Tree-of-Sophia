# Claim schema history availability review

Date: 2026-09-09 UTC. Source-work baseline:
`9ec7307f863b4725ef6c1694fdbcef3ee1b8fa98`.

The identity-transition implementation at
`0a0c4ec87d84a053d101f912aae10b086c577894` added `identity_assertion` to the
current Claim packet's assertion-layer vocabulary. Existing retained
Expression-derivation and provision-activity provenance still names the exact
earlier Claim schema digest. An integration source-foundation run rejected
those unresolved historical input bindings. It did not diagnose an Artifact
origin failure or establish that the old events had been rerun.

The existing [historical-contract route](../contracts/history/README.md)
already separates recorded inputs from active schema law. This change adds
only the missing [exact earlier schema bytes](../contracts/history/cc982ceeaa3a62dc8cb1367cd4b10cc76073b252a1ac5c9977f6b0ee6d47588c.json):
5,602 bytes, SHA-256
`cc982ceeaa3a62dc8cb1367cd4b10cc76073b252a1ac5c9977f6b0ee6d47588c`.
They were recovered from the actual pre-change Git object
`d84e4973ddfe2f25efb7633e484a0b735d0d9eb3:ToS/contracts/claim-packet.schema.json`.
`cmp` against that object and a separate SHA-256 read both passed. No
reformatting, reconstruction from memory or provenance restamping occurred.

The existing resolver requires the active schema to exist, exact retained
byte digest and the same original `$id`. Its current 1 MiB, path, symlink and
JSON restrictions remain unchanged. Ordinary source inputs cannot use this
archive. Current output/schema validation still uses the active schema; old
schema restrictions do not become new assessment policy or reintroduce a
human-only barrier.

Manual review: yes for exact source traceability, immutable historical
identity, current/historical contract distinction and unchanged provenance.
No new interpretation, rights, assessment, canon, publication or runtime
authority is granted. No new resolver, contract registry, decision or
per-incident test is needed: the existing historical-input negative controls
protect the continuing boundary.
That focused test passed (1 test, 98 deselected, 0.07 seconds), including
missing/wrong bytes, wrong schema identity, unsafe paths, symlinks, malformed
JSON and missing active-schema refusals. Documentation-family parity,
source-home validation and `git diff --check` also passed.

The integration owner must apply these bytes alongside the expanded active
schema and rerun the complete source-witness foundation check. The local
history handoff alone is not a green combined foundation, CI or merge claim.
