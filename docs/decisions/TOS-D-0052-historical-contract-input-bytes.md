# Preserve historical schema inputs without replacing current contracts

## Index Metadata

- Decision ID: TOS-D-0052
- Original date: 2026-09-07
- Surface classes: contracts, validation
- ToS layers: doctrine, contracts, source-witnesses
- Tree classes: knowledge foundation, provenance
- Guard families: exact-byte provenance, source-first authority
- Posture: accepted

## Context

The Foundation v1 language extension changed the active Corpus schema while
earlier discovery/provenance events retained the original schema digest.
The source-foundation validator compared these historical inputs only with
today's schema, producing 19 responsibility-Claim diagnostics. The recorded
digest matches preserved Git bytes; the old events do not claim that their
input is the current contract. Current source records still require current
validation independently of the historical input evidence.

## Decision

Keep exact needed prior schema bytes in `ToS/contracts/history/<sha256>.json`.
This is content-addressed source history, not another active schema registry.
A recorded `ToS/contracts/*.schema.json` input may resolve to that history
only after the current named schema exists and the retained bytes match both
the recorded digest and original ToS `$id`. Limit each snapshot to 1 MiB and
refuse symlinks, malformed inputs and path substitution.

The existing source-foundation validator uses this route for recorded
responsibility, chronology and expression-derivation inputs. It continues to
check current object/Claim schemas, reference closure and output bytes against
their active sources. Non-schema evidence and source input paths do not gain
an archive fallback. Retained bytes prove availability, not execution truth,
current compatibility, source truth or admission.

## Options considered

- Restamp old provenance with the new digest: rejected; fabricates an input
  that the recorded operation did not consume.
- Ignore schema-input drift: rejected; loses the exact input-byte check.
- Read arbitrary Git history during validation: rejected; shallow clones,
  source archives and standalone packages need an explicit portable input.
- Introduce a general artifact/history registry: deferred; the present owner
  need is exact public schema bytes and has a deterministic locator.
- Retain required schema bytes beside their owner with no active registration:
  chosen; preserves history while allowing the source contract to evolve.

## Consequences and boundaries

The additive snapshots travel in source archives and remain immutable.
Publishing them does not change the interpretation or status of old events;
no event, authority, maker or original timestamp is rewritten. A missing active
schema is not repaired by an old copy, and an unknown or incompatible current
schema version still fails its current consumer. New historical snapshots
require their own exact-byte/source review; copying an arbitrary old file is
not an implicit compatibility decision.

This is not a solution for general record history, private payload retention,
model/runtime archives or deployment. Those keep their existing owners.
Rollback of the reader may restore the earlier drift failure; deleting the
retained bytes would break historical resolution and is not ordinary cleanup.

## Source surfaces

- `ToS/doctrine/CORPUS_FOUNDATION.md`
- `ToS/contracts/history/README.md`
- `scripts/validate_source_witness_foundation.py`
- `tests/test_source_witness_foundation.py`

## Validation

Focused controls distinguish current and retained bytes, wrong paths/IDs,
corruption, non-schema inputs, missing active contracts, symlinks and byte
budgets. Full source-foundation validation checks the actual historical
events without rewriting them. Decision indexes remain derived lookup.
