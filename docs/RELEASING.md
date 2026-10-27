# Releasing Tree of Sophia

Software, corpus admission, data snapshots and ecosystem integration have
separate release boundaries under
[TOS-D-0062](decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md).
A software change can land and ship without rebuilding the production corpus,
KAG indexes, stats projections or documentation currentness carriers.

## Software changes and merge

1. Use a clean branch/worktree from the current remote `main`. Preserve other sessions' dirty work and coordinate through the exact changed
owner surfaces.
2. Review the changed source contracts. Build the native CI selector and lane
   executors from this exact checkout and bind absolute paths:

   ```sh
   cargo +1.98.1 build --locked --no-default-features -p tos-ops-mechanics-plan \
     --bin tos-release-check --bin tos-validation-lanes --bin tos-software-ci
   executor_dir="$(pwd)/target/debug"
   export TOS_RELEASE_CHECK_EXECUTOR="$executor_dir/tos-release-check"
   export TOS_VALIDATION_LANES_EXECUTOR="$executor_dir/tos-validation-lanes"
   export TOS_SOFTWARE_CI_EXECUTOR="$executor_dir/tos-software-ci"
   ```

   If `CARGO_TARGET_DIR` is set, use its absolute `debug` directory instead.
   Run `tos-software-ci plan --repo-root /absolute/checkout --base BASE_REF`
   for source selection and changed-document checks, or `tos-software-ci gate`
   with the exact `CI_NEEDS` job results. CI authenticates and invokes the
   same-run native executor directly. Native source-history tests cover
   selection, Unicode links, failure propagation and required-job outcomes.
   Workflow topology assertions separately cover the source-bound software
   producer, its fixture and package consumers, and final candidate admission.
   Documentation-only work needs the selected documentation checks. For the
   software suite, install locked browser dependencies with
   `npm ci --prefix access/web` and run `tos-release-check --repo-root "$PWD"`.
   The suite checks contracts, builds browser assets and runs program fixtures.
   Its exact commands and order live in
   [validation_lanes.json](validation/validation_lanes.json); the
   [Rust workspace guide](../rust/README.md) describes native/WASM prerequisites.

   Native immutable-file integration tests require an fs-verity-enabled
   temporary filesystem. CI provides a job-owned ext4 image with `-O verity`;
   local execution must select an equivalent filesystem. Retained-history
   fixtures also need the commit and selected blobs named in
   [retained-opening-source.txt](../tests/conformance/rust/retained-opening-source.txt).
   The native reader disables lazy fetching, so prepare those inputs first.

   The executors accept explicit command/lane timeouts, cleanup grace and
   output bounds. Command and aggregate lane deadlines are separate; choose
   changes from the workload and retain the manifest's per-step limits.
   A timeout is incomplete validation. `--phase checks` or `--phase tests` can
   select a phase when the preceding phase already passed on the same candidate.
   The tests phase runs the complete ordered test suffix.
3. For browser changes, install the locked dependencies with
   `npm ci --prefix access/web`, then run the software check above and
   `tos-validation-lanes --repo-root "$PWD" --run software_browser`.
   Browser tests require Playwright and Chromium. They create their own small
   dataset. `access/web/dist` remains an ignored build output.
