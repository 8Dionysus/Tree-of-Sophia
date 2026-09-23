# Rust migration conformance inputs

These small synthetic cases are expected behavior from the authored/source and
published access contracts at baseline `8ca90023c20a65d109dfe7d6b2f561aaf634a59d`.
They are not copies of a live corpus or output re-recorded from Rust. A legacy
implementation can be compared against them, but it is not the sole oracle.

`foundation.jsonl` supplies UTF-8 text (or raw hex for invalid UTF-8), the
operation, and exact observable results. `reject` means no value is admitted;
the FND error-code column will be frozen when its public error enum is stable.
The only canonical-byte cases here use the supported corpus v1 subset. Floats
in that canonical profile remain a declared E1 gap until Python parity is
established; a float rejection is not evidence of full v1 compatibility.

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

No Rust implementation is imported by these fixture files. OPS owns the Cargo
runner/CI wiring; FND and STO own their public APIs. The scenario-to-risk map
and current validation limits live in the migration execution evidence folder.
