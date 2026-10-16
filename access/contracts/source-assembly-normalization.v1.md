# Supplied source-cohort normalization

The internal `source_claim_publication_normalize::normalize_claim_candidate`
kernel normalizes an explicitly supplied, bounded source cohort. Claim, Agent
and metadata publication call this kernel directly. The unused public
`source_assembly_normalization` wrapper and its `tos_access` re-export have been
retired; normalization is not an installed SDK operation.

The publication owner supplies exact registries, descriptor, vocabulary and
expected normalization binding. The kernel uses the full builder's existing
normalization, bibliographic edge enrichment, Claim context, Claim finalization,
literal context, inherited-view and readable-context kernels. Source selection,
admission and publication remain with its callers.

The internal input contains explicit lists:

- `nodes`: exact `SeekRow` frames and owner dossier handles for raw
  `source-navigation` and `source-claims` carriers to replace or insert;
- `relations`: exact `SeekRow` frames and optional relation identity IDs, including
  every existing or new incident relation needed for output-node finalization;
- `retained_nodes`: exact normalized endpoints and context contributors not
  replaced by the candidate, with their ordered owner material;
- `traces`: all governing raw traces, in owner encounter order;
- `context_node_order`: every supplied Claim or annotation context contributor
  ID, in original source encounter order, without omissions or duplicates;
- `dossier_refs`: exact known source-navigation dossier membership;
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
requires the affected node as a raw node frame, even if its raw bytes did not
change. Otherwise an old finalization could retain revoked context or inherited
views. New membership and deleted rows are selected separately by the source
owner; this candidate does not infer them from absent supplied rows.

Missing supplied endpoints, governing Claim traces or Claim contexts refuse
the candidate. These checks cannot prove that an unseen dependency was
included. Unknown payload fields remain intact; the operation grants no
semantic or source acceptance. Full-builder source and global validation
remain separate.

`ClaimCandidateLimits` requires the caller to supply explicit row-count,
context, row-byte, input-byte and output-byte limits. Input accounting includes
selected registry and descriptor bytes. The shared kernels enforce their own
per-row and readable-context ceilings. These are bounded work and delivery
limits, not RSS or latency guarantees.

The result contains normalized nodes, relations and input/output byte counts.
It is a candidate, not a graph revision or an admission/publication receipt.
The publication owner binds this implementation alongside its other dependencies
in the private source root vector; the shared normalization binding alone does
not identify the entire assembler.

The selected source transition, catalog and dependency closure feed the kernel.
The assembler verifies its old-row and closure conditions and publishes resulting
inserts, updates and deletes through the source-bound prepared transaction, with
source-owner guards through commit.
