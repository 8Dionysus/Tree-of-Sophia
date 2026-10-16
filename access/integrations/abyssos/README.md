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

## Native SDK host paths

The existing `sdk-host-session` launcher uses the embedded
[`abyss-sdk-host.v1.json`](../../profiles/abyss-sdk-host.v1.json) profile.
`TOS_ABYSS_MACHINE_PATH` explicitly selects an absolute host executable;
otherwise the profile's installed candidates apply. The adapter reads the tmp
root from `abyss-machine storage paths --json` and the admission socket from
`abyss-machine resource paths --json`, checking their declared provider ABIs.
It retains and rechecks the tmp parent and keeps the existing socket owner,
peer UID, deadline, lease and storage-admission checks. Data roots select data;
this software profile and executable come from the installed product/operator.

This launcher remains an optional host capability for ordinary source SDK
operations. The standalone prepared reader, CLI and query core retain their
own routes. The frozen ecosystem deployment profile still requires its explicit
unfreeze operation. Python result heap geometry remains a separate receiving
contract.

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
`{"schema":"tos_protocol_load_schedule_v1","setup":[...],"sessions":[{"id":"s-0","operations":[...]}],"postload":[...]}`;
`setup` is optional for non-mutating work, and `postload` is optional outside a
native PostgreSQL stage. Each session contains an ordered operation list.
`--sessions 2|8|16|64|128|256` selects the stage (default 2);
the harness launches each operation index as a concurrent wave across logical
sessions. `--operation-count` caps measured operations independently and selects
them in round order across selected sessions; a truncated conflict group is
rejected by schedule validation. Each operation declares `channel`, `label`,
and `request`. Ordinary operations also declare an expected status plus either
an exact `expected_sha256` or a complete `expected_body`. Bounded navigation
advances use expected status zero and the harness's structural range oracle
instead. A semantic body expectation still checks exact JSON shape and fixed
values; `ignored_json_paths` admits only the harness's bounded typed volatile
fields. MCP, signed owner HTTP, and native
SDK channels keep separate in-flight limits. Native SDK concurrency is selected
with `--sdk-in-flight 1..4` (default 4); each item is a separate native process.
`--channel-mix mcp=P,owner_http=P,sdk=P,projection=P` selects the requested
percentage split across measured operations; the supplied percentages must sum
to 100. Selection preserves each session's operation order, so the runner
rejects a mix that cannot be reached as per-session prefixes instead of
reordering or skipping dependencies. Without this option, the schedule defines
the mix. `--operations-per-second` paces combined dispatch across channels after
the operation acquires its channel and source-read in-flight permits; it cannot
reserve a rate slot while queued behind capacity. `--load-window-seconds`
closes dispatch after the measured load window and lets already-started work
finish. Operations still waiting for capacity or a rate slot when the window
closes are recorded as not started. Operation latency starts at dispatch, while
`load_window_elapsed_seconds` includes drain time and reported load throughput
uses that full phase duration.
`--deadline-seconds` is a separate whole-run wall deadline covering setup, load
and drain, cleanup, and report writing.
At most 32 physical MCP sessions serve up to 256 logical sessions, with each
shared session serialized. Channels
are `mcp` for an ordinary 2025-11-25 Streamable HTTP JSON-RPC request,
`owner_http` for the exact authenticated native source-command request object,
`sdk` for an explicit absolute native source-command process, or `projection`
for the selected native source-projection CLI. Projection operations record the
owner's durable `queue-age` result, including sequence lag and explicit status
for historic rows. SDK receives one bounded JSON line on stdin and returns
response bytes on stdout. The harness drains stdout and stderr concurrently;
stderr is capped at 4096 bytes and only its declared prefix is used as an
oracle. Reports keep diagnostic byte counts and hashes without copying
diagnostic text. The input writer and bounded output drains run concurrently so
a full-duplex exchange cannot deadlock on full pipes. A deadline or output-cap
breach terminates the owned process group and records the operation as an error.

