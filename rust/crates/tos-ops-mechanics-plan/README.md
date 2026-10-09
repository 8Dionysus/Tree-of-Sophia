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
final success line. The first
nonzero child stops the lane with exit 1, as the former Python runner did;
child stdout and stderr retain their streams. Diagnostic traceback wording
and cross-stream interleaving are not compatibility promises. Plan shape and ordering are preserved; native owner homes no longer need interpreter files.

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

`--execute` runs the native Growth pipeline and other selected mechanics.
Agon registry, relation pack, Questbook, public mirror and artifact bundle
routes bind directly to their native owner flags by package/part home. The
route survives removal of Python wrappers; unexpected Python builders or
validators in those homes fail discovery. Artifact validation retains the
source-owned runtime integration pause and uses the selected external owner
CLI when enabled. Mirror sync and KAG generation remain explicit writes.
The retained Growth Python assertions still require `--growth-python-oracle`.

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

`tos-validation-lanes` owns the validation command plane. Its `--check`,
`--sequence ID`, and `--run ID` modes read the existing
`docs/validation/validation_lanes.json` in authored order. Native sequences
require no interpreter. A selected retained Python step requires an explicit
`--python PATH`; the native binary does not discover or install an interpreter.
The read is bounded to 1 MiB. Run uses the existing dedicated Linux
pidfd/subreaper executor with its default 300-second command wall,
3600-second sequence wall, one-second cleanup grace, and 16 MiB combined
output per child. The first child failure stops the sequence and preserves its
exit status. A signalled child is reported by its negative signal status and
returned as the corresponding Unix shell status; executable lookup failure
returns status 127. The former Python loader, runner and compatibility entries
have been retired.

`tos-release-check` is a distinct explicit consumer of the same manifest's
`release_check` sequence. `--phase all` keeps authored order; `checks` and
`tests` select before or at the complete final suffix of named `run tests: `
steps, while retaining support for one legacy final `run tests` step. It takes
an exact `--python PATH` adapter only when the selected phase retains Python
steps, and preserves the maintained runner's `PYTEST_DISABLE_PLUGIN_AUTOLOAD` default, Windows-style
`list2cmdline` progress text, first failure line on stdout, and child status.
The same native executor imposes the finite command, sequence, cleanup, and
output limits above; this differs from Python's unbounded subprocess call.
Its controlled fixture runs only a temporary four-step release-shaped
sequence. The existing release command entry selects the native consumer. This source
wiring does not claim an actual release or repository CI run.

Install with `cargo install --locked --offline --path
rust/crates/tos-ops-mechanics-plan --root <admitted isolated install root>`
from the integrated workspace. The existing native fixture may target that
installed executable via `TOS_MECHANICS_TEST_EXECUTABLE`; without the variable
it targets Cargo's built CLI. This compares the installed candidate using the
same lifecycle/ordering risks, without another test framework.

`tos-software-ci` owns software check selection and result aggregation. `plan --repo-root PATH --base REF`
reads the actual Git no-renames changed paths, validates only new local Markdown
links and merge markers, and emits the same v2 selection and optional
`GITHUB_OUTPUT` fields. `--full` and unknown/shared changes require all checks.
`gate` reads `CI_NEEDS` and rejects missing, failed, cancelled and unexpectedly
skipped jobs. It cannot run checks or accept a release. The `.github` workflow invokes this native command directly.

Git capture uses the existing dedicated Linux pidfd/subreaper boundary, capped
at 30 seconds per command and 120 seconds for the plan. Input is limited to
4096 changed paths / 4 MiB, 8 MiB per Markdown or prior Git document, and
64 MiB across changed paths, current documents and captured Git output. Current
Markdown and `GITHUB_OUTPUT` must be regular files without a final symlink;
`CI_NEEDS` and the existing append destination are capped at 1 MiB. These are
explicit finite candidate limits. Git error text/exception tracebacks are not
promised byte-identical; selection, successful output and document findings
retain the maintained semantics within this profile. No remote fetch occurs.

