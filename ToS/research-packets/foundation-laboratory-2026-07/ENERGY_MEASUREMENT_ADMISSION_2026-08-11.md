# Energy Measurement Admission — 2026-08-11

Status: direct energy observation researched and capability-tested; current
unprivileged `abyss-machine` route blocked by counter permissions

## Question

Can the Tree of Sophia laboratory add energy to its machine-cost axis without
inventing a value, confusing CPU-package energy with whole-system energy, or
widening host authority?

## Ordered research

### 1. Official documentation

- The Linux kernel [Power Capping Framework](https://kernel.org/doc/html/next/power/powercap/powercap.html)
  defines zonal `energy_uj` counters and `max_energy_range_uj` in the
  `powercap` sysfs tree. Intel RAPL package, core, and uncore zones are scoped
  hardware observations; they do not claim whole-machine coverage.
- The Linux kernel [power-supply class](https://www.kernel.org/doc/html/latest/power/power_supply_class.html)
  defines battery energy in micro-watt-hours and power in micro-watts. The
  interface describes battery state; causal attribution to one experiment
  still requires a controlled discharging protocol and workload boundary.

Adopted: direct read-only inspection under the exact laboratory identity,
explicit units, component scope, counter range, and `null` for an unavailable
observation. Rejected: changing host permissions from the research lane,
writing power limits, or treating battery capacity as automatic workload
energy.

### 2. Established work

Hähnel et al., [Measuring Energy Consumption for Short Code Paths Using
RAPL](https://www.sigmetrics.org/greenmetrics/2012/papers/Hahnel.pdf) (2012),
established the practical start/end counter method while documenting update
granularity, privilege, background-load, and hardware-scope limits.

Adopted: explicit start/end boundaries, idle baseline, range-aware delta,
single-wrap support, and rejection of intervals where unknown multiple wraps
could occur. RAPL remains a component observation rather than an external
whole-system meter.

### 3. Fresh primary work at the actual snapshot

- The peer-reviewed NeurIPS 2025 [ML.ENERGY
  Benchmark](https://papers.nips.cc/paper_files/paper/2025/hash/9dc510e3d7b0b3b2a58ffed7a3ad6b0f-Abstract-Datasets_and_Benchmarks_Track.html)
  binds energy measurements to concrete inference tasks, model/runtime
  configurations, and optimization outcomes.
- The May 2026 preprint [Energy per Successful
  Goal](https://arxiv.org/abs/2605.22883) is retained as a current watchlist
  direction for agentic work: account for the bounded goal including retries
  and failures, not only one successful model call. It is a fresh preprint,
  not an adopted standard or evidence about this Intel host.

Adopted now: future energy receipts must bind the exact hardware/runtime,
workload boundary, failures, and corresponding quality or success outcome.
Deferred: carbon conversion, cross-machine ranking, and leaderboard claims.

## Implemented owner route

Reviewed `abyss-stack` checkpoint `7f28312e` adds:

- `scripts/aoa-tos-energy-measurement`;
- a source-free capability schema;
- direct `powercap`, actual `perf` event, and battery-state probes;
- one-wrap delta mechanics;
- synthetic tests for readable, permission-blocked, unavailable, and fallback
  routes;
- an explicit ban on privilege elevation and zero substitution.

The energy utility is separate from the byte-fixed historical translation
runner. It writes nothing by default and returns exit status 2 when direct
measurement is not admitted.

## Current host evidence

The live 2026-08-11 probe under the current unprivileged laboratory identity
returned a schema-valid `blocked-counter-permission` receipt:

- kernel `7.1.7-200.fc44.x86_64`;
- four discovered energy zones: two package routes, core, and uncore;
- every `energy_uj` read denied;
- every declared range readable as `262143328850` microjoules;
- actual `perf` probe for `power/energy-pkg/` returned
  `event-unsupported` with exit 1;
- `BAT1` was present and `Not charging`, with
  `energy_now=47100000` micro-watt-hours and `power_now=0`, so it could not
  attribute a workload.

No source text, model, private candidate, payload, ACL, udev rule, power limit,
or host permission was read or changed by this result.

## Decision

Retain the capability method and its negative host result. Existing experiment
energy remains unmeasured, not zero. Monetary cost, content quality, and human
correction cost remain independent open fields.

Do not:

- label package RAPL as whole-system energy;
- estimate joules from duration alone;
- reuse the battery value while AC/not-charging;
- mutate host access from ToS or `abyss-stack`;
- rerun a model merely to exercise this new counter route;
- infer an efficiency winner without a quality- or success-bound workload.

A future exact workload may record energy only after an `abyss-machine` owner
route makes a direct counter readable or supplies a separately governed
external/whole-system measurement. That is a new host decision, not an open
permission question inside this research packet.