For a conflict race, set the same `conflict_group` on at least two owner HTTP
operations in one wave and give every member the same `alternate_outcomes` set,
containing one 2xx outcome and one signed 409 outcome. Each outcome may use an
exact response digest or a full semantic body. The group passes only when
exactly one member succeeds and all remaining members return the signed 409.
The SDK channel supports the same wave shape, with native exit 0 as success and
the ordinary source-command `JournalConflict` error as conflict. Every
scheduled owner command ID must still be unique. `retry_same_command_id` also
works for SDK: the runner discards the first completed process response and
replays the exact request and command ID in a new native process, so it measures
the owner's durable replay behavior rather than an HTTP retry.
`retry_same_command_id` deliberately closes after signed response headers and
retries the identical command body and ID with a fresh transport nonce for
owner HTTP.
`reconnect_before` deletes and reinitializes the MCP session before the
operation. When a cloned fixture path changes the selected configuration
digest, `bind_expected_configuration_from_owner` refreshes only that exact
precondition from the selected native owner's read-only `describe` response
before the measured waves; this creates no authority or admission.

For fresh native PostgreSQL source load stages, put ordinary
`source.bootstrap`, `prepare-create`/`prepare-revise`, and their
`source.create`/`record.revise` commit requests in the serial `setup` array.
Keep `rights.revoke` out of setup and measured sessions so measured writes and
their replay complete before current-use rights are revoked. The first setup
operation must be
`source.bootstrap`, with an initial job fence and absent CMD.2 schema. The
baseline must contain a private empty owner store, and every SDK invocation in
that run must bind to that same store through its source selection or the
rights owner root. The runner clones the baseline to a fresh
`owner-stage/`, requires `TOS_SOURCE_PG_URL` privately in the environment, and
executes the existing native owner entry; it does not accept SQL or create a
database. The total copied file bytes default to 268,435,456; when a measured
baseline needs more space, pass `--owner-stage-clone-cap-bytes N` with a positive
value up to 1,073,741,824. The option applies only to this invocation's private
owner-stage copy, requires `--owner-stage-baseline`, and its selected cap is
reported in `owner_stage_clone.bounded_copy_bytes`. For example,
`--owner-stage-clone-cap-bytes 536870912` selects a 512 MiB cap. The 1 GiB
ceiling bounds this load profile's temporary clone; it is not a corpus-size or
admission limit. The existing 32 MiB per-file, 16,384-file, and finite-host
resource limits still apply. Select a larger value only after measuring the
entire baseline and confirming the finite-host reservation can accommodate it.
Bootstrap provisions only an absent CMD.2 schema and refuses if that
namespace already exists. Give every run a dedicated disposable PostgreSQL 16
database plus a new output directory and reservation. The runner never resets
or drops PostgreSQL data; it retains the staged store and report under the
private output directory, while the caller owns database cleanup.

`--pg-stage-mode existing` attaches to an already bootstrapped source store in
the caller's disposable PostgreSQL database. Begin serial setup with an exact
successful native `describe` oracle; `source.bootstrap` is refused in every
phase. Put the private invocation, owner, token and projection selection files
inside `--owner-stage-baseline`, with the existing source store outside both
that baseline and the fresh output directory. Only the owner inputs are
cloned. HTTP, SDK and projection inputs must select the same canonical store,
domain and source contract; each role may carry its own acquired lease. Native
execution checks lease liveness. The caller must keep its database and host
admission alive through the complete load run and subsequent dump/cleanup.
Omitting `--pg-stage-mode` retains fresh-stage behavior.

For live PostgreSQL text search over MCP, select
`--source-text-search-selection ABS` and schedule `tos_source_text_search` in
existing-stage mode. A search-only MCP workload needs no `--model` or
`--binding`; supply that pair for other MCP operations and also
`--source-root`/`--source-inputs` for prepared source reads. Search responses
must be HTTP 200 with the matching JSON-RPC ID, a result containing a content
array, no RPC error and no `isError: true`, in addition to the declared exact
response oracle. The PostgreSQL URL stays in the private environment.

A commit may declare `bind_expected_from` with the label of an earlier
`prepare-create` or `prepare-revise` setup operation. The commit must use that
preview's invocation and exact proposal fields. The runner copies the preview's
owner-configuration, source/revision predecessor and dependency digests into
the commit request immediately before invoking the ordinary native entry. This
keeps dynamic preconditions bound to the staged owner and source state without
adding a second command implementation. Setup is serial and its result is
reported separately; load waves begin only after all setup operations pass.

