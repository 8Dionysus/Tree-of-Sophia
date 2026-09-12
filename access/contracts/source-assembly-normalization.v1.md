# Supplied source-cohort normalization

`tos_access.source_assembly_normalization.normalize_source_assembly_candidate`
normalizes an explicitly supplied, bounded source cohort. It calls the full
builder's existing normalization, bibliographic edge enrichment, Claim context,
Claim finalization, literal context, inherited-view and readable-context kernels.
It never loads a graph, discovers source membership, validates source authority,
publishes rows, writes a cache or activates a consumer.

Inputs are explicit lists:

- `node_records`: `{source_graph, record}` frames for raw `source-navigation`
  and `source-claims` carriers to replace or insert;
- `relation_records`: `{source_graph, record, identity_id?}` frames, including
  every existing/new incident relation needed by output-node finalization;
- `retained_nodes`: exact, self-consistent normalized endpoints/context
  contributors not replaced by the candidate;
- `claim_traces`: all governing raw traces, in owner encounter order;
- `context_node_order`: every supplied Claim/annotation context contributor ID,
  in original source encounter order, without omissions or duplicates;
- `source_dossier_refs`: exact known source-navigation dossier membership;
- both registries and the exact shared `normalization_binding`.

The assembler must prove **source and reducer closure**, not just graph
incidence. An Agent correction can change a Claim descriptor and all that
Claim's outgoing relation displays. Maker/evidence identities can depend on the
same Agent without any graph edge to it. A changed/deleted contribution requires
the affected node as a raw `node_record`, even if its raw bytes did not change.
Otherwise an old finalization could retain revoked context or inherited views.
New membership and deleted rows are selected separately by the source owner;
this candidate does not infer them from absent supplied rows.

Missing supplied endpoints, governing Claim traces or Claim contexts refuse the
candidate. These checks cannot prove that an unseen dependency was included.
Unknown payload fields remain intact; the operation grants no semantic or
source acceptance. Full-builder source/global validation remains separate.

`AssemblyNormalizationLimits` caps input/output JSON at 32 MiB each, output
nodes at 1024, retained nodes and relations at 2048 each, and traces at 512.
Input bytes are checked before cloning/normalization. Each final row reserves
output bytes before accumulation; these are bounded work/delivery limits, not
RSS or latency guarantees. Results are detached from inputs. Ambient build
cache state is disabled and restored, including on failure.

The result contains `nodes`, `relations`, accounting and false completeness,
publication and acceptance flags. It is not a graph or a snapshot revision.
The source assembler binds this stage's implementation alongside its other
dependencies in the private source root vector; the shared normalization
binding alone does not identify the entire source assembler.

The explicit selected-source transition, source catalog and reverse dependency
index feed this operation. The assembler then verifies its old-row/closure
conditions and publishes the resulting inserts/updates/deletes through the
source-bound prepared transaction, with source-owner guards through commit.
