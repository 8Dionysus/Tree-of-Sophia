# Agon

<a id="active-route"></a>
<a id="historical-provenance"></a>

Agon routes threshold pressure as reviewable candidates toward ToS source and
canon review. The center Agon law belongs to `Agents-of-Abyss`.

## Operations

### Threshold intake and canon restraint

Receive an Agon/KAG threshold packet, candidate scope and source boundary note.
Route the candidate to restraint or ToS review, owner evidence, return, deferral,
quarantine or an authorial request. The review boundary distinguishes intake, review,
rejection, deferral, branch candidates and canon drafts. Rejection,
defer-with-cost and branch candidates are first-class routes.

`Sophian threshold candidate != Tree-of-Sophia canon`. Authored, slow and explicit canon
follows ToS-owned source review and promotion. The intake schema is
`parts/threshold-intake/schemas/tos-agon-threshold-intake.schema.json`.

### Threshold registry

The [registry](parts/threshold-registry/README.md) maintains candidate-only
entries, its config, schema, example and generated companion. Build and validate
with:

```sh
tos-ops-mechanics-plan --repo-root ROOT --threshold-registry-build
tos-ops-mechanics-plan --repo-root ROOT --threshold-registry-validate
tos-ops-mechanics-plan --repo-root ABS --local-contracts mechanics/agon/parts/threshold-registry
tos-ops-mechanics-plan --repo-root ROOT --execute
```

The builder accepts `--check` to compare its current output. Retained assertions
are in `rust/crates/tos-ops-mechanics-plan/tests/mechanics_contracts.rs`.

### Landing handoff

Carry the landing note, center pressure and ToS boundary into a handoff receipt
and next route: center Agon law to `Agents-of-Abyss`, authored meaning to ToS.

## History

[TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md)
and [prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/agon/PROVENANCE.md)
record earlier routes. When historical material changes current behavior, update
the owning operation and record an immutable history reference.

## Growth conditions

Refine the threshold registry or add a narrower part when repeated pressure
outgrows the existing operations. Route Table I and graph-review proposals
through candidate and source review.
