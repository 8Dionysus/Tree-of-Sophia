# Frozen Claim and Agent publication inputs

These fixtures preserve the maintained Python Claim/Agent setup and independent
full-graph oracle as bounded offline data. The Rust conformance cases materialize
the exact captured source trees, prepared SQLite rows, input packets, and the
complete post-Record Agent graph. They never invoke Python or regenerate an
expected value.

`provenance.json` binds the bundle to repository commit
`1d33e8f3dcc360c7e39a3b72db13d97a0d188f23`, fixture source SHA-256
`93b293020a9179f691599434c365336aaa3998b7f6f8deffd5b87f149619cc2d`, the
original per-capture member inventories, and the capture-manifest digests. The
Agent graph's transaction-ID pointer set is empty, so the test compares the
captured full graph byte-for-byte in meaning without substituting transaction
fields.

The loader verifies each compressed archive and every member's type, mode,
length, and SHA-256. It rebases only temporary absolute paths and their bound
owner/request/source-state digests so the fixture works in a fresh private
workspace. Captured authored source bytes, derived catalogs, prepared rows,
graph expectations, and operation assertions remain intact.

The capture controller completed the Claim and Agent oracle flows, then the
broader run failed its installed-layout assertion. This bundle is capture and
oracle evidence; it does not claim those conformance cases passed or establish
installed-layout acceptance.
