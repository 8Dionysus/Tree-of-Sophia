# Bibliographic Topology Claims

This subtree owns the three evidence-bearing directions of the current corpus
identity ladder:

```text
work-expression/       Work       --has_expression--> Expression
expression-edition/    Expression --embodied_by-----> Edition
edition-item/           Edition    --exemplified_by--> Item
```

Each direction has its own source claim file. The identity records retain the
declared structural links and close over the exact outgoing claim IDs through
`expression_claim_refs`, `embodiment_claim_refs`, or
`exemplar_claim_refs`. Validation requires both representations to agree
exactly: a missing, extra, cross-typed, duplicated, or misbound relation fails.

`provenance.jsonl` digest-binds all three claim files and every record or item
manifest used as evidence. All current rows are model-made,
`public_metadata_only`, and `unreviewed`. They materialize declared
bibliographic topology only. In particular, `embodied_by` does not assert
textual identity, critical-edition equivalence, accepted text, translation
quality, rights clearance, semantic relation, graph truth, or canon.
