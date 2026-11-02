# Mechanics-local command planning and execution

The `philosophy-library` feature exposes bounded philosophy producers to
native command owners without standalone mechanics families. `tos-command`
selects that subset. Default `compiler-backed-validators` retains both
`philosophy-library` and `mechanics`; the default validator contract and
no-default-feature CI profile remain intact.

The `philosophy-library` feature exposes the bounded philosophy producers to native
command owners. `tos-command` selects that library subset. Standalone OPS binaries keep
the default `compiler-backed-validators` feature, which includes it together with the
dossier, source-registry, artifact and tree-node validators.

`tos-ops-mechanics-plan` discovers native package-local and part-local mechanics
assertions, builders and validators. The default CLI emits one
`tos_mechanics_local_plan_v2` JSON line with commands containing `kind`, `home`
and `argv`. Order is native
assertions by home, builder checks, then validators.

`--repo-root ABSOLUTE_PATH` is required. `--execute` runs the native Growth
pipeline with the other selected native mechanics from that root. Unexpected
Python test/build/validator files fail discovery before any command starts.
ToS operations do not select a neighboring repository or its interpreter.
Consumers own integration, indexing, trust and admission. Maintained validation
and release sequences use native ToS commands.

The first nonzero child stops execution. Child stdout and stderr retain their
streams; diagnostic traceback wording and cross-stream interleaving are not
compatibility promises.

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

The three CI entries (`tos-software-ci`, `tos-validation-lanes` and
`tos-release-check`) build with `--no-default-features`. That selection compiles
the shared command, process-custody and Cargo-product checks without source
storage, schema engines, publication tools or embedded mechanics assertions.
The `mechanics` feature adds those owner routes. Default compiler-backed
validators enable it; the CMD `philosophy-library` subset does not.

Discovery visits only immediate `mechanics/*` and `mechanics/*/parts/*`
homes, rejects encountered symlinks and caps scanned entries. The
native discovery fixtures protect ordering; the real validation-lane CLI fixture protects
stop behavior, output/cancellation deadlines, ordinary/escaped descendants,
successful daemon cleanup, and syscall-unavailable refusal before tool start.
A synthetic PID stream covers the prior count/byte cutoff without spawning
thousands of live processes. No authored ToS meaning is admitted.

`--execute` runs the native Growth pipeline and other selected mechanics.
Agon registry, relation pack and public mirror
routes bind directly to their native owner flags by package/part home. The
route survives removal of Python wrappers; unexpected Python builders or
validators in those homes fail discovery. Artifact validation uses the native
software or selected data export route. Consumers own their admission checks.
Mirror sync and KAG generation remain explicit writes.
Historical Growth assertions are retained evidence and have no executable oracle flag.

`tos-validation-lanes` owns the validation command plane. Its `--check`,
`--sequence ID`, and `--run ID` modes read the existing
`docs/validation/validation_lanes.json` in authored order. Maintained sequences
are native-only; both manifest checking and selection reject Python commands.
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
steps, while retaining support for one legacy final `run tests` step. It accepts
only native commands and preserves quoted `list2cmdline` progress text, first
failure line on stdout, and child status.
The same native executor imposes the finite command, sequence, cleanup, and
output limits above.
Its controlled fixture runs only a temporary four-step release-shaped
sequence. The existing release command entry selects the native consumer. This source
wiring does not claim an actual release or repository CI run.

Install with `cargo install --locked --offline --path
rust/crates/tos-ops-mechanics-plan --root <admitted isolated install root>`
from the integrated workspace. The existing native fixture may target that
installed validation executor via `TOS_VALIDATION_LANES_TEST_EXECUTABLE`;
without the variable it targets Cargo's built validation CLI. This compares the installed candidate using the
same lifecycle/ordering risks, without another test framework.

`tos-software-ci` owns software check selection and result aggregation. `plan --repo-root PATH --base REF`
reads the actual Git no-renames changed paths, validates only new local Markdown
links and merge markers, and emits the same v2 selection and optional
`GITHUB_OUTPUT` fields. `--full` and unknown/shared changes require all checks.
`gate` reads `CI_NEEDS` and rejects missing, failed, cancelled and unexpectedly
skipped jobs. It cannot run checks or accept a release. The `.github` workflow invokes this native command directly.

Git capture uses the existing dedicated Linux pidfd/subreaper boundary, capped
at 30 seconds per command and 120 seconds for the plan. Input is limited to
4 MiB of changed paths and 64 MiB across captured Git output.
`GITHUB_OUTPUT` must be a regular file without a final symlink;
`CI_NEEDS` and the existing append destination are capped at 1 MiB. These are
explicit finite candidate limits. Git error text/exception tracebacks are not
promised byte-identical; selection and successful output
retain the maintained semantics within this profile. No remote fetch occurs.

The validation-lanes, release-check and software-CI commands use the selected installed native tools. `TOS_VALIDATION_LANES_EXECUTOR`, `TOS_RELEASE_CHECK_EXECUTOR` and `TOS_SOFTWARE_CI_EXECUTOR` select their corresponding binaries.

