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

`--execute` replaces the runner mechanism. The current plan keeps four Python
unittest homes as independent safety oracles and the release-support artifact
bundle validator under its stronger owner. Agon threshold registry, relation
pack, Questbook, public mirror and Derived KAG checks use the native executable;
mirror and KAG generation remain explicit opt-in actions, never implicit lane
writes. This does not make the entire mechanics lane Rust-only. The compatibility
entrypoint `scripts/run_mechanics_local_tests.py` replaces
itself with the installed executor from `TOS_OPS_MECHANICS_EXECUTOR` or PATH,
passing `sys.executable` as the Python adapter and the explicit default limits.
It fails when that native binary is unavailable; it does not compile on demand
or fall back to the old Python runner. The named lane can retain its existing
entrypoint command. OPS owns installation and CI availability before cutover.
The former discovery oracle remains at the pre-executor source commit.

`--mechanics-topology-validate` is a separate read-only native candidate for
the existing `mechanics_topology` lane. It checks package/part membership,
route documents, local Markdown references and fragments, script/test
inventories, context budget and moved-path accounting. It bounds traversal to
10,000 entries, retained input to 64 MiB (8 MiB per file), and diagnostics to
4,096 issues of at most 8 KiB each; references have separate 100,000-entry
and 64 MiB byte bounds, and retained anchor text has its own 64 MiB bound.
The Python lane remains the independent
blocking oracle until exact issue-order parity and the actual native consumer
are accepted. This candidate does not inspect or change authored ToS meaning.

`tos-validation-lanes` is a separate candidate for the current
`scripts/validation_lanes.py` command plane. Its `--check`, `--sequence ID`,
and `--run ID` modes read the existing
`docs/validation/validation_lanes.json` in authored order. Selection and run
require `--python PATH`, which the eventual compatibility entry must pass as
its exact `sys.executable`; the native binary does not discover or install an
interpreter. The read is bounded to 1 MiB. Run uses the existing dedicated
Linux pidfd/subreaper executor with its default 300-second command wall,
3600-second sequence wall, one-second cleanup grace, and 16 MiB combined
output per child. These finite execution limits are stricter than the Python
runner's prior unbounded subprocess call. A child exit code is returned
unchanged; a signalled child is printed with Python's negative signal status
and returned as the corresponding Unix shell status. The Python loader and
`release_check` import remain active until the owner admits route cutover.
As in the existing executor, an `execvp` refusal becomes child status 127;
the Python runner previously raised an unhandled spawn exception instead.

`tos-release-check` is a distinct explicit consumer of the same manifest's
`release_check` sequence. `--phase all` keeps authored order; `checks` and
`tests` require exactly one final `run tests` step and select before or at that
step respectively. It takes an exact `--python PATH` adapter and preserves the
maintained runner's `PYTEST_DISABLE_PLUGIN_AUTOLOAD` default, Windows-style
`list2cmdline` progress text, first failure line on stdout, and child status.
The same native executor imposes the finite command, sequence, cleanup, and
output limits above; this differs from Python's unbounded subprocess call.
Its controlled fixture runs only a temporary three-step release-shaped
sequence. The actual Python release entry, CI calls, and real release sequence
remain active and unexecuted by this candidate until owner acceptance.

Install with `cargo install --locked --offline --path
rust/crates/tos-ops-mechanics-plan --root <admitted isolated install root>`
from the integrated workspace. The existing native fixture may target that
installed executable via `TOS_MECHANICS_TEST_EXECUTABLE`; without the variable
it targets Cargo's built CLI. This compares the installed candidate using the
same lifecycle/ordering risks, without another test framework.

`tos-software-ci` is the next explicit candidate for the maintained
`scripts/software_ci.py` whole selector. `plan --repo-root PATH --base REF`
reads the actual Git no-renames changed paths, validates only new local Markdown
links and merge markers, and emits the same v2 selection and optional
`GITHUB_OUTPUT` fields. `--full` and unknown/shared changes require all checks.
`gate` reads `CI_NEEDS` and rejects missing, failed, cancelled and unexpectedly
skipped jobs. It cannot run checks or accept a release; the Python entry and
`.github` workflow stay active pending whole owner acceptance.

Git capture uses the existing dedicated Linux pidfd/subreaper boundary, capped
at 30 seconds per command and 120 seconds for the plan. Input is limited to
4096 changed paths / 4 MiB, 8 MiB per Markdown or prior Git document, and
64 MiB across changed paths, current documents and captured Git output. Current
Markdown and `GITHUB_OUTPUT` must be regular files without a final symlink;
`CI_NEEDS` and the existing append destination are capped at 1 MiB. These are
explicit finite candidate limits. Git error text/exception tracebacks are not
promised byte-identical; selection, successful output and document findings
retain the maintained semantics within this profile. No remote fetch occurs.

`--active-naming-validate` is the read-only default-route candidate for
`scripts/validate_active_naming.py`. It preserves pruned top-down/sorted path
checking, token-first maximal runs, exact content-only domain/provenance
exceptions, experience route scope and active fields of mechanics topology.
Generated KAG carriers and retired history remain outside the active naming
source. Python's optional external SQLite feedback-cache route stays intact;
this native mode does not write or consume a cache and does not replace that
optional consumer or switch the authored lane.

This candidate refuses encountered active symlinks, bounds traversal to 10,000
entries / 128 directory levels, each text file to 8 MiB and aggregate read input
to 64 MiB, and issues to 4096 of at most 8 KiB each. Invalid UTF-8 text is skipped
as in Python; ordinary newline decoding is preserved. JSON topology is bounded
by the existing serde parser depth and cannot use a depth refusal as clean
validation. Whole validation has a 300-second wall. One fixed bounded pass over
Unicode scalars prepares the digit grammar from the existing Unicode16 category
primitive, while lowercasing uses FND's pinned Python16 implementation. The
controlled fixture compares the actual whole CLI/Python default consumer on
failing, cleaned, and changed-current-target states of one small disposable tree.
These are finite candidate limits and controls, not a whole repository run or
optional-cache retirement result.
