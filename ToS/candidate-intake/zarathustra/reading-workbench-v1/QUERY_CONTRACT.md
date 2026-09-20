# Reading query contract

`scripts/query_zarathustra_reading_workbench_v1.py` is the source-owned query
provider. `ToSAccessCore.zarathustra_reading_search` exposes the same result to
local CLI, HTTP and native MCP. They do not
implement separate search engines.

The provider invokes the concept query from the same software installation.
It validates the supported v1 data manifest, schema binding and input/private
artifact digests. Build-time implementation hashes remain provenance;
compatible software upgrades do not require rewriting the immutable dataset.
The selected source and analysis roots are explicit and required, with no
fallback to another checkout. Both roots are bound by the checked concept
index digest. Local access adapters pass one complete selected dataset as both
roots. Browser/WebMCP and public Worker integration are not part of this port.

## Scope and evidence

- All concept-request matches are read and enriched before `limit` is applied.
  Group counts refer to that full set; each group's `returned_occurrence_refs`
  lists only the result cards present in the response. `limit=0` is a counts
  and grouping request, not an empty-corpus claim.
- Exact historical source text is returned only from local private material.
  Each offset is context-local, Unicode-codepoint and half-open. Text slices
  and SHA-256 are verified at query time.
- The selected speaker comes from the segment containing the precise source
  occurrence. Missing, crossing or multiply resolved spans remain unresolved;
  the earlier paragraph role is retained as `speaker_predecessor`, never used
  as an implicit fallback.
- `utterer_role`, `performed_role` and `modality` remain distinct. A performed
  persona or imagined quotation is not flattened into the physical speaker;
  speaker groups retain those distinctions as separate group keys.
- `formula_memberships` contains formulas spanning that occurrence.
  `context_formula_memberships` is explicitly weaker: the formula is only in
  the same context. Formula groups count the former, not the latter. One hit
  may belong to multiple exact-normalized formulas; counts are not exclusive.
- Sentence/clause correspondences retain their status and competitors. Neither
  paragraph nor sentence correspondence proves word translation. The Russian
  historical text remains a comparator, while German remains source authority.
- Existing English on-demand task refs receive the new reading context.
  `execution_status=not_executed` is intentional: this query does not execute a
  translator or invent etymological evidence. An agent uses the exact source,
  cites external lexical evidence when needed, and returns candidate analysis.
- Matching coverage is coverage of the registered concept request. It is not
  proof that every implicit semantic mention in the book has been discovered.
- `additional_source_candidates` independently scans the German source for
  an explicit `¬` followed by a line break inside a word. Removal of that
  marked boundary must match an already selected German form. Ordinary hyphens
  and unmarked line breaks are never joined by this method. The exact original
  range and each normalization operation survive; the candidate has its own ID
  and `source_existing_occurrence_ref:null`. Counts and returned additions are
  separate from the predecessor result groups. This catches a technical gap
  without pretending the legacy occurrence index was already complete.

## Capability boundary

Missing installed provider, manifest or private source material returns `available:false`,
`result:null`, `task:null`, `publication_posture:excluded_from_public_bundle`.
Corrupt or stale material fails its integrity check; no stale paragraph result
is silently substituted. The source-bearing provider and database are not
added to the public standalone allowlist. The public capability is always
unavailable and does not load the provider.

Focused tests protect occurrence/segment attachment, source-anchor fixity,
pre-limit counts, context-versus-occurrence formula distinction, and the
transport-neutral capability. These checks do not accept speaker judgment,
historical semantics, translation or canon.
