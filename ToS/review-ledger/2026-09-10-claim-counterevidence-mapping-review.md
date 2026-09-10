# Claim counterevidence mapping — 2026-09-10

Manual source and boundary review against exact baseline
`8761045f0c32ebdb0f89cfae0f5f4f5e70c30453`.

The existing bibliographic producer reads each Claim's `counterevidence_refs`
in `scripts/source_witness_bibliographic_graph_common.py::_build_payload`
and emits `counterevidenced_by` through its unchanged `_edge` helper. The
existing graph schema already declares that edge kind. The missing surface
was its semantic registry mapping, not a missing Claim, producer or schema.

Relation registry 41 adds `tos.relation.claim-counterevidenced-by`, narrowly
scoped to that source-claims edge, with Claim domain, Evidence range and
explicit inverse navigation. It remains in the reified Claim structure;
no `source_claim_profile` or writable predicate is added. Counterevidence is
optional and does not prove that a search was performed when absent. The
[source law](../doctrine/semantic-interchange/README.md#semantic-boundaries)
owns its meaning, not a zero-unmapped target or a derived display label.

The real `tos.claim.jgb21-conception-inversion` cites the same
[reading note](2026-09-09-jgb21-inversion-source-reading.md) in both evidence
roles. Its qualifiers explain that the counterevidence limits an overreading,
not the qualified inversion Claim. The distinct support and counterevidence
edges therefore share an Evidence endpoint while retaining separate native
IDs and the complete Claim context. Neither role proves falsity, settles an
objection, performs assessment or grants admission.

Applicable manual checklist answers are **yes** for exact source return,
authored/derived distinction, qualified context, stable IDs, explicit review
state, contestability and owner boundaries. Registry RU/EN labels are derived
navigation, not authored witness translations. New source intake, identity
migration, source-language judgment, rights, canon, lived witness and broader
philosophical growth are **not applicable**. Existing source records, evidence
bytes, qualifiers, Claim and edge IDs, assessment and entity registry are
unchanged. The five prepared-atlas unknown mappings retain the posture in the
[atlas review](2026-09-09-philosophy-atlas-source-return-review.md); no fallback
is broadened to absorb them.

The one-time actual-source canary passed **one test and seven subtests** in
26.10 seconds: fresh producer without generated writes, exact source and
counterevidence context, distinct dual roles, incoming RU/EN compact/full
delivery, optional counterevidence, no new writer, foreign-scope fallback,
and domain/range/mapping-scope rejection. Peak memory was 266.4 MiB with no
swap. This remains bounded inspection evidence of that source snapshot, not a
permanent constraint on the historical Claim's interpretation.

The retained regression test uses two synthetic Claims, one with and one
without counterevidence, a shared Evidence, and Work/Agent endpoints. The
producer's existing `_claim_node` and `_edge` helpers create the carriers;
semantic validation checks their complete normalized Claim contracts. It
preserves the dual-role, source-context, incoming RU/EN compact/full,
optional-role, no-new-writer, foreign-scope fallback and invalid
domain/range/mapping-scope checks without rebuilding the whole corpus or
fixing a historical Claim's evidence order. This test passed **one test and
seven subtests** in **0.388 seconds**, with **31.9 MiB** peak memory and no
swap. It is synthetic contract evidence, not a corpus audit.

The resumed owner and root reviewer inspected the final four-file diff.
Exact-baseline registry transition, source-home lane (including the private
lived-witness structural boundary), and diff checks passed again. The two
validator lanes peaked at 34.3 MiB with no swap. No generated companion or
producer/schema change is included in this source handoff.

Final generated documentation, graph and KAG currentness, union coverage,
full release checks, CI, merge and runtime delivery remain with the integration
owner. These bounded checks and this source mapping are not semantic or
publication acceptance.
