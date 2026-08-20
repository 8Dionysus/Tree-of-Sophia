# Provenance execution receipt v2 research — 2026-08-11

## Question and boundary

The existing `tos_provenance_event_v1` records are useful navigation and
historical evidence, but their contract can admit nullable output digests, a
coarse method object, no explicit derivation edges, no first-class failed-run
byproducts, no structured model invocation, and no separation between replay
readiness, receipt authentication, human review, rights, and content authority.

The bounded question was therefore:

> What is the smallest additive execution-provenance contract that lets a
> later owner return from an output to exact input bytes, activity, method,
> software/model configuration, responsibility, manual intervention, terminal
> state, measurements, rights posture, and review boundary without rewriting
> legacy provenance or treating a green receipt as truth?

The research order was official and classical standards first, established
work second, and the freshest relevant 2025–2026 work last. The cutoff is
2026-08-11. No private source, model, or candidate output was opened.

## 1. Official and classical standards

### W3C PROV

[PROV-DM](https://www.w3.org/TR/prov-dm/) remains the stable conceptual base.
It separates Entity, Activity, and Agent; distinguishes usage and generation;
requires actual influence before calling an entity derived from another; and
models responsibility separately from the thing produced. It also supplies
bundles for provenance of provenance rather than assuming the provenance
record authenticates itself.

[PROV-CONSTRAINTS](https://www.w3.org/TR/prov-constraints/) adds ordering and
impossibility constraints. Generation precedes use and invalidation, an
activity start precedes its end, and identifiers for different relation kinds
cannot silently collapse. ToS therefore needs cross-field validation in
addition to JSON shape.

[PROV-O](https://www.w3.org/TR/prov-o/) is useful as an interoperability
projection. It is not a reason to make the graph the source of authority. ToS
keeps the event record and exact entities as owner data; a future RDF graph is
derived.

### PREMIS 3

[PREMIS 3](https://www.loc.gov/standards/premis/v3/index.html) is still the
current Library of Congress preservation-data standard and explicitly models
Objects, Events, Agents, and Rights. Its preservation-event guidance preserves
the important one-to-one law: changing a digital object creates a new object
rather than changing the identity of the old one. The v2 ToS receipt therefore
binds distinct input/output entities and does not represent correction or
normalization as an in-place overwrite.

PREMIS also makes the software/hardware environment relevant to long-term
interpretability. For ToS that means the runtime, executable digest, backend,
hardware target, Unicode data version, and an environment-profile digest are
not incidental log prose.

### in-toto and SLSA

The [in-toto Statement v1](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md)
binds an attestation to immutable subjects by digest. Its
[ResourceDescriptor](https://github.com/in-toto/attestation/blob/main/spec/v1/resource_descriptor.md)
keeps identity, digest, media type, and retrieval location distinguishable.
This supports an external manifest that hashes the exact receipt bytes and
digest-bound entity descriptors inside the receipt.

[SLSA provenance v1.1](https://slsa.dev/spec/v1.1/provenance) separates build
definition from run details, records resolved dependencies, builder identity,
invocation time, subjects, and byproducts, and makes verification an explicit
consumer operation. ToS borrows that separation, but does not call its local
unsigned research receipt a SLSA attestation or a verified execution.

### RO-Crate 1.3 and Croissant 1.1

[RO-Crate 1.3](https://www.researchobject.org/ro-crate/specification/1.3/index.html)
is the newest Recommendation, published 2026-06-22. Its
[provenance guidance](https://www.researchobject.org/ro-crate/specification/1.3/provenance.html)
records input objects, result objects, responsible agents, instruments,
software versions, start/end time, failure status, and failure information. It
also says that a curation action modifying a file should retain the old version
and produce a new result. The guidance is suitable for future interchange, not
a substitute for ToS's stricter byte and authority gates.

[Croissant 1.1](https://mlcommons.org/2026/02/croissant-1-1-standard/) is a
fresh dataset-metadata signal rather than the owner contract here. Its 2026
revision adds PROV-O lineage and structured usage-policy links. This confirms
that provenance and permission need to travel together while remaining
different questions.

## 2. Established work that defined the field

- Freire, Koop, Santos, and Silva,
  [*Provenance for Computational Tasks: A Survey*](https://doi.org/10.1109/MCSE.2008.79)
  (2008), established that useful computational provenance must support
  procedure inspection, input identification, result understanding, and
  possible reproduction; a traditional free-form notebook does not scale to
  complex computational pipelines.
- Moreau et al.,
  [*The Open Provenance Model core specification v1.1*](https://doi.org/10.1016/j.future.2010.07.005)
  (2011), established interoperable, technology-independent provenance and
  multiple coexisting description levels. A ToS domain profile should map to
  the shared model without surrendering its stronger local invariants.
- Moreau et al.,
  [*The rationale of PROV*](https://doi.org/10.1016/j.websem.2015.04.001)
  (2015), explains why PROV is deliberately extensible and lightweight. This
  supports a ToS profile for text, OCR, models, review, rights, and competence
  rather than inventing a competing universal ontology.
- Moreau,
  [*A Canonical Form for PROV Documents and Its Application to Equality,
  Signature, and Validation*](https://doi.org/10.1145/3032990) (2017), shows
  that structural validity, equality, integrity, and authorship/signature are
  separate properties. That separation is decisive: exact hash closure of a
  ToS receipt does not authenticate the producer or prove the reported run.

## 3. Fresh and currently relevant work

### Reproducibility and signatures

Pritchard and Wicenec,
[*Formal definition and implementation of reproducibility tenets for
computational workflows*](https://doi.org/10.1016/j.future.2024.107684)
(journal issue 2025), distinguish multiple reproducibility conditions and use
hash-graph workflow signatures to expose differences between nominally similar
runs. ToS therefore records the exact replay scope and known gaps instead of a
single boolean `reproducible` claim.

### AI and agentic workflows

Souza et al.,
[*PROV-AGENT*](https://arxiv.org/abs/2508.02866) (IEEE e-Science 2025), extend
W3C PROV with prompts, responses, model invocations, agent decisions,
scheduling data, and telemetry so downstream effects of a faulty decision can
be traced. This is directly relevant to solo+AI ToS work. Model identity,
revision, weights, quantization, converter, prompt digest, seed, decoding
configuration, input/output digest, runtime, backend, and hardware belong in a
structured invocation rather than a free-form note.

The paper is an early system evaluation, not stable universal law. ToS adopts
the traceability pressure while keeping model output, agent decision, human
authorization, human review, and accepted content as different roles.

### Digital humanities correction lineage

Guo and Wei,
[*From OCR to Analysis: Tracking Correction Provenance in Digital Humanities
Pipelines*](https://arxiv.org/abs/2603.00884) (NLP4DH 2026), show that silently
overwritten corrections change downstream entities and interpretations. The
paper reinforces two layers already required by ToS: span-level correction
lineage inside text layers and event-level execution lineage around the
transformation. Neither can replace the other.

### Controlled disclosure for AI research objects

Binkyte et al.,
[*Inspectable AI for Science*](https://arxiv.org/abs/2604.11261) (2026), frame
model configurations, prompts, outputs, and logs as inspectable research
objects while emphasizing confidentiality, integrity, and controlled
disclosure. It is explicitly a position and demonstrative workflow, not a
finished standard. Its useful consequence for ToS is narrower: a private
prompt or response may be withheld while an exact digest and reason remain;
withholding must then reduce replay claims rather than disappear from the
record.

## Decision for ToS

1. Preserve `tos_provenance_event_v1` unchanged as legacy owner evidence.
   Introduce `tos_provenance_event_v2` additively and adopt it only for new
   materialized transformations or question-triggered migration.
2. Make exact, distinct input, output, and byproduct entities first-class.
   Successful events require authoritative outputs. Failed and stopped events
   must not relabel partial material as output; diagnostics remain byproducts.
3. Require explicit derivation edges. Co-occurring use and generation are not
   automatically derivation. `identity_copy_of` additionally requires equal
   byte digest and size.
4. Capture exact argv, configuration, software/runtime digests, environment,
   model invocation details, terminal state, and resource measurements. An
   unavailable measurement is recorded as unavailable, not zero.
5. Record manual changes as `none_declared`, `recorded`, or `unknown`.
   `recorded` requires a digest-bound change receipt; `unknown` blocks a
   replay-ready posture.
6. Keep four independent planes:
   - lineage and byte closure;
   - replay specification;
   - evidence authentication/signature;
   - review, rights, publication, and canon authority.
7. Bind exact event-record bytes from an external manifest. Avoid a
   self-referential self-hash and avoid claiming that SHA-256 proves the
   event happened.
8. Permit digest-only controlled disclosure for sensitive commands, prompts,
   or artifacts, but do not call such a packet fully replay-ready.
9. Keep the graph downstream. A PROV-O projection may improve traversal; the
   exact event and entity bytes remain authority.

## Public synthetic A/B/C

`provenance-event-v2-abc/` exercises three actually executed commands over a
public NFD UTF-8 input fixture:

- A: byte-identical copy with `identity_copy_of`;
- B: NFC transformation with a distinct digest and `revision_of`;
- C: an intentionally failed ASCII-strict operation, exit code 7, one exact
  diagnostic byproduct, and zero authoritative outputs.

All three receipts are unsigned and model-free, claim no human evidence, and
authorize no publication or promotion. Fourteen negative controls test record
and input digest drift, command drift, terminal/output contradictions,
identity-copy drift, escaping derivations, absent model invocation, withheld
replay command, false signature verification, publication without authority,
fabricated human review, unreceipted manual change, and self-supersession.

The validator proves schema and byte closure. Independent raw-byte inspection
must still confirm the copy, Unicode transformation, and honest failed-output
boundary. Neither route proves execution truth or content quality.

## Rejected alternatives

- **Replace v1 in place:** rejected because it would rewrite historical owner
  evidence and trigger a large, low-value migration.
- **One mutable event record with status updates:** rejected because it erases
  prior state; successor events use new identity/version and explicit
  supersession.
- **Hash-only provenance:** rejected because fixity says nothing about method,
  responsibility, rights, or truth.
- **Logs as outputs:** rejected because failure diagnostics and partial files
  are byproducts unless separately admitted.
- **Signature fields that default to trusted:** rejected. Unsigned is explicit;
  signed and verified requires a separate binding and verification result.
- **Graph-native authority:** rejected. The graph is a query projection of the
  owner event and entity records.
- **Store all prompts and source content publicly for replay:** rejected.
  Controlled disclosure and rights/privacy boundaries outrank maximal capture.
- **A universal `reproducible: true`:** rejected in favor of replay scope,
  classification, and named gaps.

## Adoption trigger

Use v2 for new content-bearing transformations, model/agent invocations,
exports, or failures where exact replay and downstream influence matter.
Migrate a v1 lineage only when a concrete discrepancy, source-return question,
security review, or reproducibility decision needs stronger evidence. Do not
bulk-migrate merely to increase a schema-version count.
