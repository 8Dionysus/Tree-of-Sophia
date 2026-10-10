# Indexed knowledge search v2 source registration

`knowledge-search-indexed.v2.schema.json` describes the existing
`tos_knowledge_search_indexed_v2` response envelope and complete normalized
node/relation carriers. It is separate from `knowledge-graph.v1.schema.json`:
graph v1 and its lens/exploration consumers retain their seven-source enum
and existing packet meaning. Existing seven-source indexed v2 packets remain
structurally valid under the new response schema.

The response schema admits a nonempty `source_graph` string in a selected
indexed packet. **That structural check is insufficient to admit a source.**
At both compilation and cold/warm read-model opening, the native producer
and reader must verify every node, relation, filter source and search posting
against the exact source-owned `tos_knowledge_query_vocabulary_v1` descriptor
selected with that publication. Bind its descriptor digest, registry roots,
source cut, graph/catalog roots, compiler/index generation and execution
profile in the immutable selected publication. A missing, duplicate,
unregistered, retired, or unsupported source/adapter refuses the candidate;
it cannot become an empty search result. Queries validate `sources` against
that same selected descriptor. No Rust or transport allowlist of today's
seven source IDs is an authority.

This indexed response contract does not change the direct API's default
legacy search mode, native Rust/Worker indexed cursor formats, graph v1 packets,
LensSpec v1, exploration v1/v2, or protected exact-source access. An eighth
source may pass this *structural* schema after owner registration, but public
compatibility remains gated on the full CMP/QRY selected registry closure,
exact packet differential, and each consumer adapter's migration. In
particular, the existing native Rust and Worker seven-source readers do not
acquire eighth-source support from this schema file.

The Rust query tests exercise indexed packets against their selected query
vocabulary. Those checks are deliberately structural at this contract layer;
CMP/QRY owner tests must prove selected descriptor membership and complete
indexed publication at compile/open.