`--active-naming-validate` preserves pruned top-down/sorted path checking,
token-first maximal runs, exact content-only domain/provenance exceptions,
experience route scope and active fields of mechanics topology. Generated KAG
carriers and retired history remain outside the active naming source.
The Python implementation and its test module are retired; Rust unit tests and
the actual CLI scenario cover naming and optional feedback-cache behavior.

On Linux, `--feedback-cache ABSOLUTE_SQLITE_FILE` selects an optional external
local cache. Every path and file is still read live; only the pure content
result is reused by digest. Policy binds the native source and resolved
Rust dependencies, so old Python hints are recomputed. The parent must be
owned and not writable by other users; the cache cannot be inside the source
root or reached through a symlink into it. New files use mode `0600`.
Reserve up to 128 MiB for the 64 MiB database and its rollback journal before
selecting this write route. Corrupt rows are recomputed; storage, lock and
schema failures fall back immediately to uncached validation and are reported.
CI and release commands do not select feedback caches.

The validator refuses encountered active symlinks, bounds traversal to 10,000
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
and `validate` are the maintained route-card builder and validator.
`tos-agents-route-harness --repo-root ABSOLUTE_PATH [--check] [--output PATH]`
retains the declared-task harness, including `--source-ref` and
`--volatile-timing`. Their shared snapshot reader preserves raw digests,
Unicode text rules, target inheritance and a separate owner handoff. Generated
currentness names `rust/crates/tos-ops-mechanics-plan/src/route_cards.rs` as its
builder source. Authored cards and inventory retain their authority, and harness
results make no model-behavior claim. The Python implementations and replaced
tests are retired; the native CLI contracts preserve structural refusals,
currentness, output paths, context budgets and source provenance.

One source snapshot bounds raw input to 64 MiB / 8 MiB per file, with at most
another 64 MiB of normalized cached text, 10,000 retained files/cards,
100,000 lookups, 128 path levels and 4096-byte relative paths. The maintained
whole-repository card, currentness and harness routes stream up to 131,072
physical directory entries without retaining unrelated files; narrower source
APIs keep their original discovery limits. Cached metadata is computed once
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

The accepted validation-lanes, release-check, software-CI and topology
command entries replace themselves with selected installed tools. Active naming
is invoked directly as `tos-ops-mechanics-plan --repo-root PATH --active-naming-validate`.
`TOS_VALIDATION_LANES_EXECUTOR`, `TOS_RELEASE_CHECK_EXECUTOR` and
`TOS_SOFTWARE_CI_EXECUTOR` select their corresponding standalone binaries;
`TOS_OPS_MECHANICS_EXECUTOR` selects topology mode. Each retained entry uses
its exact override or the corresponding PATH binary, without compile-on-call
or fallback. Existing argparse spelling, interpreter adapter, environment and
exit status are preserved for those remaining adapters. Active naming and its
optional feedback cache are native; other imported Python APIs remain pending
their corresponding retirement. OPS owns installation and packaging availability independently
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

`--witness-structure-validate` validates the five maintained text-free witness
structure families through Rust: Zarathustra part correspondences and three
proposed addresses per correspondence, parallel PDF divisions, source and
target numbered-unit maps, and shared numbered labels. It checks the owner
schemas with formats enabled, exact inventory and provenance digests, page
arithmetic, ordering, anchor closure and the recorded rights bindings. It reads
tracked metadata only and does not read private PDF or TEI payloads, assess
translations, or grant rights, review or canon status. The source-foundation
lane invokes this mode directly; its Python implementation is retired.

