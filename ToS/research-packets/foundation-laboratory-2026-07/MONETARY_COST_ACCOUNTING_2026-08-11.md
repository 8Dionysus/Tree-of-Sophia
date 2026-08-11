# Monetary-cost accounting admission — 2026-08-11

Status: ordered cost and provenance-binding research complete; fail-closed v2
accounting method reviewed in `abyss-stack`; existing laboratory runs remain
monetarily incomplete

## Question

How can Tree of Sophia compare local OCR, parsing, translation, retrieval,
graph, and LLM methods by monetary cost without calling local execution free,
guessing a private electricity tariff, inventing a wage, or hiding failed
attempts behind the one successful result?

## Current evidence boundary

The foundation matrix already records wall time, memory, swap, retained bytes,
acquisition bytes, and active human time when it exists. The 2026-08-11 energy
admission additionally proves that direct run energy is unavailable to the
current laboratory identity. Those measurements do not yet produce money:

- some run routes can prove that no external provider or acquisition charge
  was incurred;
- no existing run has attributed whole-system electricity energy;
- no exact customer tariff is bound to a run;
- no hardware acquisition/lifetime/resource-allocation policy is adopted;
- active human time can be zero when no task opened, while the monetary value
  of human time remains separately unpriced.

Therefore existing total monetary cost is `unmeasured`, not zero.

## 1. Official and normative documentation

### FOCUS 1.4

