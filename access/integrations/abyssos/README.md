# Optional AbyssOS integrations

This directory is the adapter boundary for ecosystem deployment, KAG, shared
stats, and artifact admission. It intentionally contains no required runtime
code in the standalone profile.

The AbyssOS adapter is currently paused by the ToS runtime posture. No
deployment, KAG consumer activation, or artifact admission is implied until an
explicit ToS operator command unfreezes this boundary.

Tree of Sophia owns the standalone product and its projection readers.
`abyss-stack` may deploy the product; `aoa-kag` and `aoa-stats` may consume
bounded ports; `abyss-machine` retains artifact policy, registry, and consumer
admission. None of those owners is imported into the query core.

## Explicit native prepared read measurement

`measure_prepared_reads.py` is an optional host-side, finite CLI workload
component for E4. It does not activate an adapter or accept capacity. It only
invokes `knowledge search --mode compressed` with an explicitly selected native
binary, standalone prepared SQLite main and binding; it never prepares or
publishes data. Protected input files must be absolute, regular, non-symlink,
read-only files; the SQLite main must have no WAL/SHM/journal sidecars.

Provide a protected JSON schedule array. Every entry has exactly `argv`,
`exit_code`, `stdout_json`, `stdout_text`, `stderr_text`. For successful JSON,
set `stdout_text` to null and supply the complete expected packet in
`stdout_json` (including schema/revision/cursors). For refusals, set
`stdout_json` to null and provide exact stdout and stderr strings. Do not
replace refusal codes or expected packets with success labels. Requests run
once; C is caller supplied, no larger than the finite request count.

The caller creates an isolated output directory through the existing canonical
host launch/storage route. Example command shape (all uppercase values are
explicit selected profile values, not defaults):

```sh
abyss-machine resource launch --class CLASS --kind benchmark \
  --memory-demand-mib MEMORY --bytes OUTPUT_CAP --target OUTPUT \
  --unit UNIT --timeout OUTER_SECONDS --json -- \
  python3 ABS_DRIVER --binary ABS_TOS --model ABS_MODEL --binding ABS_BINDING \
  --schedule ABS_SCHEDULE --output OUTPUT --unit UNIT \
  --concurrency C --deadline-seconds SECONDS --output-cap-bytes OUTPUT_CAP \
  --response-cap-bytes RESPONSE_CAP --schedule-cap-bytes SCHEDULE_CAP \
  --max-requests REQUEST_COUNT
```

The output directory must already exist when the driver starts (use the
existing frozen owner launch body to create it). Driver checks its exact cgroup
unit and a fresh, unique canonical `write-reservation list` receipt with a
terminal execution hold covering the output cap. Its exclusive
`measurements.jsonl` streams identities, per-request elapsed time, exit status,
complete bounded stdout/stderr and comparison result, then unchanged-input
checks. Deadline/output refusal terminates this attempt; it does not retry or
raise caps. Each child process group is terminated and reaped, including
surviving pipe holders. No native call is made during source validation.

Physical/accounting scope: two full input hash passes plus schedule decode;
C child CLI processes (cold process/open per call), at most C RESPONSE_CAP raw
response buffers, JSON decode/serialization transient allocations, and at most
OUTPUT_CAP written bytes plus filesystem allocation slack. Schedule storage is
bounded by SCHEDULE_CAP and REQUEST_COUNT, but decoded Python objects and
threads are additional RSS, not bounded by serialized bytes alone. Source
capture/compiler/postings/sort/publication/history/restore and HTTP persistent
server measurements remain separate components. The existing native request
meters and budget refusals are unchanged. A whole E4 profile still requires
hardware/topology, N/E/R/U/text/degree/skew/churn/pin/concurrency dimensions,
selected identities, numerical SLO and fresh aggregate physical admission;
this finite component supplies no billion-record or hundreds-client claim.

For the persistent native HTTP component, add `--transport http --port PORT
--server-log-cap-bytes LOG_CAP`. PORT is an explicit IPv4 loopback port. The
same finite schedule instead contains exact `{ "path": "/api/knowledge/search?
mode=compressed&query=...", "status": 200, "response": FULL_EXPECTED_JSON }`
entries (the path must be one string with no whitespace introduced by this
illustration). Compressed search, search capabilities and exact knowledge node GET routes are
accepted. Error packets/statuses are compared unchanged, including stale,
budget, deadline and unavailable refusals; they do not become successes.

One protected native `serve 127.0.0.1:PORT` child owns the entire series. Driver
checks its Linux socket inode ownership before/after requests and refuses an
unrelated listener. It uses no external URL, redirects or publication request.
Connections close after each GET; the server and prepared backend persist, so
this measures persistent-server operation rather than CLI process cold opens.
Startup latency is reported separately. HTTP bodies are read in bounded
chunks, server stdout/stderr are streamed under LOG_CAP and the same aggregate
OUTPUT_CAP, and all server threads/pipes/process groups are joined/closed/reaped
before final input guards. The log drain also terminates the server at the
whole deadline or cancellation. HTTP-library header parsing has its standard
100-header/65536-byte-line bounds; account up to roughly6.4MiB header bytes per
concurrent connection plus object overhead, separately from RESPONSE_CAP body
bytes. This is a source-level allocation envelope, not measured peak RSS.

HTTP cost adds one persistent server baseline/backend caches, at most C client
connections/header parsers/body/decoded/serialized buffers, bounded streamed
server-log chunks, startup and terminal cleanup to the earlier formula. No
additional dataset copy or publication occurs. Use a separate owned output
attempt for CLI and HTTP; neither variant starts unless admission and all caps
are explicit. No runtime capacity or numerical SLO is implied by this source.

For repeated HTTP reads, the same protected schedule may instead be an exact
object `{ "shared_response": RESPONSE, "requests": [{ "path": PATH, "status": STATUS }] }`.
The request list remains explicitly finite and bounded by `--max-requests`; every
response is compared against the complete shared JSON oracle. The driver decodes
one oracle and references it without cloning. The original per-request schedule
array remains supported. Compute the encoded schedule size before writing it; the
existing schedule byte cap still applies before parsing/allocation.

The initial model must have no SQLite sidecars. After server termination, the
driver still requires exact main-file FD/path identity and SHA equality, but
permits newly created regular, same-owner coordination files only when the model
is inside the owned output directory: an empty WAL and SHM at most32KiB. This is
the selected empty-WAL workload resource profile, not a universal SQLite size
rule. Any rollback journal, symlink or nonempty WAL refuses the result. The
coordination artifacts are recorded and count toward the output reservation.
Native readers retain their ordinary readonly pathname/WAL semantics.

A finite mixed HTTP workload may share several exact packets with
`{ "shared_responses": [PACKET], "requests": [{ "path": PATH, "status": STATUS, "response_index":0 }] }`.
Indices and request/oracle counts are validated before constructing references;
packets are never cloned per client. HTTP records separately classify exact
responses, the exact `503 server busy / unavailable` envelope, connection refusal
and other errors. Refusals remain failed exact-response checks and do not become
successes. The finish record carries these counts; client futures do not measure
server-admitted concurrency.

Installed executables may retain0755 ownership modes. Protection is admitted
only when input permission bits prohibit writes or `statvfs(ST_RDONLY)` proves
the actual execution namespace mount is readonly. Writable unmounted inputs
still refuse; full opened-FD/path/stat/SHA checks remain before and after the
workload. Do not chmod the installed original or copy its ELF to satisfy this
check. Mount protection is observed inside execution, not inferred on the host.
