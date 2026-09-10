# Document date compact delivery and row-key binding review

## Scope

Reviewed the bounded successor to
`0f134716c0c216737c7a9187c2a8ed42701cca5f` in eight access files:

- `access/src/tos_access/knowledge.py`
- `access/deploy/cloudflare-worker/src/knowledge.ts`
- `access/deploy/cloudflare-worker/src/temporal-comparison.ts`
- `access/tests/test_temporal_comparison.py`
- `access/deploy/cloudflare-worker/test/temporal-comparison.test.ts`
- `access/contracts/knowledge-graph.v1.schema.json`
- `access/README.md`
- `access/deploy/cloudflare-worker/README.md`

The source-boundary checklist identified compact payload size and exact source
binding as the at-risk boundaries. No raw source, Claim meaning, stable ID,
HumanForm, assessment, rights, grant or temporal role changed. This review
records access mechanics, not catalogue or Foundation acceptance.

## Inspected behavior

Python and Worker compact carriers now omit only
`semantics.claim.source_canonical_json` from Claim semantics. The helper copies
the semantic containers; all other semantic fields, source refs and content
revision remain intact. Full inspection and temporal comparison retain the
exact companion, and the normalized source item is not mutated. The existing
262144-byte companion limit and v1 ABI remain unchanged.

Worker row-member selection now decodes JSON key identity while preserving
the selected value's original number tokens. Escaped names and outer JSON
whitespace remain valid; two names decoding to the selected key refuse exact
binding. This closes the case where JSON parsing selects a later escaped
duplicate but a literal-key scanner reads an earlier, numerically different
source spelling. Source hashes are still never reconstructed from JavaScript
numbers. No broader canonicalizer, storage normalizer or schema requirement
was introduced.

## Verification and review

- Python temporal module: 17 tests passed in 35.717 seconds; service peak
  133.6 MiB, zero swap.
- Worker temporal comparison, isolated D1 and Worker HTTP: 155 tests/subtests
  passed in 47.137 seconds; service peak 592.1 MiB, zero swap.
- Shared native documentary fixtures check complete Python/Worker full and
  compact packets plus source immutability. Valid escaped keys and ambiguous
  escaped duplicate keys run through direct comparison, actual D1 and HTTP.
- Existing compact assertion-context test passed in 0.010 seconds; unknown,
  false and conflict fields remain intact for historical carriers.
- TypeScript no-emit check passed in 2.266 seconds, 154.2 MiB, zero swap.
- Source-home validation and whitespace checks passed; no route card moved.
- Independent parent review read the complete Worker knowledge and temporal
  implementations, complete changed Worker test and Python/docs/schema diff;
  no remaining blocking finding. Its separate Worker run passed 155 tests in
  55.734 seconds, 493.4 MiB, zero swap; its typecheck passed in 2.061 seconds,
  145.4 MiB, zero swap.

Cloudflare and Workers best-practices guidance informed the independent full
module review and isolated D1 tests. No secret, binding, setting, remote store,
corpus import, generated corpus/KAG rebuild or deployment changed. Full release
validation, CI, merge, publication and live consumer checks were not performed.

## Next owner

Foundation integration owns the combined source commit, read-model rebuild and
current API consumer verification. The access patch and local checks do not
grant source truth, assessment, canon or deployment authority.