4. For Worker code, first prepare the matching rules from this exact checkout:

   ```sh
   rustup toolchain install 1.98.1 --profile minimal --target wasm32-unknown-unknown
   cargo +1.98.1 build --locked --release -p tos-web-rules --features wasm --target wasm32-unknown-unknown
   rules_target_dir="${CARGO_TARGET_DIR:-target}"
   bindgen_dir="$(mktemp -d)"
   curl --fail --location --silent --show-error \
     'https://github.com/wasm-bindgen/wasm-bindgen/releases/download/0.2.128/wasm-bindgen-0.2.128-x86_64-unknown-linux-musl.tar.gz' \
     --output "$bindgen_dir/wasm-bindgen.tar.gz"
   printf '%s  %s\n' 'b51f0208fdff83515a787bd8ab9ac5865ed84dabb66d0c709957bb59793c645f' "$bindgen_dir/wasm-bindgen.tar.gz" | sha256sum --check --status
   tar -xzf "$bindgen_dir/wasm-bindgen.tar.gz" -C "$bindgen_dir"
   "$bindgen_dir/wasm-bindgen-0.2.128-x86_64-unknown-linux-musl/wasm-bindgen" \
     --target web --out-name tos_web_rules --out-dir access/deploy/cloudflare-worker/generated \
     "$rules_target_dir/wasm32-unknown-unknown/release/tos_web_rules.wasm"
   rm -rf "$bindgen_dir"
   ```

   This verified Linux x86_64 recipe produces the matching JavaScript, WASM and
   two declaration files required by the Worker bootstrap and Node preloader.
   Then run `npm ci --prefix access/deploy/cloudflare-worker`, `npm run typecheck
   --prefix access/deploy/cloudflare-worker` and `npm test --prefix
   access/deploy/cloudflare-worker`. These checks run against local fixtures;
   the Worker CI job prepares its own rules independently of the software job.
5. For software publication, build and verify the native installable candidate
   from a clean reviewed commit. Prepare the locked `tos-access` binary with the pinned toolchain and
   its Cargo-derived `tos_native_access_build_v2` receipt from the same JSON
   artifact stream: the ten-field receipt carries the executable's observed
   dev `opt_level` (`0` or `1`) and exact SHA/size. Legacy nine-field v1
   receipts remain readable. Native command receipts continue to require O0.
   Prepare the genuine frontend through locked Vite and the matching verified
   `wasm-bindgen`/`tos-web-rules` product, then use that explicit dist handoff:

   ```sh
   /path/to/tos-access software build --root /absolute/clean-source --source-ref HEAD_SHA \
     --web-dist /absolute/current-web-dist --output /absolute/tos-software.zip \
     --native-access-binary /path/to/tos-access --native-access-receipt /absolute/native-build.json \
     --max-total-bytes EXPANDED_CAP --max-archive-bytes ZIP_CAP --max-members MEMBER_CAP --max-metadata-bytes METADATA_CAP
   /path/to/tos-access software verify --archive /absolute/tos-software.zip \
     --max-total-bytes EXPANDED_CAP --max-archive-bytes ZIP_CAP --max-members MEMBER_CAP --max-metadata-bytes METADATA_CAP
   /path/to/tos-access software install --archive /absolute/tos-software.zip --prefix /absolute/fresh-prefix \
     --max-total-bytes EXPANDED_CAP --max-archive-bytes ZIP_CAP --max-members MEMBER_CAP --max-metadata-bytes METADATA_CAP
   /absolute/fresh-prefix/bin/tos --version
   ```

   Replace `HEAD_SHA` with the exact clean commit and caps with explicitly
   admitted finite package budgets. The native receipt binds source commit/tree,
   lock, target, toolchain, profile/Access opt level and actual image
   size/hash; dirty source is refused. Archive validation and fresh-prefix install check the exact native,
   JSON and web closure without a Python runtime or corpus. Preserve the
   previous prefix and its matching verifier for rollback. This new verifier
   intentionally refuses older mixed Python/native archives.
   CI uses this native route with the eighteen native command roles for its
   software candidate. The `software_prepare` job produces exact-source native products,
   their receipts, the E2E fixture executable and browser assets once. The
   `software` fixture job and `software_package` job consume those staged inputs
   from the same source checkout after verifying and binding the stage manifest.
   The fixture job runs only the canonical native Cargo tests; browser npm and
   Playwright installation belongs to the package/browser job.
   `software_prepare` builds receipts for the exact feature sets in the shared
   native command descriptor. The package job derives byte limits from the
   actual nineteen executables, web assets and bounded software inputs; it does not reuse an
   access-only byte cap. The job builds the same native archive and sidecar
   twice and compares their bytes before verify/install, retaining the legacy
   builder's determinism assertion on the maintained artifact. `software_package`
   uploads the archive and sidecar as a provisional candidate with their digests.
   The required gate waits for `software_prepare`, `software` and
   `software_package` to succeed, verifies both candidate digests against the
   downloaded artifact, then uploads the qualified software artifact under the
   source SHA. A provisional artifact alone does not qualify a release. There is
   no Python wheel reference package in the release candidate path. Historical
   Python oracle tools remain optional comparison tools outside the native prefix.
   Public reading pages also have an independent static-only Cloudflare route.
   From `access/deploy/cloudflare-worker`, build them with
   `npm run build:public-research` and inspect the upload with
   `npm run deploy:public-research:plan`. After the normal review, required CI
   and merge checks, deploy only
   `wrangler deploy --config wrangler.public-research.jsonc`. It serves
   `/zarathustra*`, `/antiquity*` and `/__research/*` on apex and `www`, without
   D1, R2 or secret bindings. This deployment leaves the existing access
   Worker and D1 data unchanged. Record its version ID; rollback with
   `wrangler rollback VERSION_ID --name tree-of-sophia-public-research` restores
   that Worker's version while preserving the separate path routes.
   Native prepare fixture calls in CI use a 45-second allowance and a 55-second
   child wait, matching the measured joined maintenance fixture; the native
   product default remains 20 seconds. This allowance is not a production SLO.
