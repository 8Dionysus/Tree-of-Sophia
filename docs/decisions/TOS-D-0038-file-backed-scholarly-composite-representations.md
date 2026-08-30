# File-Backed Scholarly Composite Representations

## Index Metadata

- Decision ID: TOS-D-0038
- Original date: 2026-08-30
- Surface classes: source-witness, contracts, scripts/validation, docs/route-law
- ToS layers: source-witnesses, contracts, philosophy, docs
- Tree classes: corpus, source
- Guard families: source-first authority, rights boundary, artifact representation, fixity, terminal receipt, local payload boundary
- Posture: accepted

## Context

The open-work loop selected Pyramid Texts and found two exact BSB scans of Kurt Sethe’s 1908-1910 critical edition under CC BY-NC-SA 4.0. The existing scholarly-composite contract correctly requires `content_stored: false` and `public_metadata_only`, while the bibliographic Item ladder and physical-artifact visual route describe different identities.

Storing PDFs directly in the composite packet would weaken its metadata-only boundary. Forcing Sethe’s modern reconstruction into an ancient Work or physical Artifact route would be false. Retaining only links would discard operator-authorized, fixity-addressable source evidence.

Selected decision lenses: decision object, owner/source, placement, evidence state, risk/approval, and scale/handoff.

## Decision

Keep `scholarly-composite-witness.json` metadata-only. Add a separate `scholarly-composite-file-representation` contract beneath `scholarly-composites/.../representations/<provider-part>/`. Each record binds one composite, one representation identity, one content-addressed File, exact bibliographic part, originating and corroborating provider records, acquisition authority, local payload path, fixity, rights, discovery, provenance, and all negative authority flags.

Only the representation `payload/` bytes are Git-ignored. Representation metadata and rights remain tracked. Each exact File receives its own rights record. The terminal candidate receipt may name one primary acquisition and an additive `additional_acquisitions` array so a multi-volume composite closes every acquired object without inventing a collection Item or dropping later files.

## Options Considered

- Loosen the composite packet to embed `payload`. Rejected because composite identity and exact File representation would collapse.
- Force the scans into the Work/Expression/Edition/Item ladder. Rejected because the current target is a modern multi-witness scholarly composite and no complete bibliographic ladder was responsibly resolved in this iteration.
- Treat the scans as physical-artifact visual representations. Rejected because a digitized Sethe volume is not a representation of one pyramid artifact.
- Keep URLs only. Rejected because explicit license, operator authority, capacity, and exact bytes were available, and the goal prioritizes durable acquisition.
- Add a separate File-backed composite representation layer with per-file rights and fixity. Accepted.

## Rationale

The new layer preserves the owner chain: the composite owns modern editorial identity; the artifact owns physical witness identity; the representation owns one exact byte object; the rights record owns the exact File's evidence-backed reuse posture; discovery and provenance own how the object was found and acquired. A PDF can now remain valuable source evidence without becoming accepted source text, an ancient original, or public/canon authority.

## Consequences

Multi-volume scholarly objects can be acquired one File at a time and terminal receipts can close all exact objects. Local validation checks schema, composite closure, content addressing, Git-ignore posture, rights scope, discovery target, and provenance outputs.

The first retained Files are CC BY-NC-SA, an open license with conditions rather than unrestricted reuse. That first case does not make CC licensing a contract invariant: later Files may be public-domain, permission-bound, jurisdictionally conflicting, or not authorized for redistribution, and the tracked rights record must preserve that stricter truth. Every payload remains local by repository policy; later OCR, transcription, correction, translation, public publication, or semantic use must create distinct versioned layers and pass their own rights and review gates.

This decision does not create an ancient Work, accept Sethe’s readings, prove complete witness coverage, or authorize semantic, graph, canon, server-transfer, or publication effects.

## Source Surfaces

- `ToS/source-witnesses/AGENTS.md`
- `ToS/source-witnesses/LOCAL_STORAGE_BOUNDARY.md`
- `ToS/source-witnesses/scholarly-composites/README.md`
- `ToS/contracts/scholarly-composite-witness.schema.json`
- `ToS/contracts/scholarly-composite-file-representation.schema.json`
- `ToS/contracts/open-work-candidate-receipt.schema.json`
- `ToS/contracts/rights-record.schema.json`
- `ToS/source-witnesses/discovery/runs/pyramid-texts-open-work-route.2026-08-30.v1.json`

## Validation

Run `python scripts/validate_source_witness_foundation.py`, the focused scholarly-composite representation tests, `python scripts/validate_open_work_candidate_queue.py`, `python -m unittest tests.test_open_work_candidate_queue`, `python scripts/generate_decision_indexes.py --check`, and `python scripts/validate_decision_records.py`.

Also run the broad repository gate before landing.