The held route reader bounds each input to 8 MiB, retained input to 64 MiB,
and the whole pass to 30 seconds. Diagnostics are capped at 4096 and 1 MiB.
The source-data integration test requires these explicitly selected tracked
metadata files and runs with `cargo test --locked --no-default-features -p
tos-ops-mechanics-plan --test witness_structure_native -- --ignored`.
The tests exercise the actual native entry with an empty PATH and preserve the
source-only `237a` and translation-claim refusal controls. They are outside the
software-only test set, which does not select a production witness corpus.

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
philosophy topology validator. The registered lane retains its command path;
the existing executable now selects this native mode through
`TOS_OPS_MECHANICS_EXECUTOR` or installed `tos-ops-mechanics-plan` on PATH,
with no build on call or Python fallback. An empty explicit selector refuses.
Imported `main` and `run_validation` remain reference APIs until final retirement.
This candidate checks manifest/packet/branch boundaries,
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

The default `compiler-backed-validators` feature includes the philosophy graph
view validator and its compiler-backed rebuild. Normal package installation
keeps that feature enabled. The three CI prerequisite executors
`tos-software-ci`, `tos-validation-lanes` and `tos-release-check` use
`--no-default-features`: their selector, command manifest and process executor
paths do not depend on `tos-compiler`. Such a build does not provide the
compiler-backed validator flag. This is a dependency boundary, not a measured
performance or whole-validator acceptance claim.

The same `tos-software-ci` executable owns the CI artifact operations
`executor-manifest`, `executor-bind`, `software-receipts`, and `software-limits`.
They retain the existing three-executor manifest and access/command build receipt
shapes consumed by native software packaging. Producer operations require the
successful same-run Cargo JSON artifact stream, exact executable package/path,
empty feature selection, native debug profile, and the selected Rust version
output. They hash held regular files with a fixed 64 KiB buffer and verify file
identity before returning. Receipt generation describes those completed products;
it does not establish independent trust in the producer.
`software-receipts` also binds the verified owner-command image through
`TOS_NATIVE_OWNER_COMMAND_BIN` before software fixtures run, alongside the
existing Access preparation bindings. External payload validation consumes
that native custody product; packaging and installation follow the fixtures.

The workflow authenticates the downloaded verifier and manifest with
`sha256sum` against outputs from the independent producer job **before** running
that verifier. Rust then checks the exact three-file executable set plus
manifest, current commit/tree/lock, toolchain/target/profile/features, and each
image's SHA and length before publishing GitHub environment/path bindings.
Transport, executable permission changes, and GitHub's environment files remain
platform operations. The removed inline Python artifact rules are replaced by
these native operations; the Python software-selection API remains reference
evidence for its separate maintained selection contract.

Each operation has one 120-second deadline and cancellation guard, including
its existing owned Git capture and final source check. Images are capped at
1 GiB each; metadata and each Cargo line at 1 MiB; the complete Cargo stream at
32 MiB. Package limits enumerate at most 1024 members with directory depth 64,
reject symlinks, and preserve the previous double accounting for access JSON
(source plus exported copy), 1 MiB manifest/README allowance, and 16 MiB archive
overhead. They preserve the existing 1024-member and 4 MiB metadata selections.
The software archive owner still independently verifies all images/receipts and
source identities at build, verify, and install. These source changes require
matching native tests and the actual producer/download/package CI consumers;
formatting or wrapper assertions alone do not establish that acceptance.

## Independent KAG provider controls

`tos-kag-provider-controls` owns the complete bounded provider control closure.
Its `template`, `materialize` and `verify` actions accept an explicit
`--template PATH`; the latter two also require `--root PATH`. `verify` reads
the ordered receipt array on stdin; each action returns JSON on stdout and
reports refusal on stderr with a nonzero status. It uses the foundation's
`CorpusSnapshotV1` canonical profile, preserving `corpus_store.canonical` bytes.
The source template and emitted closure are capped at 64 KiB; the nine route
files, duplicate rejection, finite JSON, nonblank cards, new regular output
paths, sizes, hashes and unchanged template remain native owner checks.

