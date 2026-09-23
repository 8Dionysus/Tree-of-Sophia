# Releasing Tree of Sophia

Software, corpus admission, data snapshots and ecosystem integration have
separate release boundaries under
[TOS-D-0062](decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md).
A software change can land and ship without rebuilding the production corpus,
KAG indexes, stats projections or documentation currentness carriers.

## Software changes and merge

1. Use a clean branch/worktree from the current remote `main`. Preserve other sessions' dirty work and coordinate through the exact changed
owner surfaces.
2. Install Python test dependencies from `requirements-dev.txt`, the MCP extra,
   and locked browser dependencies with `npm ci --prefix access/web`. Review the
   changed behavior and source contracts. Run `python scripts/release_check.py`
   to check contracts, build browser assets and run program fixture tests.
   This command uses program fixtures and repository-owned dependencies.
3. For browser changes, install the locked dependencies with
   `npm ci --prefix access/web`, then run the software check above and
   `python scripts/validation_lanes.py --run software_browser`.
   Browser tests require Playwright and Chromium. They create their own small
   dataset. `access/web/dist` remains an ignored build output.
4. For Worker code, run its locked dependency install, `npm run typecheck
   --prefix access/deploy/cloudflare-worker` and `npm test --prefix
   access/deploy/cloudflare-worker`. These checks run against local fixtures.
5. Build and verify an installable candidate from the reviewed commit:

   ```sh
   python access/packaging/build_software_bundle.py --source-ref HEAD_SHA --output dist/tree-of-sophia-software.zip
   python access/packaging/validate_software_bundle.py --bundle dist/tree-of-sophia-software.zip
   ```

   Replace `HEAD_SHA` with the exact Git commit; build the browser first.
   Dirty development builds require `--allow-dirty` and record that posture.
   The validator checks exact file digests and installs a wheel in an isolated
   environment outside the checkout, without a corpus or AoA installation.
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
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rust/` or `tests/conformance/rust/` | Pinned Rust workspace formatting, native tests, WASM target check and isolated exact reader install |
| Shared contracts/profiles, packaging, dependencies, scripts, workflow, owner cards, source surfaces or any other path | Full software release suite, Worker tests and Rust workspace |

A combined change takes all needed checks. Human Markdown is identified before
its surrounding implementation directory; `ToS/` source Markdown does not use
the documentation-only shortcut. Source assessment/admission still belongs to
its owner, not to a green software gate. This selector does not publish data.
The Rust job uses its own sparse checkout and temporary Cargo cache. The
installed reader checks exact old/current bytes from a tiny synthetic store;
public access and larger runtime profiles remain subject to later gates.

PR and `main` checks use the same rules. `workflow_dispatch` explicitly runs the
full release suite, as does the local `python scripts/release_check.py` command
plus the browser and Worker routes above. A documentation-only green check is
not an installable release artifact: run the full release route when publishing
software, even when the last change was documentation.

When the software job is selected, the workflow uploads
`tree-of-sophia-software.zip` and its digest manifest.
It contains program code, API contracts, static schemas and browser assets;
`data_included` is false. Production activation, tags and public releases require their intended scope
and authorization. Standalone ToS software follows this repository's release
route independently of AbyssOS helpers.

## Registry source-contract changes

An authored semantic-registry change uses the independent
`semantic_registry_transition` source lane. Select the full pre-change commit
and run `python scripts/validate_semantic_registry_transition.py
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
2. Run `scripts/corpus_admit.py` with `--store`, `--batch`, `--input-root`
   and `--grammar-root`. Historical evidence and payload custody use their
   explicit options. A rejected batch leaves the accepted pointer unchanged.
   General record/claim batches currently run a conservative full source audit;
   only the verified retirement transition has a scoped fast path.
3. Run `scripts/corpus_build_worker.py --store STORE --revision REVISION
   --output NEW_SNAPSHOT`. The output directory must be new. Its `manifest.json`
   binds the corpus and completed compiled data; source files cannot select
   executable producers.
4. Select `NEW_SNAPSHOT/data` with `TOS_DATA_ROOT` or `--root`. The reader checks
   artifact integrity and compatibility before serving it. A failed build or
   corrupt new artifact does not replace an existing readable snapshot.

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
remain available through `scripts/validation_lanes.py`. No combined audit of
corpus, KAG, statistics and documentation currentness owns release permission.

Historical v0.5.0 provider release identities remain in `CHANGELOG.md`:
`aoa-stats@v0.2.0` (`88ff38b1b38eef939f2c5b4541cbe8363a05fc8d`) and
`aoa-kag@v0.5.0` (`f46f146cc79a26fa81ad0f400b9c5774df293e57`). The later KAG
source pin `14ee1e33e43749d23c557b3ef526eca7edb36196` is not a retagged release
or a current software CI dependency. AbyssOS consumer admission remains with
its own owner when that integration is explicitly selected.

Public site, Worker and D1 activation remain deferred. Local preparation,
fixture tests and a validated software artifact do not authorize activation.
