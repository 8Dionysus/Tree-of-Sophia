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

## Rust protocol load readiness

`rust/crates/tos-access/src/bin/tos-load-readiness.rs` is the finite Rust
measurement target for an installed native Rust release. Build it from the
selected ToS source cohort with `cargo build --release -p tos-access --bin
tos-load-readiness --features load-readiness`, then launch its absolute path
through the existing `abyss-machine resource launch` route. The harness is a
client: it starts the selected `tos` Streamable HTTP listener and, only when
scheduled, the native source-command HTTP owner. It adds no server endpoint,
source grant, or publication step. It requires no Python runtime.

The protected schedule has the form
`{"schema":"tos_protocol_load_schedule_v1","sessions":[{"id":"s-0","operations":[...]}]}`.
Each session contains an ordered operation list. `--sessions 2|8|16|64|128|256`
selects the stage (default 2); the harness launches each operation index as a
concurrent wave across sessions. An operation declares `channel`, `label`,
`request`, `expected_status`, and `expected_sha256`. Channels are `mcp` for an
ordinary 2025-11-25 Streamable HTTP JSON-RPC request, `owner_http` for the
exact authenticated native source-command request object, and `sdk` for an
explicit absolute SDK consumer command. SDK receives one bounded JSON line on
stdin and returns the response bytes on stdout; stderr is discarded. The input
writer and bounded output drain run concurrently so a full-duplex exchange
cannot deadlock on full pipes. A deadline or output-cap breach terminates the
owned process group and records the operation as an error.

For a conflict race, set the same `conflict_group` on at least two owner HTTP
operations in one wave and give every member the same
`alternate_outcomes` set, containing one exact 2xx status/digest and one exact
409 status/digest. Each operation passes when its own signed response matches
either declared outcome; the group passes only when exactly one member succeeds
and all remaining members return the signed 409. This accepts whichever session
wins the race while preserving per-status response checks. Every scheduled
owner command ID must still be unique. `retry_same_command_id` deliberately
closes after signed response headers and retries the identical command body and
ID with a fresh transport nonce. `reconnect_before` deletes and reinitializes
the MCP session before the operation.

For a small two-session smoke, use
`access/integrations/abyssos/fixtures/load-readiness-two-session.json`. It is a
finite exact-oracle schedule for one MCP `ping` in each session, using the
native handler's deterministic `{"result":{}}` response. It needs no content
specific to the selected model or binding and does not carry a source-writing
request. The source fixture must remain on the selected reviewed checkout.

Launch the smoke through the canonical host wrapper; the uppercase fields below
are explicit values selected for that attempt:

```sh
abyss-machine resource launch --class CLASS --kind benchmark \
  --memory-demand-mib MEMORY --bytes OUTPUT_CAP --target OUTPUT \
  --unit UNIT --timeout OUTER_SECONDS --json -- \
  ABS_LOAD_READINESS \
  --query-binary ABS_INSTALLED_TOS \
  --model ABS_PREPARED_MAIN --binding ABS_PREPARED_BINDING \
  --schedule ABS_CHECKOUT/access/integrations/abyssos/fixtures/load-readiness-two-session.json \
  --output OUTPUT --unit UNIT --sessions 2 --deadline-seconds 120 \
  --request-cap-bytes 1048576 --response-cap-bytes 4194304 \
  --schedule-cap-bytes 16777216 --output-cap-bytes OUTPUT_CAP
```

When a schedule uses `owner_http`, pass the owner-only option group
`--owner-binary ABS_INSTALLED_NATIVE_OWNER_COMMAND --owner-config ABS_OWNER_CONFIG
--invocation ABS_NATIVE_INVOCATION --token-file ABS_TOKEN`; the target requires
all four together and reads the token only for an owner HTTP run. The host launch
must create the pre-existing private output directory through the current frozen
owner launch body and retain its fresh, unique terminal write reservation. The
helper checks the exact cgroup unit, output ownership and mode,
and canonical reservation before opening an exclusive mode-0600
`measurements.jsonl`. The report keeps the existing `start`, `request`, and
`finish` envelope and adds MCP session/catalog observations. It records exact
status and response hashes, retries, reconnects, conflict-group pass/fail,
latency quantiles, cgroup CPU/memory/I/O deltas, and report allocation. The
catalog preflight records only handler count and digest, not a copied function
catalog.

The helper caps sessions at 256, operations at 4096 (64 per session), request
bodies at 1 MiB, response bodies at 4 MiB, schedules at 16 MiB, and report
output at 64 MiB (default 8 MiB). It requests 256 KiB stacks for session
workers and 128 KiB for each concurrent SDK output drain. At the maximum stage,
wire buffers can reach roughly 1.25 GiB before decoded schedules, runtime/server
state, SDK RSS, stacks, and filesystem allocation slack; the outer reservation
must fit the selected stage. The helper records cgroup resource deltas but does
not establish a capacity ceiling or SLO.

The existing Rust Streamable HTTP route admits at most 32 sessions and 32
connections, expires idle sessions after 900 seconds, and returns 503 at its
session/connection limits or on a busy session. Native source-command HTTP has
a 1 MiB request and 4 MiB response ceiling, 1024 nonce entries, a serial
handler with backlog 4, 5-second idle and 30-second request deadlines, and no
automatic retry. The harness reports these source limits as observed support
boundaries; it does not claim they admit the larger configured client stages.

Selected executable identities are recorded with SHA-256 (each executable is
bounded to 512 MiB); prepared model, binding, owner inputs, and executables are
checked for path/device/inode/size/time/mode stability. The schedule content is
SHA-256 checked before and after the run. This is a finite protocol measurement
of the selected installed release, not capacity acceptance, source validation,
semantic review, owner authorization, or publication.
