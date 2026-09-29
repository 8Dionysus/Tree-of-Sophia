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
The existing Python command entry now selects this native mode; the retained
Python API remains an independent oracle. This validates mechanics topology
and does not accept authored ToS meaning.

`tos-validation-lanes` is a separate candidate for the current
`scripts/validation_lanes.py` command plane. Its `--check`, `--sequence ID`,
and `--run ID` modes read the existing
`docs/validation/validation_lanes.json` in authored order. Selection and run
require `--python PATH`, which the compatibility entry passes as
its exact `sys.executable`; the native binary does not discover or install an
interpreter. The read is bounded to 1 MiB. Run uses the existing dedicated
Linux pidfd/subreaper executor with its default 300-second command wall,
3600-second sequence wall, one-second cleanup grace, and 16 MiB combined
output per child. These finite execution limits are stricter than the Python
runner's prior unbounded subprocess call. A child exit code is returned
unchanged; a signalled child is printed with Python's negative signal status
and returned as the corresponding Unix shell status. The imported Python loader and
`release_check` API remain available; command execution selects the native consumer.
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
sequence. The existing release command entry selects the native consumer. This source
wiring does not claim an actual release or repository CI run.

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
skipped jobs. It cannot run checks or accept a release. The existing Python command entry
selects this native consumer; the `.github` workflow retains its command route.

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
this native mode does not write or consume a cache. The existing command entry
selects native validation by default and retains the Python API for explicit
`--feedback-cache` requests.

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

`tos-route-cards --repo-root ABSOLUTE_PATH build [--check] [--output PATH]`
and `validate` are explicit candidates for the maintained
`build_agents_route_currentness.py` and `validate_nested_agents.py` consumers.
`tos-agents-route-harness --repo-root ABSOLUTE_PATH [--check] [--output PATH]`
retains the maintained declared-task harness, including `--source-ref` and
`--volatile-timing`. Their shared snapshot reader preserves raw digests,
Python16 text rules, target inheritance and a separate owner handoff. Native
currentness keeps `generated_by="scripts/build_agents_route_currentness.py"`
as a compatible format marker; this value does not prove that Python ran or
identify the executable that produced those bytes. Authored cards and inventory
retain their authority, and harness results make no model-behavior claim.

One source snapshot bounds raw input to 64 MiB / 8 MiB per file, with at most
another 64 MiB of normalized cached text, 10,000 entries/files, 100,000 lookups,
128 path levels and 4096-byte relative paths. Cached metadata is computed once
per unique source. Inventory punctuation is preflighted before JSON parsing;
arrays/tasks are limited to 4096 and cards to 65,536 lines of at most 8192 bytes.
The snapshot checks a cooperative 30-second deadline. Each build/validator Git
capture uses the existing Linux pidfd/subreaper executor with a 10-second wall,
one-second cleanup grace and 4 MiB combined output. Harness provenance uses at
most two such captures. Supervisor or finite-bound refusals cannot become clean
validation; ordinary unavailable Git retains the maintained conservative route.

The shared conservative output budget limits amplified fields before copying
to 16 MiB; rendering, output and currentness checks have the same byte ceiling.
Explicit writes retain nofollow directory descriptors and refuse symlinks,
FIFOs and shared hardlinks before truncation. Diagnostics are limited to 4096
findings of at most 8192 bytes. Harness additionally charges repeated cached
lower/search/token scans to a 512 MiB logical-work ceiling and limits route
text and joined prompts to 8 MiB. These are finite candidate limits and can
refuse inputs the Python scripts previously read without limits.

Caller costs include a second inventory snapshot for default build output,
a second discovery walk during currentness verification and successful
validator CLI recount, plus an existing-output read for `--check`. Rendering
and local writes need an outer wall guard. The disposable native/Python cases
exercise those real callers; the Python scripts, route-docs lane and actual
generated carriers remain active pending coordinated owner cutover.

The accepted validation-lanes, release-check, software-CI, topology and default
active-naming command entries replace themselves with selected installed tools.
`TOS_VALIDATION_LANES_EXECUTOR`, `TOS_RELEASE_CHECK_EXECUTOR` and
`TOS_SOFTWARE_CI_EXECUTOR` select their corresponding standalone binaries;
`TOS_OPS_MECHANICS_EXECUTOR` selects topology and naming modes. Each entry uses
its exact override or the corresponding PATH binary, without compile-on-call
or fallback. Existing argparse spelling, interpreter adapter, environment and
exit status are preserved. Imported Python APIs and the optional naming cache
remain available. OPS owns installation and packaging availability independently
from the prepared read-model fs-verity route; source wiring alone proves neither
installed availability nor an actual release/CI execution.