The native `tos-kag-release` publisher directly calls this module to materialize
and verify the exact control closure. It invokes the explicitly selected
external aoa-kag producer and its `validate_repo_local_kag_family.py
--probe-source` CLI for the owner's family and provider-home validation.
Export, status and integration verification have no Python dependency. Native
publication tests cover prior-success preservation, exact member verification,
consumer mutation, source-return mismatch and historical four-program releases.
These checks do not accept source meaning, rights, canon or runtime authority.

## Maintained philosophy product commands

The public philosophy atlas, graph-view, graph and post-planting audit builder
and validator scripts forward their ordinary invocation to the installed
`tos-ops-mechanics-plan --philosophy-product` command. The default native role
requires `compiler-backed-validators`; builds with `--no-default-features`
provide the separate CI prerequisite executors. The scripts use the managed
`tos-ops-mechanics-plan` alias on `PATH`, or an explicit absolute
`TOS_OPS_MECHANICS_EXECUTOR`. They fail when that selected executable is absent;
they do not fall back to the retained Python producer bodies. Those bodies
remain explicit reference functions for their existing oracle consumers.

Native product modes are `build`, `check` and `validate`; the corpus product
route derives its existing atlas, view and graph companions on the selected
source view. The original operation clock includes option and worker setup.
The default is 600 seconds with 256 MiB of physical scratch, with the existing
Linux subreaper/pidfd custody and finite cleanup grace. Exit 1 is the bounded
failure contract; a final diagnostic is not promised. SIGINT and SIGTERM exit
130 and 143 after cleanup. Generated products and successful validation do not
admit source meaning, rights, review or canon.

Prepared-dossier readiness and explicit planting use the same native role and
managed command selection; their source, table, disposition and output contract
is documented in [PLANTING_INTERFACE.md](PLANTING_INTERFACE.md). A supported
installation of this role requires its own coherent source/profile/product
proof and managed manifest membership. Installing the access, owner and worker
roles alone does not install this command.

## Accepted-corpus source export through the installed Ops entry

`tos-ops-mechanics-plan --repo-root PATH --kag-source-export-build
--store STORE --revision SHA256 --output NEW_DIRECTORY` selects the existing
`kag_corpus_export::build_export` implementation. Verify that exact directory
with `--kag-source-export-verify --kag-export DIRECTORY`. The maintained
`build_kag_export.py` compatibility entry selects this standard installed Ops
frontdoor; it has no compile-on-call or Python export fallback.

This is the bounded six-source return carrier from an actually admitted corpus
revision. It retains the authoritative export verifier, eight-file inventory,
source/producer/executing-image identity, private staging and atomic publication,
8 MiB export input/output limit and original 600-second operation clock. The
standalone `tos-kag-release` entry retains the same export engine; its broader
KAG integration build/status/verify operations have a separate owner scope and
still retain their existing host adapters. Neither export entry admits source
meaning, rights or canon.

## Maintained currentness and root-entry commands

Root-entry route values have one authored generator input,
`scripts/root_entry_map.source.json`. Native operations read it through held
`RouteSources` and the current root-entry schema; the explicit Python comparison
API reads the same declaration. The compact output ABI and selected export
verification stay unchanged.

Agent surface, documentation family, decision indexes and root-entry map use
`tos-ops-mechanics-plan --repo-root PATH` directly in the validation manifest.
Select `--agent-surface-build [--check]`, `--agent-surface-validate
[--fetch-budget-bases]`, `--documentation-family-build [--check]`,
`--documentation-cross-corpus-validate`, `--decision-index-build [--check]`,
`--decision-records-validate`, `--root-entry-map-build [--check]`, or
`--root-entry-map-validate`. The two root-entry operations accept an explicit
`--kag-export PATH`; selected exports require the genuine corpus admission and
installed Ops `--kag-source-export-build` / `--kag-source-export-verify` route.

Root-entry map has no Python builder, validator or imported comparison engine.
Its route declaration remains `scripts/root_entry_map.source.json`; the native
owner validates its schema and live references before building or checking the
selected companion. The other route families retain their explicit comparison
APIs until their own retirement. Compatible generated format markers may keep
historical names without requiring those launchers.

