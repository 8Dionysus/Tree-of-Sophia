# Monetary-cost accounting admission — 2026-08-11

Status: ordered research complete; fail-closed accounting method reviewed in
`abyss-stack`; existing laboratory runs remain monetarily incomplete

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

## Decision

Promote the accounting mechanics with limits. For every new material run:

1. bind one goal and functional unit before execution;
2. retain every attempt, including failure and retry;
3. record direct billed charges from explicit evidence;
4. price electricity only from attributed whole-system/facility energy and a
   same-currency tariff receipt;
5. estimate hardware only under a declared allocation policy;
6. record active human seconds independently of any hourly valuation;
7. leave unavailable fields null;
8. treat a legacy emitter label as historical raw evidence, not a monetary
   aggregate;
9. compare monetary cost only inside compatible task and evidence boundaries.

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