`--semantic-registry-transition` is an explicit native gate for the maintained
semantic registry transition law. `--baseline-commit` supplies the immutable
baseline; otherwise `TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT` applies. Initial
introduction requires `--allow-initial-introduction` or the exact environment
value `TOS_SEMANTIC_REGISTRY_ALLOW_INITIAL_INTRODUCTION=1`; the environment accepts
only `0` or `1`. `--json` emits the maintained result shape. These flags require
this mode. Native `semantic_registry_transition_native` fixtures keep baseline,
replacement, ancestry, profile/version and historical schema assertions, using
actual shallow Git metadata. Their CLI cases invoke the native entry directly.
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
This maintained source-home validation route is native. The former Python
validator remains historical evidence, not an active lane or imported API.

`--philosophy-topology` selects only the native candidate for the maintained
philosophy topology validator. The registered lane retains its command path;
the existing executable now selects this native mode through
`TOS_OPS_MECHANICS_EXECUTOR` or installed `tos-ops-mechanics-plan` on PATH,
with no build on call or Python fallback. An empty explicit selector refuses.
The replaced Python entry and imported APIs are retained only as historical evidence.
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
They retain the existing three-executor manifest and command receipt shape,
while Access build receipts now distinguish explicit dev O0/O1 profiles. The
producer requires the successful same-run Cargo JSON artifact stream, exact
executable package/path, role feature selection, native debug profile, and the
selected Rust version output. It permits O0/O1 only for Access and keeps all
native command roles at O0. It hashes held regular files with a fixed 64 KiB
buffer and verifies file identity before returning. Receipt generation describes
those completed products; it does not establish independent trust in the
producer.
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

## Standalone source and stats exports

`tos-kag-release export-build` and `export-verify` operate on the selected ToS
corpus and exact source-return export. `tos-stats-release validate`, `build`,
`verify` and `status` operate on ToS port bytes and immutable exports. Neither
command selects an external checkout, interpreter, producer or validator.
Consumer configuration, indexing and admission are implemented by consumers.
See `kag/VALIDATION.md` and `stats/VALIDATION.md` for the native procedures.

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
standalone `tos-kag-release` entry retains the same export engine. Consumers own
indexing and integration in their repositories. Neither export entry admits
source meaning, rights or canon.

## Maintained currentness and root-entry commands

Root-entry route values have one authored generator input,
`scripts/root_entry_map.source.json`. Native operations read it through held
`RouteSources` and the current root-entry schema. The compact output ABI and selected export
verification stay unchanged.

Use `--root-entry-map-build [--check]` or `--root-entry-map-validate` with `tos-ops-mechanics-plan --repo-root PATH`. Both accept `--kag-export PATH`; selected exports require the corpus export route.

Root-entry map has no Python builder, validator or imported comparison engine.
Its route declaration remains `scripts/root_entry_map.source.json`; the native
owner validates its schema and live references before building or checking the
selected companion. Replaced Python implementations remain historical evidence.
Compatible generated format markers may keep
historical names without requiring those launchers.

## Native validation lanes

`tos-validation-lanes --repo-root ABSOLUTE_PATH --sequence rust_workspace`
shows the authored command sequence; `--run rust_workspace` executes it under the
same bounded process supervisor. Maintained sequences are native-only; manifest
checking and command selection reject Python commands.
The manifest's exact `{repo_root}` argument resolves to the selected canonical
root without shell expansion.

Tiny entry and philosophy projection lanes call their existing Rust implementations directly.

The source-home lane runs `--lived-witness-validate` through the native owner.
It checks the schema, exact body/review digests, purpose-specific permissions,
route documents and Git private-path boundaries; authorship, consent, memory
and meaning remain unvalidated. `--intake-pack-validate` checks the maintained
nine-table intake pack, its anchors, promotion residue, gloss coverage and
predicate/class registry counts against explicitly selected repository inputs.

`tos-software-ci verify-reader-install` and `verify-mechanics-install`
replace the former Python installation helpers. Each
requires `--repo-root ABS`. Installation checks use a fresh prefix outside the
checkout, or an explicit existing `--installed-prefix ABS`; reader verification
checks retained old/current exact bytes and platform capabilities. Mechanics
verification runs installed command phases and the native CI selector/gate;
`--command-entries-only` omits Cargo installation of the full mechanics package
and its separate lifecycle suite. Installed symlink entries must resolve inside
the selected prefix. Verification does not modify an existing prefix.

Both operations share native child-process custody, cancellation, a default
900-second child limit, a 3600-second operation limit, and 16 MiB output per
child. Caller-selected resource/storage admission remains external.


The source-home validator is a native command: `--source-home`. Source-home schema assertions run in Rust.

The semantic-registry transition gate is a native command. Its thirteen native
CLI tests preserve immutable baseline selection, independent version advances,
historical schema and reader identity, initial-introduction authority, and
Git replacement, shallow-history and graft refusal. Git is the explicit host
operation; the former Python gate and wrapper are retired. The historical
reader path remains an identity sentinel for earlier commits, never a fallback.

Agon and Experience retain their package-local schema and mutation
assertions in the native `--local-contracts HOME` command and
`tests/mechanics_contracts.rs`. Their five replaced Python test files are
retired. Native discovery selects the existing owner homes directly and
refuses new unreviewed Python tests there; it no longer hashes retired source
files to discover a supported command. `experience_contracts` runs that same
native consumer through the validation lane.