`--semantic-registry-transition` is an explicit native gate for the maintained
semantic registry transition law. `--baseline-commit` supplies the immutable
baseline; otherwise `TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT` applies. Initial
introduction requires `--allow-initial-introduction` or the exact environment
value `TOS_SEMANTIC_REGISTRY_ALLOW_INITIAL_INTRODUCTION=1`; the environment accepts
only `0` or `1`. `--json` emits the maintained result shape. These flags require
this mode. The existing Python executable entrypoint selects
`TOS_OPS_MECHANICS_EXECUTOR` or installed `tos-ops-mechanics-plan` on PATH and
replaces itself with this mode; no build or Python fallback occurs. Imported
`validate_transition` and `main` remain available as reference APIs.

The existing 13-case `tests/test_semantic_registry_transition.py` suite selects
an explicitly retained native image with `TOS_SEMANTIC_REGISTRY_TEST_EXECUTABLE`.
Without that test selector it preserves its Python reference route. The native
route keeps baseline, replacement, ancestry, profile/version and historical
schema assertions, using actual shallow Git metadata rather than a Python mock.
Its three CLI assertions traverse the Python executable wrapper and native exec.
Strict duplicate-key refusal accepts the backend's declared diagnostic wording;
this does not assert general malformed-JSON diagnostic equality.

The gate holds root/parent descriptors and reads four current regular nofollow
files, each at most 1 MiB, plus four exact Git-baseline members. The Foundation
JSON profile retains depth 64, 300,000 visits and 4,300 integer digits. The
64 MiB logical decoded-state envelope includes current/baseline values,
conservative rule indexes and issue storage, with 4,096 issues and a 1 MiB
issue cap. Raw/canonical buffers and schema backend allocations are separate
whole-process costs. Git output is capped at 16 MiB total, each query at most
2 MiB combined, and the final report at 1 MiB. Command/whole walls are at most
30/300 seconds; public limits only tighten them. It performs no fetch, executes
no baseline code, and makes no source, rights or semantic admission.

`--source-home` is an explicit native candidate for the maintained
`validate_tos_source_home.py` law. It checks core source-branch membership, stable
IDs, owner surfaces, lane references, source-home README fragments and absent
legacy root surfaces using the retained route reader. It reads the source-home
manifest, validation-lane manifest and README; JSON uses the bounded Foundation
legacy last-wins profile. Existing reader limits apply: 8 MiB per text file,
64 MiB raw and 64 MiB normalized caches, 10,000 entries, 100,000 operations and
30-second snapshot wall. Diagnostics cap at 4,096 issues and 1 MiB rendered text.
The Python lane and imported APIs remain active pending actual consumer acceptance.

`--philosophy-topology` selects only the native candidate for the maintained
philosophy topology validator. The existing Python executable and registered
lane remain unchanged. This candidate checks manifest/packet/branch boundaries,
planting schema and exact atlas/backlog/Work/Collection membership, branch
planting references/counts and metadata labels in all ToS descendant paths.
It uses the existing retained route reader and its shared limits, including
30-second operation deadline and 10,000 visited entries across the three
overlapping enumerations. It may refuse a larger namespace; this is no full
corpus or scaling claim. Cached source bytes are reused, but atlas JSONL rows
are evaluated for each planting as in the maintained function.

The schema format profile follows the existing LegacyPythonObserved optional
format behavior: date-time, uri and uri-reference do not assert; other enabled
formats retain their backend checks. Finite rule diagnostic parity does not
claim arbitrary malformed JSON/schema exception wording equality. Root source
review and actual four-state consumer evidence remain separate from source
wiring or formatter success.

`--philosophy-graph-views-validate` is the explicit native candidate for the
existing graph-view catalog validator. It reads the generated atlas carrier
and uses the existing `tos-compiler` view builder; it does not build an atlas.
Expected and current schema checks precede Python sorted compact value equality
and the maintained lens/boundary checks. The schema profile has no format
assertions. Duplicate JSON members keep the last decoded value at this reader
seam before normalization for the native builder. Existing Python lane remains
unchanged. The local compiler dependency and this candidate need separate
whole source/cost review; prior philosophy runtime grants do not include it.

The graph-view validator reads its derived atlas operand with a separate
128 MiB cap and the current graph-view catalog with the existing builder's
16 MiB output cap. Both use retained nofollow descriptors through byte-only
reads, own exact raw-byte SHA-256 observations and avoid authored text caching.
Authored/source docs keep the existing route reader and builder bounds. These
derived byte observations grant no source, canon or runtime authority. Raw
bytes, Foundation parse state, canonical bytes and serde DOM coexist during
normalization; atlas DOM then coexists with builder indexes/source DOM/output.
The separate limits preserve the whole input contract, without asserting RAM
fit or full corpus capacity from a small controlled consumer.
