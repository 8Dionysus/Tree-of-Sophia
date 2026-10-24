# Bibliographic Topology Claims

This subtree retains the legacy streams for three evidence-bearing directions
of the current corpus identity ladder:

```text
work-expression/       Work       --has_expression--> Expression
expression-edition/    Expression --embodied_by-----> Edition
edition-item/           Edition    --exemplified_by--> Item
```

Each legacy direction has its own source claim file. Native compound growth
instead retains `source-claims.jsonl` in the new Expression (`has_expression`),
Edition (`embodied_by`) or Item (`exemplified_by`) home, alongside the exact
compound receipt and source capture. See the [source-witness topology and
creation routes](../README.md#speaking-topology). These are two supported
carriers, not instructions to move native Claims into a legacy batch.

The identity records retain the
declared structural links and close over the exact outgoing claim IDs through
`expression_claim_refs`, `embodiment_claim_refs`, or
`exemplar_claim_refs`. Validation requires both representations to agree
exactly: a missing, extra, cross-typed, duplicated, or misbound relation fails.

The legacy `provenance.jsonl` digest-binds its three claim files and every record
or item manifest used as evidence. Native carriers retain their separate verified
compound publication and current/history evidence; legacy batch provenance is
not a receipt for a later native append. The retained legacy rows are model-made,
`public_metadata_only`, and `unreviewed`. They materialize declared
bibliographic topology only. In particular, `embodied_by` does not assert
textual identity, critical-edition equivalence, accepted text, translation
quality, rights clearance, semantic relation, graph truth, or canon.
