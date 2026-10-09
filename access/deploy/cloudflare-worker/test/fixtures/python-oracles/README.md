# Frozen former Python outcomes

These two Brotli files preserve bounded outputs captured from the former
Python fixture/reference calls used by selected Worker tests. They are test
data only: no Python interpreter or module is loaded, and the outcomes do not
own current ToS meaning, source acceptance, rights, or publication.

`frozen-oracles.v1.json.br` contains 510 successful call records from 13 Worker
test sources and descriptors for 285 deduplicated outputs. Each record binds
the callsite source path and digest, source commit, exact former program digest,
stdin digest and byte count, output digest and byte count, and any capture-only
execution overlay. SQLite-backed calls also bind the exact database and
sidecar file sizes and SHA-256 digests. Ordered sequence records retain their
index. `frozen-oracle-output.v1.br` is the shared output byte bank.

The Worker test helper verifies compressed and decoded bank hashes, then
selects an outcome by exact source, program, input, and SQLite-state identity.
It verifies the selected output digest before returning those bytes to the
existing test assertions. A missing, mismatched, exhausted, or ambiguous
record fails the test. Historical source and output provenance remain in the
metadata; local capture-storage paths are intentionally omitted.

Changing these fixtures requires a newly reviewed capture against the exact
Worker test revision. A green fixture comparison proves only that the current
Worker test result matches the recorded output; it does not revalidate current
source semantics or establish production acceptance.
