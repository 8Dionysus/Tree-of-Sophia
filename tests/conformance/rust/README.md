# Rust migration conformance inputs

These small synthetic cases are expected behavior from the authored/source and
published access contracts at baseline `8ca90023c20a65d109dfe7d6b2f561aaf634a59d`.
They are not copies of a live corpus or output re-recorded from Rust. A legacy
implementation can be compared against them, but it is not the sole oracle.

`foundation.jsonl` supplies UTF-8 text (or raw hex for invalid UTF-8), the
operation, and exact observable results. `reject` means no value is admitted;
the runner compares it with the public FND error code. The original E1 codec
failed closed on canonical floats; the expanded oracle below is the independent
gate before a later FND profile can claim Python parity.

`canonical-profiles-v1.jsonl` holds 17 separate Python-oracle edge vectors for
future FND expansion: finite binary64 rendering at exponent and rounding
boundaries, negative zero, subnormal and maximum finite values, integers above
2^53 and u64, Unicode key order/escaping, nested values, and an integer at
CPython's default 4,300-digit limit. A companion source-parse vector checks
4,301 digits fail with the declared budget code. Each oracle row records
exact bytes and SHA-256 for two distinct existing owner algorithms:
`scripts/corpus_store.py::canonical` appends one LF, while
`scripts/source_record_profiles.py::catalog_entry` hashes the compact sorted
UTF-8 JSON body without that LF. The expected values came from Python
`json.loads`/`json.dumps`, not Rust. The runner compares both snapshot and
source-record outputs and their digests when the new FND API is integrated.
The foundation cases retain overflow, non-JSON `NaN`, and lone-surrogate
negatives. `SourceCommandInputV1` is checked only as a separate strict-input
byte profile; command identity/receipts require CMD owner acceptance and
whole-history vectors before any authority claim.

`corpus-v1/` is a tiny independent immutable store, with two exact revisions
and three synthetic objects. Its `fixture.json` names revisions, selected
descriptors and negative mutations. A test runner must copy it into a private
temporary directory before mutating any bytes or symlinks. Store lookup checks
only selected object bytes, while manifest validation examines the selected
revision's full member/index metadata. Raw store access is trusted and grants
no public visibility; public/private checks belong to an owner adapter.

`source-return.jsonl` contains six schema and semantic envelope cases for an
exact historical `{id, version, digest}` reference. It deliberately belongs to
the later source-owner/public adapter gate, not the v1 corpus locator API.
Unavailable envelopes retain the requested ref and withhold source bytes;
available results retain unknown record fields without granting current use.
The exact record digest uses compact sorted UTF-8 JSON **without** the corpus
snapshot's final newline; these two canonical profiles must not be conflated.

The `runner.rs` integration test reads these independent expectations and
calls only the public FND/STO APIs. Its 21 foundation cases and tiny store
checks avoid the production corpus. OPS owns this directory's Cargo manifest,
workspace registration, lockfile and CI wiring; FND and STO own their APIs.
The scenario-to-risk map and current validation limits live in the migration
execution evidence folder.

On Linux, the runner also swaps the root, revisions, and objects pathnames
after opening a reader. Exact old bytes must still come from opened directory
capabilities. A symlinked revision component and a FIFO selected object must
be refused; an ancestor symlink in the absolute root path must also fail.
These are deterministic hostile fixtures; they do not claim to
exercise every possible concurrent rename interleaving. The Linux FIFO probe
uses `mkfifo` and a subprocess watchdog so a regression that blocks in `open`
fails within ten seconds instead of hanging the suite.
