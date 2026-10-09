# Releasing Tree of Sophia

Software, corpus admission, data snapshots and ecosystem integration have
separate release boundaries under
[TOS-D-0062](decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md).
A software change can land and ship without rebuilding the production corpus,
KAG indexes, stats projections or documentation currentness carriers.

## Software changes and merge

1. Use a clean branch/worktree from the current remote `main`. Preserve other sessions' dirty work and coordinate through the exact changed
owner surfaces.
2. Install locked browser dependencies with `npm ci --prefix access/web` and
   review the changed behavior and source contracts. Build the native release
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
   Workflow topology assertions remain separate checks.
   Run `tos-release-check --repo-root "$PWD"`
   to check contracts, build browser assets and run program fixture tests.
   This command uses program fixtures and repository-owned dependencies.
   Native immutable-file integration tests require fs-verity on the selected
   temporary filesystem. CI supplies a job-owned ext4 image formatted with
   `-O verity`, points `TMPDIR` at it and unmounts it after the tests; local
   execution must likewise select a filesystem with fs-verity enabled.
   The wrapper forwards explicit `--command-timeout-ms`, `--lane-timeout-ms`,
   `--cleanup-grace-ms` and `--max-output-bytes` to the native executor.
   Omitted limits retain its defaults. Select a phase with `--phase checks`
   or `--phase tests` when the preceding phase already succeeded on the same
   candidate. The tests phase executes the final authored `run tests` suffix;
   that suffix may contain several named test-group commands, all of which run
   in order and retain the same bounded command and lane deadlines. Set a
   longer command deadline only from the selected workload
   cost; a timeout is incomplete validation, not a passing test result. These
   flags do not change host resource admission or skip checks in that phase.
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
5. Build and verify the native installable candidate from a clean reviewed
   commit. Prepare the locked `tos-access` binary with the pinned toolchain and
   its exact nine-field `tos_native_access_build_v1` build receipt. Prepare the
   genuine frontend through locked Vite and the matching verified
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
   lock, target, toolchain, profile and actual image size/hash; dirty source is
   refused. Archive validation and fresh-prefix install check the exact native,
   JSON and web closure without a Python runtime or corpus. Preserve the
   previous prefix and its matching verifier for rollback. This new verifier
   intentionally refuses older mixed Python/native archives.
   CI uses this native route with the eighteen native command roles for its software
   candidate. It builds receipts for the exact feature sets in the shared native command
   descriptor, and derives package byte limits from the actual nineteen
   executables, web assets and bounded software inputs; it does not reuse an
   access-only byte cap. The job builds the same native archive and sidecar
   twice and compares their bytes before verify/install, retaining the legacy
   builder's determinism assertion on the maintained artifact. There is no
   Python wheel reference package in the release candidate path. Historical
   Python oracle tools remain optional comparison tools outside the native prefix.
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

### Checks selected for a change

The workflow always checks changed Markdown for conflict markers and newly
introduced relative file links (not remote URLs or fragment anchors). It then
selects the union of the following software checks. Deletes and both sides of
renames participate. Empty changes, unknown paths and selection-policy changes
use the full suite; a missing or failed selector fails the required gate.

| Changed paths | Selected checks |
| --- | --- |
| Root, `docs/` or `access/` human Markdown, excluding `AGENTS.md` | Documentation checks only; no package rebuild or browser/Worker install |
| `access/web/` or `access/e2e/` code/configuration | Software contracts, browser build/unit/types/behavior, isolated software package install |
| `access/src/` or `access/tests/` | The browser/package checks, reader/API fixture tests, and Worker cross-adapter tests |
| `access/deploy/cloudflare-worker/` code/configuration | Worker type and behavior tests, including cross-adapter fixtures |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rust/` or `tests/conformance/rust/` | Software contracts, browser assets and native package install, plus pinned Rust workspace formatting, native tests, FND/VAL WASM target checks, isolated exact reader install, and generated WEB.1 codec checks in Node WebAssembly |
| Shared contracts/profiles, packaging, dependencies, scripts, workflow, owner cards, source surfaces or any other path | Full software release suite, Worker tests and Rust workspace |

Every run first prepares the three native CI executors with the pinned Rust
compiler. The plan job records their source commit/tree, lock digest, toolchain
and binary sizes/digests; software, Rust and the required gate reuse that same
run's artifact and verify its identity before placing the native executors on PATH.
Missing products or a failed plan fail closed. Documentation-only changes skip
the software package and browser/Worker jobs, but still need this native selector
prerequisite. Cargo cache reuse is an optimization, not evidence of identity.

A combined change takes all needed checks. Human Markdown is identified before
its surrounding implementation directory; `ToS/` source Markdown does not use
the documentation-only shortcut. Source assessment/admission still belongs to
its owner, not to a green software gate. This selector does not publish data.
The Rust job uses its own sparse checkout and temporary Cargo cache. The
installed reader checks exact old/current bytes from a tiny synthetic store;
the generated WEB.1 binding runs independent codec vectors in Node. The
matching `wasm-bindgen` CLI is verified against its pinned release digest.
Browser/Worker bundle installation, public access and larger runtime profiles
remain subject to later gates.

PR and `main` checks use the same rules. `workflow_dispatch` explicitly runs the
full release suite, as does the local `tos-release-check --repo-root "$PWD"` command
plus the browser and Worker routes above. A documentation-only green check is
not an installable release artifact: run the full release route when publishing
software, even when the last change was documentation.

When the software job is selected, the workflow uploads
`tree-of-sophia-software.zip` and its digest manifest.
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

A KAG or stats artifact names its exact ToS source/data revision and external
owner revision, validates its own integrity and compatibility, and blocks its
own publication on failure. It does not block unrelated source/software merge.
Consumers must expose revision and staleness rather than claim that an older
integration follows the latest source. See `kag/VALIDATION.md` for KAG checks.

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
