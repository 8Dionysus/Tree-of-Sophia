# Capacity profile

This profile names the capacity that mechanisms are measured against, as
[TOS-D-0074](decisions/TOS-D-0074-measured-mechanisms-and-plain-governance.md)
requires. The numbers marked **proposed** are hypotheses to benchmark; the
owner approves them in review.

**Measured** means observed on a named snapshot and setup; **derived** means
calculated from stated inputs; **proposed** means a scenario or objective to
test. Every workload and recovery objective below is proposed. The one-year
stage is a planning horizon.

## Data and stages

The 2026-11-12 investigation counted **515 measured** separate JSON files
with top-level `schema_version=tos_corpus_record_v1` in `ToS/`, excluding
`ToS/contracts/` and paths containing `/history/`, at Git snapshot
`674248c45bd8a0e048a9d349d66fccbfe076b996`. This selected slice is not the
complete corpus or a measurement of runtime capacity. TOS-D-0074 sets the
architectural direction at billions of records and hundreds of concurrent
agent clients.

| Input or estimate | Now | One year | Target |
| --- | --- | --- | --- |
| Test records, proposed | 100,000 | 10 million | 1 billion; also test 2 billion |
| Logical record size, proposed | 4,000 bytes average | 4,000 bytes | 4,000 bytes |
| Index and history factor, proposed | 3× record bytes | 3× | 3× |
| Footprint, derived | 1.2 GB | 120 GB | 12 TB; 24 TB at 2 billion |
| Topology, proposed | Single test host | Dedicated PostgreSQL, separate immutable source bytes, read replica or export | Multiple database/projection hosts, object storage, separate clients |

The footprint is record count × 4,000 bytes × 3, in decimal units. It excludes
replicas, source files, temporary space and backups; actual distribution and
cost must be measured. Do not add Git, database records and historical graph
carriers as independent copies of the corpus without establishing overlap.
Binary-source volume and rights distribution remain unknown.

For each stage, **proposed** fixture sizes are 90% at 1,000 bytes, 9% at
10,000 bytes and 1% at 220,000 bytes: a **derived** mean of 4,090 bytes,
rounded to 4,000 for the estimate above. Include eight revisions for 10% of
records, access checks and removal from projections after rights changes. At
target, include a node with 100,000 relations and rights changes affecting 10%
of records. Compare with real distributions when available. The existing
[synthetic workload](../rust/crates/tos-command/src/source_capacity_workload.rs)
defines scenarios including eight revisions, a 100,000-degree hub, 256 agents
and a 90/10 read/write mix; those inputs are not observed traffic.

## Workload objectives

All values in the table are **proposed**. Concurrency is parallel in-flight
operations; rate is successful completions per second for the named class.
Latency runs from request submission through the complete response, including
network and rights checks. Measure each class separately and then a mixed
workload; separate maxima are not a claim of simultaneous capacity.

| Class | Now: concurrent; p50/p95; rate | One year: concurrent; p50/p95; rate | Target: concurrent; p50/p95; rate |
| --- | --- | --- | --- |
| Reader: card or page of up to 100 items | 8; 150/500 ms; 10/s | 64; 200/700 ms; 100/s | 256; 250/1,000 ms; 400/s |
| Search: typical query and first authorized page | 4; 300/1,000 ms; 3/s | 32; 400/1,500 ms; 30/s | 128; 500/2,000 ms; 100/s |
| Agent write: version check, atomic commit and receipt | 2; 500/2,000 ms; 1/s | 16; 700/3,000 ms; 8/s | 26 among 256 agent clients; 1,000/5,000 ms; 25/s |
| Full replaceable projection build: one at a time | 15/30 min for 100,000 records | 6/12 h for 10 million | 3/7 days for 1 billion |

Build times are proposed p50/p95 of full successful runs, not per-record
latencies. A single build cannot establish p95. Dividing the record count by
the proposed p95 time gives **derived** average rates of approximately 56,
231 and 1,654 records/s respectively; these are not sustained-phase
measurements. Measure startup, finalization and sustained throughput separately.
For incremental projection of 100 changed records, proposed p95 to current
visibility is 5 minutes now, 2 minutes in one year and 1 minute at target;
measure queue lag and rights visibility as well.

## Recovery objectives

All objectives are **proposed**, subject to owner approval and a specified
failure domain. RPO is the maximum loss of acknowledged writes. RTO starts at
failure detection and includes selecting a verified backup, restoring and
checking IDs, bytes and rights, and enabling the named traffic.

| Objective | Now | One year | Target |
| --- | --- | --- | --- |
| RPO for acknowledged writes | 0 | 0 | 0 |
| RTO for reads from verified backup | 4 h | 2 h | 1 h |
| RTO for acknowledged writes | 8 h | 4 h | 2 h |
| RTO for replaceable projection | 24 h | 24 h | 7 days |

Zero RPO for loss of an entire site requires an explicitly chosen and tested
synchronous durability architecture and may be expensive. A local failure test
does not prove site-loss recovery. Name the failure domain, backup, replication
and dependencies in every result.

## Measurement acceptance

After the owner approves the profile and an isolated test environment, use
synthetic or separately authorized data and a fixed, recorded workload. For
each stage and request class, propose three independent runs, each at least
30 minutes after at least 5 minutes of warmup and at least 10,000 completed
requests; extend the run where the rate requires it. Add a cold-start run,
rare heavy requests, overload and a mixed run of at least 30 minutes with 90%
reads and 10% writes. Record the mix's actual class rates and concurrency;
do not substitute the separate class maxima. For builds, report each complete
run and sample count; do not claim a reliable p95 from three observations.

The proposed acceptance threshold is at most 0.1% errors at the stated load,
with each class meeting its rate and latency objectives in each run. Report
successful and failed requests, p50/p95 of successful responses, timeouts,
incomplete or incorrect responses and overload refusals. Overload must fail
explicitly without corrupting data; correctness, permissions and currentness
are required alongside speed. An unmeasured or under-sampled objective is
unverified, not passed.

Exercise failure after commit before response, command retry, concurrent
conflict, checkpoint and rollback, projection lag, and node loss and restore.
Compare recovered IDs, bytes and rights with the expected committed state and
measure both RPO and each applicable RTO. Record the exact code SHA, fixture
seed and distribution, data and response sizes, clients, concurrency, hardware,
topology, software versions, network, resource peaks, queue lag and costs.
Keep detailed artifacts outside Git without private payloads. Historical
benchmarks and CI success do not substitute for this evidence.
