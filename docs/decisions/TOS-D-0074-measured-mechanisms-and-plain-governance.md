# TOS-D-0074 Measured Mechanisms and Plain Governance

## Index Metadata

- Decision ID: TOS-D-0074
- Original date: 2026-11-02
- Surface classes: architecture, docs/route-law, github/checks
- ToS layers: source-witnesses, access, docs
- Tree classes: corpus
- Guard families: identity, measurement, owner boundary
- Posture: accepted by explicit operator instruction

## Context

The target in [TOS-D-0069](TOS-D-0069-rust-execution-foundation.md) is billions of source records and hundreds of concurrent agent clients. Durable architecture choices need to support that scale while implementation mechanisms follow evidence from real capacity requirements. Repository governance also needs a direct, maintainable form.

## Decision

Design expensive-to-change choices for the target now: stable identity, canonical bytes, the data model, one authoritative store per data class, and the agent API. Authored data lives in Git; records, revisions and the change queue live in PostgreSQL under [TOS-D-0071](TOS-D-0071-transactional-source-and-independent-projections.md); source bytes live in one immutable store; exports use SQLite and D1. The D1 publisher was later retired under `retired/d1-publication`; local SQLite publication remains.

Choose mechanisms, including budgets, isolation, custom engines and load machinery, after measurement against a target capacity profile with numerical objectives. Operational limits live in deployment profile data. Every transitional implementation ships with an explicit removal condition.

Repository governance is plain documentation. CI checks AGENTS.md size only. Generated outputs stay out of Git.

This decision refines [TOS-D-0069](TOS-D-0069-rust-execution-foundation.md), [TOS-D-0031](TOS-D-0031-agents-route-topology-and-progressive-disclosure.md), and [TOS-D-0041](TOS-D-0041-prompt-light-agent-routes-and-on-demand-validation.md), and retains TOS-D-0071.

## Options Considered

- Design durable architecture for the target and select mechanisms through measurement.
- Build every prospective scaling mechanism immediately.
- Defer durable identity, storage and API choices until load arrives.

## Rationale

Stable foundations preserve identity and make later growth tractable. Numerical capacity profiles connect operational complexity to observed need. Plain governance keeps contributor instructions directly editable.

## Consequences

Capacity work names objectives and measurements in the [capacity profile](../CAPACITY.md). Its proposed values await owner approval. Deployment profiles own operational limits. Transitional work names its removal condition, and maintainers keep governance documents directly.

## Source Surfaces

- `AGENTS.md`
- `docs/decisions/TOS-D-0069-rust-execution-foundation.md`
- `docs/decisions/TOS-D-0071-transactional-source-and-independent-projections.md`
- `.github/workflows/repo-validation.yml`

## Validation

Review the decision and its source surfaces together; check the AGENTS.md word limit in the repository workflow.
