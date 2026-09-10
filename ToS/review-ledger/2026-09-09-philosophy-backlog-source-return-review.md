# Philosophy backlog source-return review

Date: 2026-09-09 UTC. Source baseline:
`801513a02218c0b02bdc375e40fc69d43a464ac5`.
This extends that baseline's explicitly bounded atlas return; it does not
retroactively enlarge the earlier review's claims.

## Source-first inventory and identity

The three source families are declared by
`ToS/philosophy/atlas/dossiers/branch.manifest.json`. Their complete current
inventory was inspected before selecting a projection seam:

| Family under `ToS/philosophy/atlas/dossiers/` | Records | Dossiers |
| --- | ---: | ---: |
| `source-anchor-backlog.jsonl` | 9530 | 189 |
| `term-index.jsonl` | 4743 | 188 |
| `transmission-backlog.jsonl` | 3326 | 189 |

The exact source-file SHA-256 values are respectively:

- `d7e1a5d558d7073a38378bef5833b368fe39d156f645c073de253a91d847564f`
- `b84cb317d4fe5c41c2051339b8aefa68ef2c0099d05792b1f65f24a2f67551ef`
- `b312e9db0f771eb786256413c02ed3f42341ce3a819887a63564b5b5653a4de3`

There are 17599 records and 15636669 source bytes. Every record has its exact
family `source_ref`, a matching existing dossier and atlas-row ID, original
source-document/table/row coordinates, and the declared branch route. Those
coordinates are unique within each current family, but are not an authored
stable identity contract. None of the families declares global record IDs.
Only 1852 anchor records carry `source_local_id`; the 173 distinct strings are
unique only after dossier scoping. They remain unchanged source fields.

Anchor kinds remain 3189 `control_or_review_anchor`, 3795
`corpus_or_edition_anchor` and 2546 `risk_control_source_need`. Transmission
direction remains the recorded incoming/outgoing value. No new philosophical
type is inferred from a term, source lead, risk-control requirement or channel.
`T3-43` has no term records; `T3-57` has none of the three backlogs.

## Existing mappings versus the actual gap

All 9530 anchor objects already equal the union of 190 branch-local anchor
mirrors under canonical JSON comparison. Source bytes were not erased. These
mirrors are not additional source records and are not counted twice.

The reviewed discovery candidate ledger has 18 anchor source refs selecting
14 unique exact rows. Each selector resolves to one actual source row. These
are reviewed leads with explicit evidence ceilings, not full raw-record
delivery or acceptance of all other backlog entries. The discovery snapshot
also binds the anchor file digest and count; it does not carry its full body.

The earlier atlas/graph ordinary reader returned dossier counts and source
paths, but no complete record-level content from any of these three streams.
Thus the delivery gap was all 17599 raw records, not a new source, identity or
semantic-classification requirement. Existing branch mirrors and the 18
reviewed refs are unchanged and retain their separate authority limits.

## Compatible return and manual boundary review

The existing `atlas-dossier:{dossier_id}` node now carries
`properties.source_backlogs`. Its three family fields use the existing dossier
manifest names: `source_anchor_backlog`, `term_index`, `transmission_backlog`.
Each returns the exact `source_ref`, `source_file_sha256`, `record_count` and
`records`. Each record has the complete original parsed `source_record`, exact
`source_record_ref`, separate file and canonical-record digests, JSONL
`source_row` and physical `source_line`. Original DOCX coordinates remain raw
fields; they are not relabelled as JSONL addresses.

- Traceability and preservation: all raw members, source-local IDs, route
  constraints, status, confidence, limitations, unresolved source needs,
  language, transliteration, transmission notes and unknown nested fields
  survive unchanged. Missing, null, false and empty values remain distinct.
  Identical records on different lines remain different source occurrences.
- Addressability without new identity: select the existing dossier and the
  family/source-file snapshot plus record/line locator. This creates no graph
  node, relation, corpus identity, source assertion, semantic registry entry,
  translation, source anchor or canon admission.
- Explicit absence: all three family objects remain present on every dossier,
  including their source ref/digest and empty `records` arrays. A missing file,
  aliased family ref, source-ref mismatch, unmatched dossier/document/branch,
  changed source bytes or dossier-count mismatch fails instead of dropping
  rows or manufacturing an empty result. Portable checks also reject changed
  raw bodies, digests, repeated/out-of-order locators and substituted parents.
- Source authority: the aggregate files selected by the existing authored
  dossier manifest supply the return. The builder binds and rechecks that
  manifest and those exact bytes. It creates no competing source registry.
  Discovery lead review, source-visible assessment, rights, publication and
  canon remain with their owners. Raw carriage is not their acceptance.
- Consumer and topology scope: both philosophy-node and normalized
  knowledge-node inspection retain the nested raw arrays. Existing full-node
  inspection has no raw-attribute byte cap; `relation_limit` only bounds
  adjacent relations. The 16 KiB human-Form selection budget is a different
  contract and is not bypassed or enlarged. Existing graph IDs, source-owned
  view membership, clusters and node/edge counts remain unchanged; content
  fingerprints appropriately bind the additional returned source records.

Golden-kernel transfer, new branch planting, lived witness, calibration,
compost, multilingual canon, counterpart alignment and runtime ownership do
not apply. No broader durable decision is introduced.

## Verification and remaining owner

Focused checks cover lossless unknown/missing values, duplicate unkeyed
occurrences, physical lines versus record ordinals, explicit empty families,
missing files, ambiguous refs, source drift and portable substitution
negatives. Full source parity compares every actual record and its two digests
with exactly one occurrence under its existing dossier. The ordinary-reader
canary inspects normal, large, partly empty and wholly empty dossiers through
the real core methods and existing semantic normalizer; it does not rebuild
unrelated corpus adapters or certify live API/UI behavior.

The atlas, graph-view and graph-projection canonical validators and the
unchanged post-planting audit passed. Both complete focused test files passed
51 tests and 19 subtests. The resource-gated validation unit completed in
210.8 seconds with approximately 1.2 GiB peak memory and no swap. The separate
source-home check also passed. The two projection builders and view builder
completed under their own normal admission; their peak was 830.4 MiB with
6.4 MiB swap. No resource gate was forced or bypassed.

Independent comparison against the exact baseline found identical node/edge
IDs, view membership, cluster membership and graph counts. All 17599 records
are returned; the largest serialized nested backlog family collection is
201967 UTF-8 JSON bytes for `atlas-dossier:T3-51` (144 records). These are
full raw inspection bytes, not a Form-selection budget override. Authored
philosophy sources, branch mirrors, discovery refs and their statuses have
no diff against the baseline.

The integration master retains final whole-source M01 assembly, corpus/KAG
and artifact companion sealing, and landing. The remaining semantic mapping
and source-assessment questions are preserved in source, not solved by this
raw return. Full release, CI, merge, publication and runtime acceptance are
outside this bounded local change.