For native PG `record.revise`, a typed
`tos_load_native_pg_record_revise_semantic_oracle_v1` body binds the result
schema, domain, source path, command ID and previous revision to that exact
preview. The runner verifies the actual request, materializations, receipt,
durable commit and authority fields; postload exact reads verify the new
source bytes. Replay must preserve the original successful response apart
from its two replay flags and bounded per-invocation source-work counters.
This oracle does not use `retry_same_command_id`;
use the separate postload replay check.

SDK `source-projection` reads must use the selected owner binary and the same
PG store, domain and source contract as the other clients. Exact-read oracles
keep schema, source identity, revision, byte length, SHA-256 and base64 content
exact while validating the three source-work counters as bounded integers.
The report counts native PG SDK integrity separately from prepared-source
vector checks, and also reports the total selected-source integrity checks.

The `projection` channel invokes the existing native `source-projection`
process for `init`, `status`, `apply`, `search`, `search-text`,
`exact-source`, or bounded `advance-navigation`. Text operations use the
`tos_source_projection_selection_v1` selection. Navigation uses the exact
protected `tos_local_source_navigation_owner_v1` selection and its nested
producer limits. A navigation schedule pairs one `bootstrap-navigation` as the
final serial setup operation with one or more measured
`advance-navigation` requests using the same owner file; bootstrap is never a
repeated load operation. Keep it after source bootstrap and fixture setup so
the measured advance starts from that exact baseline. Each measured navigation
request must use `max_events: 1`, stay within the selected producer limit, and
pass the ordinary installed CLI. The runner validates each declared navigation
outcome: one contiguous published range within `max_events`, an already-current
idle response, a replayed range, or a `more_pending/event_budget_exceeded` prefix
that advances the cursor while leaving residual lag. Missing event proof,
an event that cannot fit the first-item budget, malformed ranges, unknown tags,
and cursor movement outside the declared bound fail the operation. It records
sequence lag before and after each request; a
published pending prefix is a valid load observation and does not claim that the
projection is current. After all scheduled workers finish,
the runner performs a separate quiescent postload drain through the same
protected selection, with `max_events: 1` per call. It requires the source head
to remain stable, each call to advance the cursor, and the final cursor to reach
that head; the drain is bounded by the overall deadline and the finite step cap.
Its work is reported separately from load throughput and channel mix.
Navigation exposes no queue age, so the report marks age as unavailable instead
of estimating it. Text projection operations continue to report the owner's
queue-age result.
An `apply` entry may set `interrupt_resume_after_ms`; the runner
then checks the durable cursor before and after terminating the process,
requires a partially advanced cursor still below the source head, resumes the
same apply request, and verifies that status becomes `Current`. This requires
at least two pending events and `max_events` of at least 2 so the interrupted
stage can prove persisted intermediate progress. The runner stops only its own
child process groups and retains the staged files and report for inspection.
Only one interrupting projection operation is allowed per schedule, with no
other projection process in its wave, so another worker cannot advance the
cursor and stand in for the interrupted process.
The final response uses a semantic progress check because the interruption
position varies: the exact progress schema and work-counter fields must match,
the applied event count must equal the resumed cursor advance and remain within
`max_events`, and the response must agree with the final durable cursor.
This entry omits a static expected body or digest and does not accept alternate
outcomes or ignored JSON fields.

The optional serial `postload` list runs after measured session workers and the
quiescent navigation drain. Its checks do not enter load throughput, latency
percentiles, operation-count limits or channel-mix percentages; they do count
against the whole-run deadline and must pass for a zero exit. A native PostgreSQL
stage requires an exact SDK source read through either the prepared `source read`
entry or the selected native owner `source-projection --selection ABS` entry,
one exact replay of a successful measured SDK mutation, and this rights
sequence, in order:

1. A native rights `describe` with `bind_expected_configuration_from_owner:
   true` and `expected_rights_allowed: true`; the runner binds the selected
   owner configuration and structurally checks the current rights state.
2. `rights.revoke` with `bind_expected_from` set to that describe's label and
   both `expected_configuration` and `expected_rights_version` set to `null`.
   The runner binds both values from that successful response and verifies the
   native revoke receipt, command ID, rights-version increment, revoked state,
   and non-admission scope.