6. Complete the ordinary checkpoint review for the exact repo, commit and
   session; open a PR. Required **Repo Validation** selects checks from the
   exact changed paths using the table below. Failed, cancelled or unexpectedly
   skipped selected work cannot pass; an unselected job must be skipped.
7. Merge only after review and required CI succeed. Verify the resulting remote
   `main` commit and its checks; synchronize clean dependent worktrees without
   resetting another session's dirty checkout. Record the merge commit and
   the software candidate identity separately.

### PR, main and release qualification

PRs run their normal selected checks. A main push may reuse source CI only
after verifying the merged PR, one unambiguous successful workflow/required gate
at its latest attempt,
the gate's actual pinned checkout commit and its PR parents, and equality of
that tested tree with the landed tree. The native selector checks the tree
again. Changed-document and selected documentation-owner checks still run.
Missing, ambiguous or inaccessible evidence uses normal main selection.
Read-only API failure cannot accept an unverified prior result.

This source result does not qualify a package for the landed SHA. Run the
complete `Repo Validation` workflow with `workflow_dispatch` on the exact main
SHA before selecting its software candidate for release. Explicit full runs
ignore reuse evidence and retain all build, receipt, determinism, install,
browser, Worker, Rust and PostgreSQL checks. Publication and deployment are
separate actions. [D0073](decisions/TOS-D-0073-verified-pr-source-ci-and-exact-sha-releases.md)
records the tested-tree and package-provenance boundary.

### Checks selected for a change

The workflow always checks changed Markdown for conflict markers and newly
introduced relative file links (not remote URLs or fragment anchors). It then
selects the union of the following software checks. Deletes and both sides of
renames participate. Empty changes, unknown paths and selection-policy changes
use the full suite; a missing or failed selector fails the required gate.

