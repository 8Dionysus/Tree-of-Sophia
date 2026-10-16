# Selected corpus native read fixtures

These frozen synthetic software inputs were captured during the successful R4 historical-oracle run. The monolithic corpus index is the exact ToS test input used for that run. The partitioned directory contains the exact seven selected software input prefixes in the R4 capture, including corpus and bibliographic `.parts` payloads. The Rust test verifies every committed file against `fixture-files.sha256`, creates a temporary Git commit, calls the native `tos_source_store::capture_git` and `restore_capture` APIs, and exercises the normal compiler/seal/cold-open path. The checksum file covers all 65 other files and is pinned by SHA-256 in the Rust test. No Python test helper or generated runtime database is included.

Capture provenance: historical Python commit `b095824a7a7728f16ce09a9c8c213c8944bce574`; tested source commit `db15df0d2a46a3c219a8d29fc5f101228500e9bf`; R4 capture provenance SHA-256 `bc98e9ac77b30738b768581a8644c4ce830785ccc2aeba24b1bd1c7197a3b662`; selected-lens product SHA-256 `4392830f969983432e821449eb193eec13797bc6169923915a52b663c822118e`.

These synthetic fixtures carry no ToS source, rights, or publication authority.
