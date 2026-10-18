# Native protocol load readiness

`tos-load-readiness` exercises an explicitly selected installed ToS release.
It needs no AbyssOS installation, source checkout, resource broker or host
reservation registry. Build the Rust target with `--features load-readiness`.

Create a new private output directory and choose a finite user service:

```sh
mkdir -m 700 /absolute/new-output
systemd-run --user --wait --collect --pipe --unit=tos-load-readiness.service \
  -p MemoryMax=2G -p MemorySwapMax=0 -p RuntimeMaxSec=150 \
  -p TimeoutStopSec=10 -p KillMode=control-group -- \
  /absolute/tos-load-readiness --query-binary /absolute/installed/tos \
  --model /absolute/prepared-main --binding /absolute/prepared-binding \
  --schedule /absolute/checkout/access/load-readiness/fixtures/load-readiness-two-session.json \
  --output /absolute/new-output --unit tos-load-readiness.service --sessions 2 \
  --deadline-seconds 120 --request-cap-bytes 1048576 \
  --response-cap-bytes 4194304 --schedule-cap-bytes 16777216 \
  --output-cap-bytes 8388608
```

Choose the actual memory and storage budgets for the selected workload. A host
may supply its own outer policy externally. The runner checks its real cgroup,
private output ownership and exclusive directory custody. Before setup it
allocates the full report cap in the actual report inode without extending its
JSONL payload. Concurrent runs allocate separate blocks. Completion and error
return release the unused tail while retaining the written report. Its capped
writers enforce the output bound. No reservation-service
identity is accepted as a substitute for the actual kernel limits.

Use the existing `capacity-fixture` and owner-only arguments described by the
binary's help for source-write schedules. Such runs require a pristine selected
baseline and a disposable database; they never select a live production owner
implicitly. Keep failed attempt evidence and select a fresh stage on retry.

Supported concurrency stages are 2, 8, 16, 64, 128 and 256 sessions. The runner
caps work at 4096 operations, request bodies at 1 MiB, responses at 4 MiB,
schedules at 16 MiB and output at 64 MiB. Choose session count, operation count,
channel mix, dispatch rate and load window explicitly. The whole-run deadline
includes setup, dispatch, drain, cleanup and reporting.

Reports retain exact selected executable/schedule identities, protocol results,
latency and throughput, source-projection lag, resource deltas, cancellation and
failure counts. Output uses a ToS-owned execution identity. Reports are bounded
measurements of that invocation; they do not establish a capacity ceiling, SLO,
source admission, semantic review, canon or publication.

Review each stage before increasing concurrency. Stop on nonzero exit, incomplete
work, failed invariants, cgroup pressure or peak memory reaching the existing
90-percent threshold where a finite memory limit is observed. An unavailable
limit remains unknown. Keep the pristine input and failed reports.