3. A second rights `describe` with the same owner-configuration binding and
   `expected_rights_allowed: false`.
4. A read through the selected `source-projection` CLI using `search`,
   `search-text`, or `exact-source`, with expected status `2` and
   `expected_stderr_prefix` set to `source-projection refused: current rights
   revoked`. This proves the stale projection read gate refuses the revoked
   state; the runner does not issue a queue-age probe after this refusal.

The exact source read and mutation replay run before the first rights describe
and revoke, so they establish source integrity and idempotency while rights are
still enabled. This route tests the CMD-local current-use bit and the projection
reader that enforces it. It does not claim that ordinary `source.read` is denied
by this separate projection-only revocation. The replay sets
`idempotency_replay_of` to the successful mutation's `command_id` and repeats
its request, invocation, setup binding and response oracle. The runner executes
that replay only if the original mutation completed successfully; it does not
count the replay as load. No postload operation admits raw SQL or an alternate
owner entry.

Postload entries emit separate `postload_check` records with the check kind,
expected and actual rights state when applicable, and exact response evidence.
The runner refuses to start the fixture `rights.revoke` if any preceding
postload verification failed or if the required exact-source read, PostgreSQL
source integrity read, successful mutation replay, or rights-enabled describe
did not pass. The revoke record is reported as not started; no fixture rights
state is changed in that case. Its receipt's `request_sha256` must equal the
`SourceCommandInputV1` canonical record digest of the exact bound request. The
projection denial is reported as a passed refusal while remaining outside the
count of successful commands.
`load_resource_delta` brackets only measured session work, while the existing
`resource_delta` remains the full process interval including setup, projection
drain and postload checks. Both use the launch unit's cgroup; record PostgreSQL
service I/O and database storage separately when the database runs outside that
unit.

For a small two-session smoke, use
`access/integrations/abyssos/fixtures/load-readiness-two-session.json`. It is a
finite exact-oracle schedule for one MCP `ping` in each session, using the
native handler's deterministic `{"result":{}}` response. It needs no content
specific to the selected model or binding and does not carry a source-writing
request. This checks MCP transport and session setup only; it is not evidence for
the new PostgreSQL source/navigation/write path. The source fixture must remain
on the selected reviewed checkout.

For a runnable source-read workload and synthetic owner race, the
`export_protected_native_load_readiness_fixture_when_selected` conformance test
exports a private owner-stage baseline and a 256-session schedule. It uses the
committed prepared Metadata predecessor and native composite Forms fixture,
issues a metadata-only source handle through the selected `tos` product, and
captures exact MCP response bodies. It preflights the synthetic Forms owner on
a separate temporary clone; the exported baseline remains pristine. Select a
fresh private export directory, installed `tos`, native owner command, and
exact schema worker before running:

```sh
TOS_LOAD_READINESS_EXPORT_DIR=/ABS_PRIVATE_EXPORT \
TOS_LOAD_READINESS_EXPORT_QUERY_BINARY=/ABS_INSTALLED_TOS \
TOS_NATIVE_OWNER_COMMAND_PATH=/ABS_INSTALLED_NATIVE_OWNER_COMMAND \
TOS_SCHEMA_WORKER_PATH=/ABS_TOS_SCHEMA_WORKER \
cargo test -p tos-conformance --test conformance \
  command_metadata_publication_cases::export_protected_native_load_readiness_fixture_when_selected \
  -- --exact --nocapture
```

The exporter writes `owner-stage-baseline/`, `load-readiness-schedule.json`,
and `load-readiness-fixture.json`. Use the baseline's `metadata-source` paths
for `--source-root`, `--source-inputs`, `--model`, and `--binding`, and its `forms`
paths for `--owner-config` and `--invocation`; use
`owner-stage-baseline/synthetic-http-token` for `--token-file`. Pass the entire
baseline with `--owner-stage-baseline`. Run the same schedule at `--sessions 2`,
then 8, 16, 64, 128, and 256, with a fresh private output directory and
reservation for each launch. Every invocation clones that baseline, rebases the
prepared source-input roots and their bound SQLite row, and discards its
mutated owner clone with the output. The schedule includes indexed search,
native source-handle discovery and exact metadata-record return, an SDK
read-only owner `describe`, one synthetic shared Forms apply race, same-ID
lost-response retry with fresh nonces, and an MCP reconnect. It carries no
production grant, credential, source record, or R2 endpoint.

