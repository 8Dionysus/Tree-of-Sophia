# Supplied source-cohort normalization

The native Rust API `tos_command::source_assembly_normalization::normalize_source_assembly_candidate`
normalizes an explicitly supplied, bounded source cohort. `tos_access` re-exports
the API for native Rust consumers. The caller supplies the exact owner-selected
registries, descriptor, vocabulary and expected normalization binding. The
implementation calls the full builder's existing normalization, bibliographic
edge enrichment, Claim context, Claim finalization, literal context,
inherited-view and readable-context kernels.

This API does not discover or load a graph, find missing source membership,
validate source authority, publish rows, write or restore a cache, or activate a
consumer. It has no ambient cache state.

Inputs are explicit lists:

- `node_records`: `{source_graph, record}` frames for raw `source-navigation`
  and `source-claims` carriers to replace or insert;
- `relation_records`: `{source_graph, record, identity_id?}` frames, including
  every existing or new incident relation needed for output-node finalization;
- `retained_nodes`: exact, self-consistent normalized endpoints and context
  contributors not replaced by the candidate;
- `claim_traces`: all governing raw traces, in owner encounter order;
- `context_node_order`: every supplied Claim or annotation context contributor
  ID, in original source encounter order, without omissions or duplicates;
- `source_dossier_refs`: exact known source-navigation dossier membership;
- `normalization_binding`: the candidate's binding, checked against the
  independently selected owner binding.

Both entity and relation registry bytes, the authored vocabulary descriptor and
the expected normalization binding are selected separately from the candidate.
Caller-provided records and the owner-selected dependencies are read-only; the
result is detached from those inputs.

The assembler must prove **source and reducer closure**, not just graph
incidence. An Agent correction can change a Claim descriptor and all that
Claim's outgoing relation displays. Maker or evidence identities can depend on
the same Agent without any graph edge to it. A changed or deleted contribution
requires the affected node as a raw `node_record`, even if its raw bytes did not
change. Otherwise an old finalization could retain revoked context or inherited
views. New membership and deleted rows are selected separately by the source
owner; this candidate does not infer them from absent supplied rows.

Missing supplied endpoints, governing Claim traces or Claim contexts refuse
the candidate. These checks cannot prove that an unseen dependency was
included. Unknown payload fields remain intact; the operation grants no
semantic or source acceptance. Full-builder source and global validation
remain separate.

`AssemblyNormalizationLimits` caps input and output JSON at 32 MiB each, output
nodes at 1024, retained nodes and relations at 2048 each, and traces at 512.
The shared native kernels also enforce their per-row and readable-context
ceilings. The selected registries and descriptor are separately bounded by
the input-byte ceiling. These are bounded work and delivery limits, not RSS or
latency guarantees.

The result contains `nodes`, `relations`, accounting and false completeness,
publication and acceptance flags. It is not a graph or a snapshot revision.
The source assembler binds this stage's implementation alongside its other
dependencies in the private source root vector; the shared normalization
binding alone does not identify the entire source assembler.

The explicit selected-source transition, source catalog and reverse dependency
index feed this operation. The assembler then verifies its old-row and closure
conditions and publishes resulting inserts, updates and deletes through the
source-bound prepared transaction, with source-owner guards through commit.
