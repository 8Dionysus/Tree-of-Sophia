# Releasing Tree of Sophia

Software, corpus admission, data snapshots and ecosystem integration have
separate release boundaries under
[TOS-D-0062](decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md).
A software change can land and ship without rebuilding the production corpus,
KAG indexes, stats projections or documentation currentness carriers.

## Software changes and merge

1. Use a clean branch/worktree from the current remote `main`. Preserve other
   sessions' dirty work; another working session is not a global release lock.
2. Install Python test dependencies from `requirements-dev.txt`, the MCP extra,
   and locked browser dependencies with `npm ci --prefix access/web`. Review the
   changed behavior and source contracts. Run `python scripts/release_check.py`
   to check contracts, build browser assets and run program fixture tests.
   This command does not inspect production data or fetch sibling repositories.
3. For browser changes, install the locked dependencies with
   `npm ci --prefix access/web`, then run the software check above and
   `python scripts/validation_lanes.py --run software_browser`.
   Browser tests require Playwright and Chromium. They create their own small
   dataset. `access/web/dist` is a build output, not a Git companion.
4. For Worker code, run its locked dependency install, `npm run typecheck
   --prefix access/deploy/cloudflare-worker` and `npm test --prefix
   access/deploy/cloudflare-worker`. This does not import D1 or deploy.
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
   session; open a PR. Required **Repo Validation** covers software contracts,
   Python behavior, browser behavior, Worker contracts and the installed
   software artifact. Failed, cancelled or skipped required work cannot pass.
7. Merge only after review and required CI succeed. Verify the resulting remote
   `main` commit and its checks; synchronize clean dependent worktrees without
   resetting another session's dirty checkout. Record the merge commit and
   the software candidate identity separately.

The workflow uploads `tree-of-sophia-software.zip` and its digest manifest.
It contains program code, API contracts, static schemas and browser assets;
`data_included` is false. CI candidate publication is not a production deploy.
Tags and public releases require their intended scope and authorization; an
AbyssOS release helper is not a prerequisite for standalone ToS software.

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