For the first two-session run, resolve the generated fixture paths as below and
use distinct selected values for `OUTPUT`, `UNIT`, and the outer resource class,
memory and timeout fields. The host resource owner must precreate the private
output and reservation for that unit before launch:

```sh
abyss-machine resource launch --class CLASS --kind benchmark \
  --memory-demand-mib MEMORY --bytes OUTPUT_CAP --target OUTPUT \
  --unit UNIT --timeout OUTER_SECONDS --json -- \
  ABS_LOAD_READINESS \
  --query-binary ABS_INSTALLED_TOS \
  --model EXPORT/owner-stage-baseline/metadata-source/derived/prepared.sqlite \
  --binding EXPORT/owner-stage-baseline/metadata-source/prepared-binding.json \
  --source-root EXPORT/owner-stage-baseline/metadata-source \
  --source-inputs EXPORT/owner-stage-baseline/metadata-source/source-inputs.json \
  --schedule EXPORT/load-readiness-schedule.json \
  --owner-stage-baseline EXPORT/owner-stage-baseline \
  --owner-binary ABS_INSTALLED_NATIVE_OWNER_COMMAND \
  --owner-config EXPORT/owner-stage-baseline/forms/owner.json \
  --invocation EXPORT/owner-stage-baseline/forms/invocation.json \
  --token-file EXPORT/owner-stage-baseline/synthetic-http-token \
  --output OUTPUT --unit UNIT --sessions 2 --deadline-seconds 120 \
  --request-cap-bytes 1048576 --response-cap-bytes 4194304 \
  --schedule-cap-bytes 16777216 --output-cap-bytes OUTPUT_CAP
```

Repeat with `--sessions 8`, 16, 64, 128, and 256 after reviewing the
two-session report. Give each launch a new output directory, unit and matching
host reservation; `EXPORT` remains the same pristine fixture baseline.

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
requested channel percentages and scheduled/started/passed/failed/not-started
counts by channel, load-phase latency and completed-operation throughput,
text-projection sequence lag and owner-reported oldest pending age, navigation
sequence lag before/after bounded advances, cgroup CPU/memory/physical-I/O deltas, memory pressure
events, actual owner-stage clone bytes, and report allocation. Historical queue
events without an enqueue time remain `unknown_historical`; the runner does not
estimate their age from client time. The catalog preflight records only handler
count and digest, not a copied function catalog.

Select scale dimensions independently. `--sessions` chooses the supported
concurrency stage (2, 8, 16, 64, 128, or 256). `--operation-count N` caps the
measured schedule operations and selects them in round order across those
sessions. `--channel-mix` requests a channel percentage split while preserving
each session's order; `--operations-per-second N` paces combined load dispatch
across all channels. `--load-window-seconds N` limits the time for starting new
operations; the runner drains work already in flight afterward, so the reported
load phase can exceed that window. The whole-run `--deadline-seconds` includes
setup, dispatch, drain, cleanup, and reporting. The explicit schedule controls
the scenario mix. Vary source data volume in the separate protected
`capacity-fixture` preparation with `--target-records N`; that input-size choice
does not change the schedule operation count, dispatch rate, or requested
channel mix.

Before each stage, record its selected source-record target, operation count,
duration, dispatch rate, channel mix, and host reservation. Stop the sequence on
the first nonzero runner exit, an incomplete load window, an unexpected
response/conflict invariant, any increase in cgroup `memory.events` pressure
(`high`, `max`, `oom`, or `oom_kill`), or—when cgroup `memory.max` supplies a
finite limit—peak memory reaching 90% of that limit. If the limit is absent or
unbounded, the 90% threshold is reported as unavailable; do not infer it from
the outer reservation (`memory_headroom_threshold_available` is false and
`memory_peak_at_or_above_90_percent_of_limit` is null). The helper marks that
stage failed when any listed stop condition occurs. Keep the JSONL report
and exact invocation for diagnosis. Do not advance to a larger session stage
after a stop. Recover a
failed fresh-PG attempt from a new private stage copy and an empty disposable
database selected for that attempt; retain the pristine baseline and the failed
report, and leave database cleanup to its owner.

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