## Native validation lanes

`tos-validation-lanes --repo-root ABSOLUTE_PATH --sequence route_docs`
shows the authored command sequence; `--run route_docs` executes it under the
same bounded process supervisor. Native sequences need no Python argument.
The manifest's exact `{repo_root}` argument resolves to the selected canonical
root without shell expansion. An explicitly retained Python step still
requires `--python EXACT_INTERPRETER`; no interpreter is discovered for it.

Route currentness, nested cards, task harness, tiny entry and the philosophy
projection lanes call their existing Rust implementations directly.
`tos-ops-mechanics-plan --agents-route-harness-check --repo-root ABSOLUTE_PATH`
and `--tiny-entry-validate` also expose those narrow owner checks individually.
`tos-release-check` follows the same interpreter rule for its selected phase;
its remaining Python test steps still require explicit selection until retired.

The source-home lane runs `--lived-witness-validate` through the native owner.
It checks the schema, exact body/review digests, purpose-specific permissions,
route documents and Git private-path boundaries; authorship, consent, memory
and meaning remain unvalidated. `--intake-pack-validate` checks the maintained
nine-table intake pack, its anchors, promotion residue, gloss coverage and
predicate/class registry counts against explicitly selected repository inputs.

`tos-software-ci verify-reader-install`, `verify-mechanics-install`, and
`verify-web-host` replace the former Python installation/host helpers. Each
requires `--repo-root ABS`. Installation checks use a fresh prefix outside the
checkout, or an explicit existing `--installed-prefix ABS`; reader verification
checks retained old/current exact bytes and platform capabilities. Mechanics
verification runs installed command phases and the native CI selector/gate;
`--command-entries-only` omits Cargo installation of the full mechanics package
and its separate lifecycle suite. Installed symlink entries must resolve inside
the selected prefix. Verification does not modify an existing prefix.

WEB verification generates the pinned wasm-bindgen 0.2.128 binding and consumes
it in Node, with the existing optional float oracle and Worker environment
selectors. `--generated-assets ABS` consumes already prepared bindings through
the same real hosts; that mode verifies behavior and does not claim a new build.
All three operations share native child-process custody, cancellation, a default
900-second child limit, a 3600-second operation limit, and 16 MiB output per
child. Caller-selected resource/storage admission remains external.


The source-home validator and decision builder/validator are native commands:
`--source-home`, `--decision-index-build [--check]` and
`--decision-records-validate`. Their Python implementations and replaced tests
are retired. Source-home schema assertions run in Rust. Decision tests use
fixed reviewed output bytes and explicit refusal cases with an empty PATH;
record validation remains separate from generated-index currentness.

The semantic-registry transition gate is a native command. Its thirteen native
CLI tests preserve immutable baseline selection, independent version advances,
historical schema and reader identity, initial-introduction authority, and
Git replacement, shallow-history and graft refusal. Git is the explicit host
operation; the former Python gate and wrapper are retired. The historical
reader path remains an identity sentinel for earlier commits, never a fallback.

Agon, Experience and Questbook retain their package-local schema and mutation
assertions in the native `--local-contracts HOME` command and
`tests/mechanics_contracts.rs`. Their five replaced Python test files are
retired. Native discovery selects the existing owner homes directly and
refuses new unreviewed Python tests there; it no longer hashes retired source
files to discover a supported command. `experience_contracts` runs that same
native consumer through the validation lane.


Agent-surface currentness and validation use `--agent-surface-build [--check]`
and `--agent-surface-validate`. Their replaced Python builder, validator and
executable test oracles are retired. Rust tests retain immutable profile binding,
activation metadata, package fixity, tracked-file selection, context probes,
public-safety and historical KAG receipt contracts. Historical receipt JSON is
fixture data; its structural validity does not establish current source, shard
or external producer-runtime identity. The default external-integration profile
validates its authored publication routes without selecting an ambient KAG
artifact or executing its producer.