| Changed paths | Selected checks |
| --- | --- |
| Human Markdown at root or a known guide home in the [selector](../rust/crates/tos-ops-mechanics-plan/src/software_ci.rs); Rust guides outside `src/` and `tests/`; test README/validation guides | Changed-document checks; fixture directories retain their consumer route |
| `AGENTS.md` in those homes | Source card shape, inventory, inheritance, script references and task-route harness; no unrelated package/browser/Worker build |
| `ToS/` Markdown, `QUESTBOOK.md`, `quests/` Markdown or Questbook's `QUESTBOOK_TOS_INTEGRATION.md` | Complete Rust consumer lane and installs; ToS cards additionally select the source route checks |
| `.agents/agents-route.current.json`, `.agents/agent-surface.current.json`, `docs/validation/documentation-family.current.json` | Check the changed carrier through its existing owner builder/validator; route-carrier changes also check cards and task routes |
| `access/web/` or `access/e2e/` code/configuration | Source-bound software preparation, fixture tests, browser build/unit/types/behavior, and isolated package install |
| `access/src/` or `access/tests/` | Source-bound software preparation, package/browser checks, reader/API fixture tests, and Worker cross-adapter tests |
| `access/deploy/cloudflare-worker/` code/configuration | Worker type and behavior tests, including cross-adapter fixtures |
| Rust source under `rust/crates/tos-reader/src/` | Rust workspace and isolated exact reader install; the independent browser/package job is omitted |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rust/` or `tests/conformance/rust/` | Software contracts, browser assets and native package install, plus pinned Rust workspace formatting, native tests, FND/VAL WASM target checks, isolated exact reader install |
| Shared contracts/profiles, packaging, dependencies, executable scripts, workflow, structured source surfaces or any other path | Full software release suite, Worker tests and Rust workspace |

The software selection runs one exact-source producer followed by two required
consumers. `software_prepare` produces the native products and receipts, E2E
fixture and browser assets once. `software` runs the selected native fixture
cohort; `software_package` builds and installs the candidate and runs browser
behavior against that installed package. The required gate requires all three
results when software is selected and requires them to be skipped otherwise.
The package consumer reports the provisional archive and sidecar digests; only
the required gate publishes the qualified artifact after it checks those
digests and every selected job result.

The Rust selection runs three named matrix checks under the required `rust`
aggregate: `conformance`, `workspace` and `cold-install`. Each runner performs
the common pinned-toolchain, retained-history, wasm-bindgen and fs-verity
preparation, then the native lane executor selects the common preparation
prefix and only that group's ordered suffix from the single `rust_workspace`
route. The group filter preserves each selected step's command, order and
budget. Matrix fail-fast is disabled so all three checks report their outcome.
The independent PostgreSQL durable-command laboratory remains a separate
required job; its pinned PostgreSQL 16 service, fs-verity volume, exact schema
worker and two fresh restore databases are unchanged. The required gate still
consumes one `rust` aggregate result, which succeeds only when all matrix
children succeed, as well as the PostgreSQL result.

Every run first prepares the three native CI executors with the pinned Rust
compiler and `--no-default-features`. This selection retains command custody,
source identity and required-job checks without compiling mechanics, storage
or schema-validation families. The plan job records their source commit/tree, lock digest, toolchain
and binary sizes/digests; software preparation and both software consumers,
Rust, and the required gate reuse that same run's artifact and verify its
identity before placing the native executors on PATH.
The package builder reuses these three products as its corresponding command
roles. Producer receipt generation rechecks their source-bound manifest and copied bytes;
the other native roles still require their completed Cargo artifact streams.
Missing products or a failed plan fail closed. Documentation-only changes skip
the software package and browser/Worker jobs, but still need this native selector
prerequisite. Cargo cache reuse is an optimization, not evidence of identity.
Optional plan and Worker caches hold public Cargo registry data, compiled
libraries, compiler fingerprints and build-script outputs. Finished command/test
images and incremental compiler state are excluded. Keys separate the observed OS
image/architecture, pinned compiler, lock and Cargo/toolchain manifests, feature
cohorts, reduced debug profile and source tree. Identical trees share a key across
commit metadata changes; compatible earlier trees can restore through the same
compiler/dependency/cohort prefix. Cache retention never replaces source-bound
product generation or candidate preservation.
Every Cargo command, product receipt, artifact hash and selected check still
runs after a cache hit or miss. Software tests use a separate target directory
from production images so test features cannot replace packaged executables.
Cache availability does not select checks or qualify a software candidate.

A combined change takes all needed checks. Human Markdown is identified before
its surrounding implementation directory. Other Markdown under a `fixture`, `fixtures`
or `testdata` component keeps its enclosing consumer checks. Unknown homes keep
the full selection. `ToS/` source Markdown retains the complete Rust consumer
lane because owner cards and research packets are exact source-test inputs;
the independent browser and Worker implementations do not consume those bytes.
`QUESTBOOK.md` and its integration guide also retain Rust because they are
parsed or required inputs to the Questbook owner validator.
Source assessment/admission still belongs to
its owner, not to a green software gate. This selector does not publish data.

Source route checks use `tos-route-cards validate-source` and the existing task
harness. This runs the same authored card/inventory/inheritance/reference laws
as `validate`, without requiring an untouched generated route carrier to be
rebuilt. `validate` keeps its existing parity check. When a generated carrier
itself changes, CI selects that owner's parity check explicitly. Ordinary
software and documentation changes do not acquire a repository-wide currentness
dependency. All selected documentation checks run inside the required plan job;
failure, missing selection outputs or cancellation still reject the gate.
Each Rust matrix runner uses the existing sparse checkout and its own temporary
Cargo target. The software producer and its fixture/package consumers share
their source checkout scope and use the pinned Ubuntu 26.04 image that matched
the measured software baseline. Rust host-program fixtures use the qualified
Ubuntu 24.04 runner explicitly; changing its distribution is a separate
validation step. The installed reader checks exact old/current bytes from a tiny
synthetic store.
The Worker job checks the shipped `tos-web-rules` WASM product with the matching
`wasm-bindgen` CLI verified against its pinned release digest.
Browser/Worker bundle installation, public access and larger runtime profiles
remain subject to later gates.

Software producer/consumer jobs and Rust runners share one resource-profile
script and diagnostics route. Software profiles identify the job; Rust profiles
identify the matrix group. The profile records `os.availableParallelism()`, the
process CPU affinity, the cgroup CPU quota, host available memory and cgroup
memory limit/headroom. Cargo uses four build jobs only when the runner confirms
at least four CPUs and 12 GiB of available memory across the visible cgroup v2 hierarchy; otherwise
it uses at most two, lowering to one when fewer than two effective CPUs or
less than 6 GiB of measured memory are available. An absent quota at the
verified hierarchy root is recorded as unbounded. If the mount hides ancestors
or the cgroup capacity is otherwise unknown, four jobs are not enabled. Resource
JSON and Cargo timing reports are uploaded under unique run and attempt names,
using the software job or Rust matrix group for separation, including when a
software check or Rust lane fails. The cgroup `cpu.stat` snapshots bracket the
measured validation work but can include work charged to the runner cgroup
outside it. `memory.peak` is recorded before and after
without resetting the job cgroup, so its high-watermark may include setup that
ran earlier in that job and is not a lane-only maximum. These artifacts help
explain cost and limits; they do not prove validation, product identity or
release authority.

PR and `main` checks use the same rules. `workflow_dispatch` explicitly runs the
full release suite, as does the local `tos-release-check --repo-root "$PWD"` command
plus the browser and Worker routes above. A documentation-only green check is
not an installable release artifact: run the full release route when publishing
software, even when the last change was documentation.

When software is selected, `software_package` uploads a provisional archive and
sidecar. After the required gate accepts all selected work and verifies both
reported digests, it uploads `tree-of-sophia-software.zip` and its digest
manifest as the qualified artifact.
It contains program code, API contracts, static schemas and browser assets;
`data_included` is false. Production activation, tags and public releases require their intended scope
and authorization. Standalone ToS software follows this repository's release
route independently of AbyssOS helpers.

## Local native owner-command delivery

Source mutations retain an owner-scoped invocation route. The existing software
archive can deliver their executables together with access, the schema worker and
the three compiler-free ops entries using the optional fixed command cohort
documented in access/README.md. Access-only archives remain supported. Archive
installation grants no write authority and installs no invocation/config files.
For separately prepared local products, the existing Cargo installation route
also remains available from an exact reviewed checkout:

```sh
owner_prefix=/absolute/fresh-owner-prefix
cargo +1.98.1 install --debug --locked --offline --path rust/crates/tos-command \
  --bin tos-native-owner-command --root "$owner_prefix"
