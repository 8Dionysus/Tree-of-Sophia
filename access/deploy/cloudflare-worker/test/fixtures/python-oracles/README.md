# Frozen former Python outcomes

These two Brotli files preserve bounded outputs captured from the former
Python fixture/reference calls used by selected Worker tests. They are test
data only: no Python interpreter or module is loaded, and the outcomes do not
own current ToS meaning, source acceptance, rights, or publication.

`frozen-oracles.v1.json.br` contains historical call records from the selected Worker
test sources and descriptors for deduplicated outputs. Each record binds
the callsite source path and digest, source commit, exact former program digest,
stdin digest and byte count, output digest and byte count, and any capture-only
execution overlay. SQLite-backed calls also bind the exact database and
sidecar file sizes and SHA-256 digests. Ordered sequence records retain their
index. `frozen-oracle-output.v1.br` is the shared output byte bank.

The Worker test helper verifies compressed and decoded bank hashes, then
selects an outcome by exact source, program, input, and SQLite-state identity.
It verifies the selected output digest before returning those bytes to the
existing test assertions. Known reference refusals retain their exception
outcomes with local runtime prefixes removed. Nine temporal mutation calls
retain exact before/after JSON and metadata rows; the helper applies those
bytes in one SQLite transaction and verifies both predecessor and final file
state. Search, temporal, inspection and exploration packet comparisons execute an independent
lossless JSON comparator over actual results. Exploration retains independent
scene outputs for exactly the selected node/relation carriers and focus;
opaque continuation fields are not inputs to that pure scene function. Successful comparison results
are never treated as frozen expected packets. A missing, mismatched, exhausted, or ambiguous
record fails the test. Historical source and output provenance remain in the
metadata; local capture-storage paths are intentionally omitted.

Changing these fixtures requires a newly reviewed capture against the exact
Worker test revision. A green fixture comparison proves only that the current
Worker test result matches the recorded output; it does not revalidate current
source semantics or establish production acceptance.


The database-backed frozen cases bind the complete SQLite file bytes in addition
to the exact former program, request and result. Their fixture writer uses
SQLite **3.51.2**. Worker CI pins Node **24.14.0**, whose bundled SQLite has that
version, and checks it before running the suite. A newer SQLite writer can change
the physical file header/layout even for the same SQL rows; that does not match
these frozen inputs. This pin belongs to the historical fixture writer, not the
Worker's deployed engine or the native software package. Fixture state and result
hash checks remain mandatory; an unknown database never borrows another outcome.
