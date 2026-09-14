# Native Expression / Edition growth review

Date: 2026-09-09 UTC. Scope: source work after
`ffd919acf91af38fab4b4ef6e3ba7a7e6aa4f989`, not Foundation v1 acceptance.
The Operator's existing Foundation direction authorizes this bounded source
growth. An implementation helper returned the frozen adapter; the primary
agent reviewed its source boundaries and executed the real operation. A
separate read-only reviewer found no remaining actionable implementation
finding. Neither a test nor this engineering review grants source admission.

## Referent and source inspection

The [Gutenberg catalog](https://www.gutenberg.org/ebooks/4363) and
[opening header and transcriber note](https://www.gutenberg.org/cache/epub/4363/pg4363-images.html)
were inspected for a narrow identity choice. They describe an electronic
edition and distinguish editorial adaptations from the reported underlying
Helen Zimmern translation. They are two pages of one provider, not independent
historical attestations. The full translation and every supplied electronic
format were not compared; the broad Complete Works reference did not establish
an exact historical printing.

The new [Edition](../source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/en-zimmern-gutenberg-4363/editions/project-gutenberg-4363/edition.json)
therefore identifies the Project Gutenberg electronic editorial manifestation
numbered 4363. Its label, heading and source note retain that limitation.
It is provisional with `no_equivalence_claim`, unverified external identity
and no publication, provision, responsibility or exemplar claims. Provider
dates were not converted into Work, translation or print-publication dates.
No Item, File, payload, fidelity, equivalence or rights assertion is introduced.

The separate [embodied_by Claim](../source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/en-zimmern-gutenberg-4363/editions/project-gutenberg-4363/source-claims.jsonl)
observes only the declared metadata link. Its entire Russian statement,
source record, language, script, evidence and `unreviewed` state remain
available. Its two local evidence paths bind metadata, not additional
historical observations. Serialization provenance is not an assessment.

## Boundary checklist

- **Yes — source return, layer separation and plurality.** Expression,
  Edition, Claim, human form and transaction are distinct identities.
  One selected Expression is this command's bounded write set, not global
  bibliographic cardinality. Existing multi-Expression and collection Editions
  remain valid, and an Edition need not have an Item.
- **Yes — lineage and restricted changes.** Only the selected Expression's
  record version, appended embodiment Claim ref, explicitly rebound forms and
  history change. Its previous notes and translator responsibility remain
  exact. The new Claim has its own Edition home; the initial Work/Expression
  and translator streams and their captured receipts are immutable.
- **Yes — authority and recovery.** An exact protected expiring grant selects
  Work, Expression, new Edition, Claim, event and form IDs. The common
  selected-metadata transaction owns pending visibility and exact-byte
  recovery. Generic Edition grants and standalone `embodied_by` Claim writes
  remain closed. Read support cannot manufacture write permission.
- **Yes — meaning and language.** RU and EN names are explicitly supplied
  source-copy forms. Ready form materialization does not accept their
  linguistic or bibliographic truth. Qualification and source language stay
  attached; no translated canonical tree is created.
- **Yes — stronger owners.** Public metadata visibility is not payload
  availability, rights clearance, consent, publication or canon. No runtime,
  KAG, proof, memory or external authority moves into this operation.
- **Not applicable.** No counterpart, compost, calibration, lived witness,
  philosophical template, gold promotion or concept merge/split occurs.

## Real operation and preservation

`expression.edition.create` completed in 3.012 seconds, peak 33.2 MiB,
zero swap. The [committed receipt](../source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/en-zimmern-gutenberg-4363/editions/project-gutenberg-4363/expression-edition-receipt.json)
binds the request, captured environment, predecessor and seven explicit forms.
Transaction:
`sha256:2f5d323a222a6134651470076ee53740956c5a73295aca3e1796da190913388b`.
Expression v2 advanced to v3, and Edition v1 was created. The ready committed
publication is generation 6, token
`sha256:2b9002e73551ce1b68473a677af204514d4d123a0f09742c1a2bb90925101b82`.

A fresh process verified all three independent compound receipts:
Work/Expression creation, translator attachment and Expression/Edition creation.
The metadata reader resolved Expression v1, v2 and v3 by exact canonical refs
and by original logical path plus raw SHA-256, and resolved Edition v1 both
ways. Ten retained Work, Claim, receipt, provenance and legacy topology files
kept their before-operation raw hashes. All seven current forms materialized
as ready, with source qualifications and full Claim context retained.

The same process retried the exact captured creation request through the CLI
under the current grant. It returned `replayed: true` with the same transaction;
the publication token did not advance. That end-to-end canary passed in
3.104 seconds, peak 49.7 MiB, zero swap. Three earlier harness runs reached
the successful retry but failed on a nonexistent reporting field; they are not
reported as successful end-to-end runs. No production contract was changed to
satisfy that harness error.

## Implementation and derived checks

The focused synthetic regression passed 137 test methods and 1,018 subtests
in 856.349 seconds, peak 160.8 MiB, 15.1 MiB swap. It covers exact grants,
create/replay/conflict, interrupted commit and rollback, retained predecessors,
catalog closure and refusal of standalone topology writes. Independent review
read the 572-line adapter, pure topology delta, shared lifecycle, retained
reader and write guards. It did not independently repeat the real source choice
or grant execution; those remain the primary agent's evidence above.

The source catalog, bibliographic graph and corpus index were regenerated
in owner order in 56.594 seconds, peak 553.1 MiB, zero swap. Afterward, source
foundation validation, catalog parity, bibliographic graph parity and graph
validation all passed together in 66.416 seconds, peak 194.7 MiB, zero swap.
The source-foundation result checks present optional payloads, not availability
or completeness of private bytes. The access-contract regression passed all
44 tests in 186.456 seconds, peak 1.3 GiB, zero swap; the earlier stale
Goodwin-Edition navigation mismatch disappeared after owner regeneration.

Reproducible focused commands:

```sh
python -m pytest mechanics/growth-cycle/tests/test_source_edition_commands.py
python -m pytest mechanics/growth-cycle/tests/test_metadata_version_reader.py
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_catalog.py --check
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
```

Grant-free discovery now exposes 25 connected handlers and 27 owner-schema
tags. The adapter's grammar and source profile handles remain the execution
authority; no parallel command catalog was introduced. Item/File adoption,
non-source-copy parent forms and other compound growth are separate remaining
work. Local checks do not assert CI, merge, release, deployment, source
admission or completion of Foundation v1; integration retains its own owner.