cargo +1.98.1 install --debug --locked --offline --path rust/crates/tos-validation \
  --bin tos-schema-worker --root "$owner_prefix"
sha256sum "$owner_prefix/bin/tos-native-owner-command" "$owner_prefix/bin/tos-schema-worker"
```

Preparation needs its own admitted storage/process budget. Keep the exact source,
lock, toolchain and both resulting product identities with the owner receipt;
an executable found on PATH is not an authorization or a matching worker proof.
The complete historical `foundation` audit is available from the installed
`tos-research foundation` entry. The full software cohort builds that executable
separately with `--no-default-features --features research-tools`, preserving the ordinary reader's
feature set. A standalone owner build can instead select `--features research`.
Ordinary selected-source validation
does not compile the laboratory and gold-set implementations; requesting a full
audit from that build returns an explicit refusal before source capture.
The owner supplies a protected, user-owned `0600` invocation file and protected
owner config, with absolute `native_executable` and `schema_worker.absolute_path`
from this prefix and their exact SHA digests. The existing invocation also binds
owned corpus store/source revision, software capture/restored root and selection,
software components and finite budgets. Claim/Item profiles retain explicit
`original_source_revision`; Alignment retains `owner_context`. These existing
profiles remain supported. The prepared common dispatcher profile
`tos_local_native_source_invocation_v1` binds both `original_source_revision` and
`owner_context` (null only for an unused field), plus `assessment_schema_worker`
(null unless the selected signing operation requires that worker). Required
worker bindings retain exact executable custody; request content cannot select
a worker, owner config or invocation path. Family-specific profiles and
authorized operations belong to the command owner.

The portable installed Rust client consumes the selected owner request on
stdin without requiring this checkout or a Python runtime:

```sh
/absolute/prefix/bin/tos-native-owner-command source-commands \
  --invocation /absolute/protected-native-invocation.json < /absolute/request.json
