# Native Collection growth

A Collection identifies a multi-work publication and carries its membership
assertions in the corpus.

## Independent boundaries

`tos_local_corpus_create_owner_v2` adds initial `collection` to the existing
standalone Corpus creator. Version 1 is unchanged. Creation requires one new
canonical `collections/.../collection.json`, provisional identity, no
equivalence, version 1 and an explicitly empty `membership_claim_refs`.
The empty list says that no membership assertion was supplied to this
transaction; it does not assert that the real collection has no members.

`tos_local_collection_membership_owner_v1` delegates
`collection.work.attach` and separately scoped recovery. The request binds
one exact existing Collection, one unchanged existing Work and one new
`contains_work` relation Claim under a separate relation home. It appends only
that Claim reference, advances the Collection once, rebinds its source-copy
forms and retains the exact selected predecessor/history. Shared selected
metadata publication owns atomicity, pending barriers and recovery, not
membership meaning.

The grant fixes both IDs and source paths, predicate, new Claim/home/event,
principal, form IDs, evidence allowlist and expiry. Retained legacy membership
provenance streams must be selected explicitly when the parent already contains
legacy membership assertions. Native current closure is verified against its
committed compound capture; old batch bytes are never rewritten.

## Qualified membership assertion

The source caller supplies the membership statement, statement
language/script, `membership_scope`, evidence and uncertainty. Competing
Claims may name the same Work while keeping distinct identities. The operation
binds metadata endpoints and declared evidence references; the new Claim
remains unreviewed. Source-visible assessment evaluates the evidence,
completeness and membership assertion. Canon, equivalence, translation and
rights retain their owner routes.

Only the compound handler may introduce the parent membership relation.
Separately delegated Claim corrections preserve its exact compound origin and
continuous history; qualified wording can change without silently retargeting
the endpoints or deleting the membership qualification. Both current and
historical Collection versions remain available to exact metadata readers.

## Verification

Run `mechanics/growth-cycle/tests/test_source_collection_commands.py`, the
Collection creation cases in `test_source_commands.py`, metadata reader and
discovery tests, then source-foundation and registry checks when their owner
inputs change. Synthetic fixtures test mechanisms, never historical membership.