The [FinOps Open Cost and Usage Specification 1.4](https://focus.finops.org/focus-specification/v1-4/)
is the current published release, ratified on 2026-06-04. It distinguishes
invoice-facing billed cost from effective cost after allocation and
amortization, and requires an explicit billing currency.

ToS adopts the distinction, not the vendor billing schema. A direct API or
cloud charge may be billed evidence. Electricity, local hardware allocation,
and human time are separate estimates and must not be mislabeled invoice rows.

### Software Carbon Intensity

The [Software Carbon Intensity specification](https://sci.greensoftware.foundation/)
requires a declared system boundary, functional unit, quantification method,
energy term, and separate embodied-hardware allocation. ToS is not calculating
SCI here, but the same boundary law prevents a CPU-package counter, full laptop,
shared service, and human review from being collapsed into one unexplained
number.

### CFE tariff source

The official [CFE domestic tariff route](https://app.cfe.mx/Aplicaciones/CCFE/Tarifas/AcervoHistorico/Tarifas/Tarifas_Casa)
requires the supply tariff found on the customer's bill before the applicable
rate can be selected. Rates also depend on tariff class, month, blocks, and
high-consumption posture. Machine location alone is insufficient.

The laboratory will not read or commit a private bill automatically. A future
electricity-cost input must cite an operator-provided tariff fact or a redacted
derived receipt with its month, class, currency, and unit rate.

## 2. Established work

### Financial and environmental cost are related but distinct

[Strubell, Ganesh, and McCallum (ACL 2019)](https://aclanthology.org/P19-1355/)
estimate financial cost from hardware and electricity or cloud-compute time,
while reporting environmental effects separately. This supports distinct cash,
energy, and environmental fields rather than one generic `cost` value.

### Energy attribution and life-cycle assumptions must be disclosed

[Henderson et al. (JMLR 2020)](https://jmlr.org/papers/v21/20-312.html)
provide systematic energy reporting and explicitly discuss the difficulty of
allocating manufacturing or life-cycle impact to one experiment. For ToS,
hardware cost is therefore an estimate only when purchase cost, expected
lifetime, allocated seconds, and resource share are all explicit.

### Efficiency belongs beside quality

[Schwartz et al., Green AI (CACM 2020)](https://cacm.acm.org/magazines/2020/12/248800-green-ai/fulltext)
argue for reporting a financial price tag for development, training, and
running alongside effectiveness. ToS preserves that pressure but refuses a
price tag whose missing components were silently set to zero.

## 3. Freshest relevant evidence at the snapshot

### ML.ENERGY — peer-reviewed 2025 benchmark

The [ML.ENERGY benchmark](https://papers.nips.cc/paper_files/paper/2025/hash/9dc510e3d7b0b3b2a58ffed7a3ad6b0f-Abstract-Datasets_and_Benchmarks_Track.html)
binds energy results to the actual task, model, and service environment. It
reinforces that a rate from another machine or a component counter cannot be
used as the energy for this run.

### A-LEMS — May 2026 preprint watchlist

[A-LEMS](https://arxiv.org/abs/2605.22883) is a fresh preprint, not a standard
or settled method. Its directly useful pressure is the unit *successful goal*:
agent orchestration, tool calls, retries, failures, and recovery attempts stay
inside the accounting boundary instead of disappearing when a final answer is
produced.

No newer source found by 2026-08-11 displaced the official billing/boundary
contracts or the established reporting law. Fresh work refines the functional
unit; it does not license guessed inputs.

## 4. Exact evidence-binding continuation

The first accounting contract hashed its canonical input but still allowed
free-form evidence labels. That was insufficient: a successful schema check
could not prove which exact run receipt or cost artifact supported a component,
and `attempt_count` did not prove that every failed or successful attempt had a
receipt. A second ordered research pass therefore addressed provenance binding
before changing the contract.

### Official and normative documentation

- [W3C PROV-DM](https://www.w3.org/TR/prov-dm/) separates immutable entities,
  activities, agents, usage, generation, and derivation. Changed bytes are a
  new evidence entity, not an update hidden behind the same label.
- [in-toto Attestation Statement v1](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md)
  identifies immutable subjects by cryptographic digest, and its
  [reference predicate](https://github.com/in-toto/attestation/blob/main/spec/predicates/reference.md)
  uses digest-bearing resource descriptors. ToS adopts subject-by-digest
  closure without claiming in-toto conformance.
- [SLSA 1.2 provenance](https://slsa.dev/spec/v1.2/provenance) keeps artifact
  identity, completeness, authenticity, and accuracy distinct. The local
  route adopts that separation without claiming a SLSA level.
- [RO-Crate 1.3](https://www.researchobject.org/ro-crate/1.3/), released as a
  Recommendation on 2026-06-22, relates research inputs, outputs, instruments,
  and actions. It informs the evidence catalog but is not itself cryptographic
  attestation.

### Established work

- The [FAIR Guiding Principles](https://doi.org/10.1038/sdata.2016.18) require
  persistent identifiers, explicit metadata-to-data links, qualified
  references, and detailed provenance.
- The [in-toto USENIX Security 2019 paper](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias)
  establishes the value of complete step/material/product verification. The
  monetary route borrows completeness discipline, not supply-chain authority.
- The [RO-Crate 2022 paper](https://doi.org/10.3233/DS-210053) demonstrates a
  lightweight research-object package applicable beyond software, including
  digital-humanities workflows.

### Freshest relevant work at the snapshot

- [REPRO-Bench (ACL Findings 2025)](https://aclanthology.org/2025.findings-acl.1210/)
  evaluates agents against papers and real reproduction packages; its best
  baseline reached only 21.4% accuracy. Automated green is therefore not
  accepted as source-visible reality.
- [Proofs of Autonomy (ICML TAIG workshop 2025)](https://openreview.net/forum?id=qxFgQHN69d)
  binds actions to verifiable execution traces and an immutable agent
  definition. Its identity, non-repudiation, MPC, and privacy claims remain
  outside this unsigned local route.
- [AgentTrace](https://arxiv.org/abs/2602.10133) (February 2026) separates
  operational, cognitive, and contextual telemetry. Structured logging is
  useful, but remains weaker than byte-verified evidence closure.
- [From Agent Traces to Trust](https://arxiv.org/abs/2606.04990) (June 2026
  preprint) distinguishes the execution-provenance graph from its evidence-
  support projection and identifies unified schemas and privacy-aware audit as
  open problems.
- [ProvenanceGuard](https://arxiv.org/abs/2607.01236) (July 2026 preprint)
  checks whether tool calls and parameters are supported by traceable context.
  It supports typed evidence references, not a claim that accounting proves
  agent alignment.

The admitted scope is deliberately narrower: exact local file bytes, byte
length, SHA-256, reference closure, and a complete attempt ledger. It does not
prove producer identity, signature authenticity, statement truth, agent
reasoning, source quality, or human acceptance.

## Admitted method

Reviewed `abyss-stack` checkpoint
`c1758692f15a588cabb21e34da5dec33efaeafd9` adds:

- `scripts/aoa-tos-monetary-cost`;
- strict input and receipt schemas;
- decimal, no-currency-conversion accounting;
- separate direct billed, electricity, hardware-allocation, and human-time
  components;
- cash, marginal, fully loaded, and per-successful-goal aggregates;
- null propagation when required evidence is absent;
- rejection of CPU-package energy as whole-system electricity;
- inclusion of retries and failed attempts in the goal boundary;
- seven source-free tests and a deliberately cash-only example.

The example proves one precise distinction: an evidenced zero external charge
can coexist with blocked electricity, unmeasured hardware allocation, zero
active human seconds, unpriced human time, and null marginal/fully loaded cost.
It is not a measurement of a historical ToS run.

Reviewed follow-up checkpoint
`60c6a9e911a6f12829de0152d50ba63f5fc7847e` closes the same ambiguity in
the two non-runner-bound Russian-surface receipt emitters. Future receipts from
those mutable routes now say `unmeasured-local-execution`, through one shared
posture, instead of `zero-local-runtime`. The regression suite rejects a return
of the false-zero label. Digest-bound translation runners retain their exact
historical bytes and profiles: their legacy field means at most that no direct
external charge was recorded. A genuinely new episode through one of those
fixed routes must pair its raw run receipt with the separate monetary-cost
evidence route before any cost is reported. No old receipt, private payload, or
model result was rewritten or rerun.

Reviewed exact-binding checkpoint
`71f8f0b56ed92aa9b9ea0abf4eb5e0a60fe41bb7` preserves v1 as a historical
compatibility surface and adds v2 beside it. For v2, the accounting command:

- resolves every evidence descriptor relative to the input packet;
- rejects absolute/traversing paths, symlinks, missing files, duplicate IDs,
  and duplicate paths;
- reads each regular file and verifies both byte length and SHA-256;
- requires every component evidence reference to resolve and every descriptor
  to be used;
- records one contiguous ordered attempt row per `attempt_count`, requires its
  status counts to agree with `successful_goal_count`, and binds each attempt
  to evidence of kind `run-receipt`;
- emits a canonical descriptor-set digest while stating
  `unsigned-not-attested` so file integrity is not mislabeled authenticity.

The public v2 fixture is source-free and its four evidence files are themselves
hash-checked by the repo-wide schema contract. Twenty-one direct monetary tests,
the complete 228-test laboratory suite, the laboratory validator,
`source-fast`, and the stack validator pass. The broader stack pytest remains
environment-blocked outside this slice because the installed host MCP SDK no
longer exports the checkout's `MCPError`/`MCPServer` API; that unrelated failure
is not reported green or repaired through the ToS accounting change.

## Decision

Promote the accounting mechanics with limits. For every new material run:

1. bind one goal and functional unit before execution in a v2 input;
2. retain every attempt, including failure and retry, as one digest-bound
   run-receipt row;
3. bind every component to a verified local evidence descriptor and reject
   both dangling and unused descriptors;
4. record direct billed charges from explicit evidence;
5. price electricity only from attributed whole-system/facility energy and a
   same-currency tariff receipt;
6. estimate hardware only under a declared allocation policy;
7. record active human seconds independently of any hourly valuation;
8. leave unavailable fields null;
9. treat a legacy emitter label as historical raw evidence, not a monetary
   aggregate;
10. compare monetary cost only inside compatible task and evidence boundaries;
11. label local SHA-256 closure unsigned unless a separately admitted signing
    or attestation route actually exists.

Do not retrofit the cash-only example onto old runs. Their monetary cost stays
unmeasured until their own evidence closes.

## What this does not establish

- a private tariff, bill, wage, hardware purchase price, or amortization policy;
- run energy for any existing experiment;
- fully loaded monetary cost for any existing experiment;
- currency conversion, tax, carbon, or social cost;
- a model, OCR, retrieval, graph, or translation winner;
- content quality, German competence, human correction quality, or canon;
- permission to rerun a model merely to populate a cost field.
- producer identity, signature authenticity, non-repudiation, or truth of the
  digest-bound evidence contents.
