# Frozen source command fixtures

`manifest.json` retains the captured per-case source paths, bytes, modes and
historical provenance. `shared/` stores byte-identical schemas and registry
inputs once; case-specific records and command requests remain in `fixtures/`.
The reader verifies each selected member against its capture hash and copies it
into the isolated case root. Shared storage does not read current production
schemas or change the captured source bytes.
