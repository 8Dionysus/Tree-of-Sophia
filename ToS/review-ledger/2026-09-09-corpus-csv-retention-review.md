# Canon and intake field-retention review

## Source and changed boundary

This Foundation M01 increment preserves the complete parsed records of the two
current `edges.csv` sources in the corpus index and ordinary normalized reader.
Previously the builder selected relation IDs, endpoints, predicate, layer and
status but omitted other authored columns, including anchor scope, confidence
and notes. The source CSV files were intact; the loss occurred in their derived
carrier. No CSV, canonical node, identity, direction, status or admission decision
was edited in this change.

`scripts/tos_corpus_index_common.py` now adds `properties.source_record`,
`source_row` and `source_file_sha256` under the matching optional
`relationEdge.properties` contract in `ToS/contracts/tos-corpus-index.schema.json`.
The full row retains unknown columns and distinguishes null missing cells from
empty strings. The ordinal counts data records, not physical lines; quoted
multiline cells remain intact. The digest binds exactly the bytes parsed,
including line endings. Duplicate or empty column names, unnamed surplus cells
and malformed quoting produce an error instead of a partial relation pack.
A post-parse source-byte change refuses publication.

The shared reader already carries these fields into relation `attributes` and
returns the exact source path through its relation pack. This is source-field
retention, not a new universal Claim, native record identity, interpretation of
confidence or acceptance of intake. Existing normalizer and endpoint semantics
are unchanged. Older snapshots remain schema-valid without this optional
binding; a new index must be shipped with its matching schema, never silently
validated against an older schema that rejects the new field. Rolling back the
derived reader/index cannot delete the unchanged authored sources.

## Observed current corpus

The [reader observation](2026-09-09-corpus-csv-reader-observation.json) records
the exact source revision and source hashes at the inspected snapshot, based
on parent source commit `010084d604890b4cdcb6e2ecb8d57081bef607ab` plus this change.
It checks all 128 intake and 125 canonical CSV records against
`ToSAccessCore.knowledge_graph()`: exact raw row, original ID, logical ordinal,
source digest, return path and unchanged status. All 92 tracked `node.json`
records also match the reader's complete source payload and exact file hash.
No node change was necessary; this confirms their existing carrier.

The cold reader check took 22.245 seconds and peaked at 1.4 GiB with zero swap
under host resource admission. This is a whole-reader correctness observation,
not a compact query latency budget. The corpus builder plus all 15 focused
tests and 21 subtests passed in 31.17 seconds, with a 938.1 MiB unit peak and
zero swap. The tests include synthetic unknown/multiline/null cases, malformed
CSV refusal, source mutation, legacy schema compatibility, and enumeration of
all actual CSV rows and canonical nodes. The full-reader check independently
read the raw CSV/node files rather than trusting the builder's selected rows.

A repeat of `python scripts/source_witness_projection_coverage.py --rows`
at this rebuilt corpus reached all 665 public catalog identities. The stale
Collection carrier now agrees with its source: 632 direct mappings, 28 native
adapters and five legacy Link Claims still requiring clarification on this
branch. Their separately reviewed adapter belongs to the integration branch;
this observation does not pretend it is already present here. Source catalog
membership and the aggregate digest of its 390 source files are unchanged.

## Review and residual scope

Source traceability, stable identities, source versus derived layering,
candidate versus canon, plurality and the ToS ownership boundary pass the
manual review. The retained fields expose previously omitted context without
turning it into stronger knowledge. Assessment, language quality, permissions,
historical versions and actual canon admission retain their separate owners.
No new decision, memo, candidate queue or authority registry is needed.

The whole corpus index was regenerated through its existing builder. Its
source-navigation refresh is generated currentness, not a source edit. Final
documentation and route companions, focused topology and source-home checks
are recorded with the exact commit checkpoint review. Cross-corpus KAG sealing,
the separately changing atlas, combined CI, merge and deployment remain with
their existing integration owners. This bounded observation does not close
all of M01 or the Foundation goal.
