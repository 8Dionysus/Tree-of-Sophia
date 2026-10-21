# TOS-D-0072 Repository-independent execution and exports

## Index Metadata

- Decision ID: TOS-D-0072
- Original date: 2026-10-17
- Surface classes: access/runtime, docs/route-law, kag/provider, scripts/validation
- ToS layers: access, contracts, mechanics, derived exports, docs
- Tree classes: none
- Guard families: source-first authority, artifact integrity, compatibility, scoped validation
- Posture: accepted by explicit operator instruction

## Context

ToS declares an independent product, but several operations still require
services or source checkouts from neighboring ecosystem projects. Ordinary SDK
and desktop launch select AbyssOS resource/storage machinery. Signed OCR import
checks an abyss-stack checkout and executes its Python adapter. KAG and stats
publication select external code, interpreters and validators. Local source
checks additionally contain consumer producer/runtime verification.

The Operator explicitly required removal of these dependencies while preserving
useful libraries, databases, tools and website analytics. A future maintainer
needs this boundary to distinguish an independent ToS operation from a merely
optional command that still implements another repository's integration.

## Decision

ToS operates, builds and releases its own data without an ecosystem repository,
its Python environment, its host services or its admission broker. Consumers
connect from their own side using ToS-owned artifacts and contracts.

The native ToS launcher owns its real per-operation Linux containment, private
storage, deadlines, cancellation and cleanup. It accepts explicit locations
and checks kernel state. It does not discover an AbyssOS installation or daemon.

Signed OCR import verifies the independently pinned signer, receipt, source
binding and exact output bytes through a native standard cryptographic library. Producer
source references and implementation digests are provenance, without checkout
inspection or execution. Verification authenticates the signed assertion; it
does not repeat the producer's runtime admission or accept source fidelity,
rights, interpretation or canon.

ToS builds and verifies bounded source-return and stats exports. A consumer
owns provider configuration, indexes, additional grammar checks, trust policy,
registry and admission. Those procedures are not ToS commands or release gates.
Retained source bytes, historical evidence and explicit data selection remain
protected.

This supersedes the KAG provider/action pin and external validation requirements
in TOS-D-0030, TOS-D-0035, TOS-D-0044, TOS-D-0065 and TOS-D-0066,
and the KAG integration prescriptions in TOS-D-0026, TOS-D-0028 and TOS-D-0029.
Those records preserve historical rationale and identities; deleted provider
pins and adapters must not be restored as prerequisites.

This supersedes ToS-owned integration orchestration in TOS-D-0062. Its separate
software/corpus identities and exact-byte obligations remain. TOS-D-0071's
PostgreSQL transactional center and independent portable projections remain.

## Options Considered

- Remove all third-party libraries and replace their capabilities internally.
  The Operator narrowed this direction to retain useful dependencies.
- Keep ecosystem adapters inside ToS behind optional profiles. The Operator
  rejected optionality as a sufficient answer to repository dependence.
- Keep ToS execution and export on the producer side, with consumer integration
  implemented by consumers. This is the accepted boundary.

## Rationale

Growth of the philosophical source home should not require maintaining other
projects' installations, policies, code identities or release procedures. Mature
libraries and databases directly support ToS capabilities; an ecosystem source
checkout is an avoidable operational dependency. The boundary also gives every
consumer responsibility for its own changing runtime and admission rules.

## Consequences

Retire reverse integration commands, host discovery and the corresponding unused
validation bodies, templates, fixtures and navigation. Preserve useful standard
tools and existing Linux safeguards. Public Cloudflare hosting and analytics
remain explicit capabilities of the site. External consumers may need to adapt
their import procedures; ToS does not mutate those repositories or claim their
runtime acceptance.

Local checks must exercise standalone exports, exact membership, tamper refusal,
original source binding and preservation of a prior valid result. A green source
check is separate from CI, publication, installation and runtime acceptance.

## Source Surfaces

- `BOUNDARIES.md`, `access/AGENTS.md`, `access/README.md`
- `rust/crates/tos-access/src/private_stage_run.rs`
- `rust/crates/tos-access/src/doctor.rs`
- `rust/crates/tos-command/src/source_text_owner_ocr.rs`
- `rust/crates/tos-ops-mechanics-plan/src/constructor_desktop_main.rs`
- `rust/crates/tos-ops-mechanics-plan/src/kag_corpus_export.rs`
- `rust/crates/tos-ops-mechanics-plan/src/stats_release.rs`
- `rust/crates/tos-ops-mechanics-plan/src/agent_surface_exports.rs`
- `kag/VALIDATION.md`, `stats/VALIDATION.md`, `docs/RELEASING.md`

## Validation

Use the native decision index builder and record validator in `docs/VALIDATION.md`.
Run the affected standalone CLI, SDK, signed-OCR and source-route checks. Preserve
exact source/history and verify software packaging against the changed native
command roles. Consumer builds and admission run at their respective owners.
