# Historical KAG contract fixture

These unchanged JSON bytes come from Tree-of-Sophia commit `c0a66037c0e2f5a88e658555bdda5f054c22dc03`, before
TOS-D-0062 externalized integration artifacts. They are test data, not a current
KAG publication or runtime admission. `manifest.json` preserves
`kag/indexes/index_family.manifest.json`; `receipt.json` preserves the receipt
named by its `family_identity.content_digest` under `kag/receipts/index_family_budget/`.

Rust checks the portable producer, procedure, file, environment, dependency,
candidate and history bindings. An isolated fixture explicitly refuses live
remeasurement; current publication still requires the selected source, artifact
and external producer runtime. No Python oracle is executed by these tests.

- `manifest.json` SHA-256: `6ffe18237d571f60205ff4d629e6a7f9035c747f0b1b9b182f18ce6d5242b2a7`
- `receipt.json` SHA-256: `b5e9ca076d6904b184127bfd3d98c71e3f4e7b583e20fdd289f2cc43c9f65ebf`