```

Discover the packaged implementation grammar without source or owner access:

```sh
/absolute/prefix/bin/tos-native-owner-command source-commands --discover
/absolute/prefix/bin/tos-native-owner-command source-commands --discover --handler native-work-expression
```

CLI discovery and HTTP `GET /commands/catalog` use the same packaged descriptor.
Discovery grants no authority; the HTTP route retains its existing authentication.

The optional Owner software role delivers this entry and the packaged
`access/contracts/source-commands.v1.json` input/issuer contract. The local
account that owns the selected source supplies its protected policy files;
request transport does not manufacture their principal, authority, allowed
operations or expiry. Existing native family `describe`, `prepare` and mutation
requests use their unchanged typed grammar and independent current-use checks.
A Host Rust private-stage issuer is a separate explicit resource boundary when
needed by the selected receiver, not a source-field authority.

The installed executable also accepts `--invocation ABSOLUTE_FILE` directly.
Both routes must retain the same selected owner/source/rights/recovery and worker
custody checks. Installing products does not construct an invocation, authorize a
mutation, select a production corpus or switch a running cohort. The normal
mutation dispatcher uses the protected native invocation and fails closed when
it is missing. Source preparation is distinct from composition, installed
default availability and the final cohort switch.

## Registry source-contract changes

Semantic definition edits start in `ToS/doctrine/semantic-interchange/definitions/`.
Assemble and replace the complete flat compatibility registry pair, then run
`tos-definition-tree check-compatibility ToS/doctrine/semantic-interchange/definitions ToS/doctrine/semantic-interchange`.
The software workflow enforces this deterministic generated-pair check; it does
not accept new semantic meaning. Historical raw bytes remain in the pinned
migration baseline; parsed equality does not imply historical byte equality.

An authored semantic-registry change uses the independent
`semantic_registry_transition` source lane. Select the full pre-change commit
and run `tos-ops-mechanics-plan --repo-root ABS --semantic-registry-transition
--baseline-commit FULL_COMMIT_OID --json`, or supply
`TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT` to the named validation lane. Retain
that baseline and the comparison result with the source review. The owning
[registry contract](../ToS/doctrine/semantic-interchange/README.md) specifies
version advances, retained schema routes and semantic review.

A first introduction additionally requires explicit
`--allow-initial-introduction` and complete baseline ancestry establishing
the absence of earlier registries, contracts and the declared-profile reader.
The validator verifies those conditions. Software CI and this source-contract
check retain separate results and scopes.

## Data and corpus operations

Select data explicitly through `TOS_DATA_ROOT` or `--root`; the reader does not
search the current working directory or sibling repositories. Existing
`TOS_DATA_ROOT` selects a dataset; `TOS_RELEASE_ROOT` selects a managed release
pair. The former `TOS_ROOT` and `AOA_TOS_ROOT` aliases have been removed.
Code-owned API schemas and browser assets always come from the software.
A UI change does not require rebuilding a compatible selected query store.

Corpus identity, provenance, fixity, rights, review and admission remain with
`ToS/` owners. Changing a source record requires its affected owner validation;
passing software CI does not admit that record or publish it. Full dataset
coverage tests are marked `data_release` and require a separately selected
`TOS_DATA_ROOT`. They do not run as software tests.

The source-to-reader operation is explicit:

1. Prepare a `tos_corpus_batch_v1` manifest naming the exact base revision,
   validator identity, source bytes/modes and any explicit retirements.
2. Run `tos-native-owner-command corpus-admit` with explicit `--store`,
   `--batch`, `--input-root`, `--grammar-root` and protected `--invocation`
   paths. Historical capture and payload custody use their explicit options
   when selected. A rejected batch leaves the accepted pointer unchanged.
   General record/claim batches currently run a conservative full source audit;
   only the verified retirement transition has a scoped fast path.
3. Produce a managed native Original candidate with
   `tos-native-owner-command corpus-build < REQUEST_JSON` and the
   `tos_native_corpus_build_request_v1` schema. The request names the exact
   immutable corpus store and revision, a separately selected, digest-bound
   software capture and restored root, its Git commit/tree and component
   selection, and the exact schema-worker path and digest. `mode: build`
   composes the six fixed source runtime products and invokes the native data
   writer; it writes fresh `data_directory` and private release-candidate
   directories under the selected persistent store and does not move a release
   pointer.
4. For parity, send the same source, software and worker selection in `mode:
   check` with an absolute `comparison_root`. This recomposes and compares the
   exact six products without a persistent write. Submit a successful build's
   private pair to the ManagedRelease owner for full pair compatibility and
   selection. A producer candidate is not an installed or published release.

Bulk source revisions and their historical evidence use permanent local
storage and permitted private Cloudflare R2 backups with verified restore.
Curated authored sources remain Git-backed; generated catalogs and projections
are not Git companions. Preserve exact old commits/locators before retiring
tracking. Payload custody remains permanent local plus its permitted private
R2 copy; local-only records gain no upload or public rights. No history rewrite
or deletion of unique corpus evidence is part of release.

## Integration and historical audits

ToS builds and verifies source-return and stats exports from its own selected
source/data revision, without consumer source code, Python environments or
services. See `kag/VALIDATION.md` and `stats/VALIDATION.md`. Consumers import those
artifacts and own indexing, composition, trust and admission on their side.
Consumer release identities and currentness do not gate a ToS software release.
TOS-D-0072 supersedes the former provider-pin/action-fetch requirements, including
TOS-D-0066; historical decision commands are not current release prerequisites.

The former full repository aggregate is retired. Select the affected source,
data or external integration operation directly; individual owner procedures
remain available through `tos-validation-lanes`. No combined audit of
corpus, KAG, statistics and documentation currentness owns release permission.

Historical v0.5.0 provider release identities remain in `CHANGELOG.md`:
`aoa-stats@v0.2.0` (`88ff38b1b38eef939f2c5b4541cbe8363a05fc8d`) and
`aoa-kag@v0.5.0` (`f46f146cc79a26fa81ad0f400b9c5774df293e57`). The later KAG
source pin `14ee1e33e43749d23c557b3ef526eca7edb36196` is not a retagged release
or a current software CI dependency. AbyssOS consumer admission remains with
its own owner when that integration is explicitly selected.

Public site, Worker and D1 activation remain deferred. Local preparation,
fixture tests and a validated software artifact do not authorize activation.
