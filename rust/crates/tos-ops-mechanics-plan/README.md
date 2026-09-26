# Mechanics-local command planning and execution

`tos-ops-mechanics-plan` discovers package-local and part-local mechanics
tests, builders and validators. The default CLI still emits exactly one
`tos_mechanics_local_plan_v1` JSON line: `test_file_count` plus commands with
`kind`, `home` and `argv`. Order remains unittest-discover per test home,
then builders `--check`, then validators; homes and names sort by path.

`--repo-root ABSOLUTE_PATH` is required. `--python COMMAND` selects the
explicit interpreter adapter (default `python`; choose an exact interpreter
to match the former Python runner's `sys.executable`). `--execute` instead
runs that same plan from the repository root, printing the progress lines and
final success line used by `scripts/run_mechanics_local_tests.py`. The first
nonzero child stops the lane with exit 1, as the former Python runner did;
child stdout and stderr retain their streams. Diagnostic traceback wording
and cross-stream interleaving are not compatibility promises. Plan output
and discovery behavior are unchanged.

Execution has explicit limits, optionally lowered or raised within the hard
ceilings using these flags:

| Flag | Default | Hard ceiling |
| --- | --- | --- |
| `--command-timeout-ms` | 300000 | 3600000 |
| `--lane-timeout-ms` | 3600000 | 86400000 |
| `--cleanup-grace-ms` | 1000 | 2000 |
| `--max-output-bytes` | 16777216 | 67108864 |

All limits must be positive. Output is a combined stdout/stderr byte ceiling
per child; streaming uses fixed-size buffers and nonblocking child and sink
FDs. Deadline/cancellation checks also bound a blocked output sink. Cleanup
adds at most its declared grace beyond an execution deadline. Limit or
custody failure exits 1; SIGINT/SIGTERM cancellation exits 130/143 after
cleanup. A saturated error sink can omit the final diagnostic.

Execution requires a dedicated single-threaded Linux process without other
children, available `/proc`, `PR_SET_CHILD_SUBREAPER`, and pidfd syscalls. It
probes both pidfd open and send-signal (signal0 to itself) before any tool
starts, and fails closed if those facilities are unavailable. Each tool gets
its own process group and a parent-death signal. The supervisor becomes a subreaper,
attempts to terminate and reap ordinary and escaped/session-changing
descendants by pidfd after root exit, timeout, output overflow or cancellation.
Successful cleanup requires both the root reap and a complete empty direct-child
scan. Numeric PGIDs are signalled only before their root is reaped. Cleanup is
bounded: any grace exhaustion, enumeration, permission or reap failure returns
failure and residual/unknown custody evidence for host handoff. Even ordinary
interruptible descendants may remain when a large family exceeds the grace;
kernel-uninterruptible children are one possible residual, not the only one.
PID listings stream through an 8KiB buffer under the cleanup deadline; there
is no descendant-count or listing-size cutoff before cleanup. A per-PID
failure is retained while cleanup continues over later descendants. At deadline,
diagnostics include the root identity/reap state and a bounded residual sample.
That sample may be partial or empty after an incomplete scan; it never proves
that no descendants remain. This is custody of trusted local tools,
not a hostile-code sandbox. SIGKILL/crash of the supervisor itself, processes
outside its ancestry, and malicious same-user interference require external
host supervision. No host services or privileges are configured by this tool.

Discovery visits only immediate `mechanics/*` and `mechanics/*/parts/*`
homes, rejects encountered symlinks and caps scanned entries/commands. The
synthetic oracle fixture protects ordering; the real CLI fixture protects
stop behavior, output/cancellation deadlines, ordinary/escaped descendants,
successful daemon cleanup, and syscall-unavailable refusal before tool start.
A synthetic PID stream covers the prior count/byte cutoff without spawning
thousands of live processes. No authored ToS meaning is admitted.

`--execute` replaces only the runner mechanism. The discovered tools still
invoke Python; this does not complete their Rust migration. The compatibility
entrypoint `scripts/run_mechanics_local_tests.py` replaces
itself with the installed executor from `TOS_OPS_MECHANICS_EXECUTOR` or PATH,
passing `sys.executable` as the Python adapter and the explicit default limits.
It fails when that native binary is unavailable; it does not compile on demand
or fall back to the old Python runner. The named lane can retain its existing
entrypoint command. OPS owns installation and CI availability before cutover.
The former discovery oracle remains at the pre-executor source commit.

Install with `cargo install --locked --offline --path
rust/crates/tos-ops-mechanics-plan --root <admitted isolated install root>`
from the integrated workspace. The existing native fixture may target that
installed executable via `TOS_MECHANICS_TEST_EXECUTABLE`; without the variable
it targets Cargo's built CLI. This compares the installed candidate using the
same lifecycle/ordering risks, without another test framework.
