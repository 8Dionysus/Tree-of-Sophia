# Version and legacy retirement conditions

This inventory records removal gates under [TOS-D-0069](decisions/TOS-D-0069-rust-execution-foundation.md), not approval to remove data. The baseline is `3574eb128b89d9d43d4e67c80fd493de3b0dcea5`. Counts are tracked files or matching lines, as stated; binary contents, generated stores and operator data require separate inventories. A name or a zero text-match count does not establish that a format is unused. Writers and readers below name principal owners, not an exhaustive call graph.

For each package, inventory installed versions, deployments, selected publications, backups and all consumers with the operator. Inspect service arguments and configuration without exposing credentials; inspect store manifests, SQLite metadata, D1 publication metadata and PostgreSQL version/profile and consumer metadata through authorized read-only access. Record counts, retention policy and the exact selected revisions. Exercise old and replacement software/data pairs, restoration, rollback, authorization and rights; retain exact bytes, IDs, references and historical locators. Establish target-capacity and recovery evidence before a large cutover or final legacy retirement. Each gate below adds to these shared conditions. Unknown external counts remain open until this inventory is complete.

## Source store V1

`tos-source-store/src/manifest.rs`, `capture_import.rs` and `CorpusReader` write or read `tos_corpus_snapshot_v1`; `tos-reader` selects V1 by default and admission can migrate it to V2. Two tracked JSON snapshot fixtures carry the V1 marker. Inspect configured store roots, revision manifests and backups for selected and retained snapshots and installed `tos-reader` consumers.

**Removal condition:** Every retained V1 revision required by policy has a verified V2 mapping preserving membership, bytes, identity, rights and history; old and new revisions restore and read, pointers and supported clients have switched, and no retained obligation requires the V1 reader. **Package:** Source-store migration and later V1 reader retirement; retain an old software/data pair when historical verification requires it.

## Source store V2 and physical revisions

`tos-source-store/src/native_v2/` writes and reads V2 roots and commits, including physical and revision variants through V4; `tos-command/src/source_admission_spooled_candidate.rs` builds initial, successor and V1 migration roots, and `tos-reader --format v2` reads them. A search of tracked JSON/JSONL for selected native root/commit identifiers found zero matching lines; this is not a store count. Inspect V2 current pointers, retained roots, commit variants, backups and reader configuration outside Git.

**Removal condition:** For each retired variant, an inventoried, resumable migration preserves exact members, locators and history; restore and read both versions, switch all selected pointers and consumers, and satisfy retention policy. **Package:** Source-store migration tooling and a subsequent per-variant retirement. V2 remains an active target until its own gate is met.

## Legacy knowledge search

`tos-query/src/knowledge_legacy_search.rs` scans selected carriers; `tos-access/src/core_legacy_query_executor.rs`, `knowledge.rs` and the Worker D1 `knowledgeSearchD1` serve callers. The compiler and D1 publication routes produce its input carriers. Tracked legacy-search carrier counts are not established; JSON marker counts alone cannot identify those carriers. Inspect installed CLI, HTTP, MCP and web clients, selected stores and D1 deployments and offsets.

**Removal condition:** Every supported consumer and publication uses a replacement with accepted normalization, ranking, ordering, offsets, rights and error behavior, or an explicitly versioned and accepted change; outstanding continuations expire or migrate and rollback works. **Package:** Search consumer cutover and legacy executor retirement.

## Indexed search V2

`tos-query/src/search_v2.rs` defines typed requests, vocabulary and cursor behavior; `tos-access/src/prepared_local.rs` and `indexed_cursor.rs` serve indexed readers, while the Worker calls `knowledgeSearchD1Indexed`. Prepared/D1 publishers supply indexed data. Three tracked JSON files mention `tos_knowledge_search_indexed_v2`; they are contract descriptions, not an inventory of pages. Inspect deployed capabilities, D1 and local publications, clients and unexpired cursors.

**Removal condition:** A selected API implements accepted indexed semantics and rights, all clients and publications switch, and old cursor lifetimes expire or are migrated with tested continuation and rollback. **Package:** Offline export and consumer cutover; retain reusable V2 semantics in the chosen owner.

## Compressed search V3

`tos-compiler/src/local_prepared_search.rs` writes SQLite search structures; `tos-query/src/compressed_search_state.rs` and `compressed_search.rs`, plus `tos-access/src/prepared_local.rs`, read and serve them. Two tracked JSON contract files mention `tos_knowledge_search_compressed_v3`; external SQLite databases and cursor counts are unknown. Inspect selected SQLite publication metadata, CLI/HTTP/MCP/SDK capability use and cursor expiry.

**Removal condition:** The chosen offline export reproduces accepted query, rights, budget and continuation behavior; all selected publications are rebuilt and verified, consumers switch, and old cursors expire or migrate. **Package:** Offline export followed by compressed-mode cutover and retirement.

## Prepared local search publication

`tos-compiler/src/local_prepared.rs`, `local_prepared_search.rs` and `d1.rs` publish prepared SQLite/D1 data; `tos-access/src/prepared_local.rs` and the Worker consume it. `tos_local_prepared_read_model_v1` occurs in two tracked JSON files, including one frozen fixture; actual publication counts are unknown. Inspect configured local publication roots, SQLite metadata and D1 deployment bindings.

**Removal condition:** Every selected local and D1 publication is rebuilt from the chosen export, verified for source revision, rights, search parity and restore, and every supported reader switches. **Package:** Offline export and prepared-publication retirement. Preserve the separate public D1 and private source-access boundaries.

## Source-text search

`tos-query/src/source_text_search.rs` defines query rules; `tos-command/src/durable_source_text_search.rs` reads the selected private PostgreSQL projection. Native CLI, HTTP and MCP consume it, and `access/deploy/cloudflare-worker/src/source-text-gateway.ts` can forward authenticated requests. Three tracked JSON files mention `tos_source_text_search_v1`; they describe contracts, not stored search data. Inspect configured PostgreSQL consumers and gateway/client configuration without exposing secrets.

**Removal condition:** An accepted successor covers every selected field, rights decision, currentness fence, cursor and recovery behavior, and all consumers and outstanding continuations migrate. **Package:** Central search completion and a separately reviewed API cutover. This path is the current central foundation and is retained until its gate passes.

## PostgreSQL search projection

`tos-command/src/source_search_projection.rs`, `durable_projection.rs` and `durable_schema.sql` maintain derived search state and progress; `durable_source_text_search.rs` reads it. The tracked repository does not contain an inventory of PostgreSQL rows or selected consumers. With authorized read-only SQL, enumerate schema/profile versions, domains, consumer IDs, progress and row counts, plus backups and installed writers; do not extract private text.

**Removal condition:** A successor projection has a complete source-change backfill, transactional progress, equivalent selected-field and rights coverage, verified recovery and accepted capacity; all consumers switch and the old projection can be rebuilt or restored. **Package:** Central search completion and projection migration. PostgreSQL source data and private access remain separately governed.

## Diagnostic query-store search

`tos-query/src/source_diagnostic_legacy.rs` and `source_diagnostic.rs` read authenticated `tos_query_store_v1` carriers; `tos-access/src/core_snapshot.rs` and `core_legacy_query_executor.rs` expose selected query-store operations. Compiler snapshot routes produce carriers. Zero tracked JSON/JSONL files contain the exact marker; binary and external carriers are unknown. Inspect configured query-store files, snapshot metadata, backups and CLI/MCP consumers.

**Removal condition:** Every retained selected carrier and required diagnostic operation has a source-returning, authorized replacement; regenerate or migrate carriers, verify restore and switch consumers. **Package:** Diagnostic carrier migration and reader retirement.

## Files named legacy or historical

A basename search finds 100 tracked files containing `legacy` or `historical`, including data and fixtures; nine are Rust files, eight under `rust/` totaling 12,941 lines and one under `tests/`. Their names are not a deletion list. The Rust files are `tos-access/src/core_legacy_query_executor.rs`, `tos-query/src/{knowledge_legacy_search,source_diagnostic_legacy}.rs`, `tos-compiler/src/legacy.rs`, and `tos-command/src/{source_legacy_claim_store,source_legacy_historical_claim,source_native_legacy_claim_cli,source_native_pg_forms_historical_cli}.rs`; the test is `tests/conformance/rust/command_legacy_claim_cases.rs`. Command modules write or read claims and forms; the compiler and query modules produce or read compatibility carriers. Historical-subject forms and authored Claims remain active responsibilities. Inspect operator claim stores, permissions, selected captures and consumers as well as tracked references.

**Removal condition:** Classify each file and its records by actual behavior; move active historical-subject responsibilities to an accepted owner, satisfy each relevant search or claim migration gate, preserve records and history, and prove no supported caller needs the old implementation. **Package:** Per-owner claim/forms, compiler, query and conformance cleanup; remove only individually qualified files.

## Legacy Python observed JSON and format profiles

`tos-foundation/src/json.rs` defines `JsonMode::LegacyPythonObserved`; `schema_profile.rs` defines `FormatProfile::LegacyPythonObserved20260923`. `tos-validation/src/executor.rs` and command admission/validation routes consume them, including diagnostic input modes. Producers can supply historical Python-shaped JSON and schemas. Zero tracked JSON/JSONL files contain the exact `tos_legacy_python_observed_json_v1` marker; unmarked inputs and operator data are not counted. Inspect retained inputs, schema-worker settings, validation requests and external consumers.

**Removal condition:** Inventory every accepted JSON and format behavior, including duplicate keys, non-finite numbers, Unicode, formats and errors; migrate inputs and consumers to an explicitly specified profile and demonstrate independent conformance and historic verification. **Package:** JSON/format profile migration and later compatibility-mode retirement.

## CPython-compatible Unicode

`tos-foundation/src/{unicode.rs,unicode_generated.rs,unicode_predicates16.rs}` implements pinned Python-compatible Unicode behavior; `access/shared/{native-unicode.ts,native-unicode.generated.ts}` supplies browser/Worker behavior. Rust query and foundation owners and Worker/web readers use these rules; generated tables derive from pinned Unicode data. Tracked data encoded under these rules cannot be counted from a marker. Inspect search indexes, stored normalized keys, client versions and pending cursors outside Git.

**Removal condition:** A versioned successor demonstrates accepted case, strip, classification and ordering behavior across Rust, browser and Worker; rebuild indexes, migrate all consumers and expire or translate cursors. **Package:** Unicode contract migration and generated-table cleanup.

## Python value and compact JSON codecs

`tos-foundation/src/python_value.rs` implements Python-compatible value behavior; `json.rs` owns `emit_python_compact_json`. Command and access code emit packets and stored values, while native/Worker consumers read them through foundation and WASM bindings in `access/shared/native-semantics.ts`. Zero tracked JSON/JSONL files mention the emitter function name; that does not count its emitted bytes. Inspect external receipts, cached packets, content digests, wire consumers and saved inputs.

**Removal condition:** Independent vectors cover numeric, container, string, serialization and error semantics; every retained byte-sensitive artifact remains verifiable and all supported producers and readers adopt an explicit successor. **Package:** Python-value and wire-codec migration, followed by compatibility cleanup.

## Canonical profiles

`tos-foundation/src/json.rs` defines `CorpusSnapshotV1`, `SourceRecordDigestV1` and `SourceCommandInputV1`. Command, source-store and access writers use them for snapshots, record digests and command inputs; readers and verifiers recompute their bytes. Only the snapshot profile adds a final LF. Zero tracked JSON/JSONL files contain each of the three exact profile identifiers; unlabelled preimages and digests remain uncounted. Inspect retained manifests, revision and command receipts, selected publications, backups and external verifiers.

**Removal condition:** A new explicitly named profile has independent vectors and a documented byte/number/Unicode contract; every retained preimage, digest, identity and locator remains verifiable through a tested mapping or retained old reader, and all active writers and consumers switch. **Package:** New canonical profile, source-store re-baseline and later per-profile retirement. A new JCS profile alone does not reinterpret old digests.

## Corpus schema families

The 193 tracked `ToS/contracts/*.schema.json` files include five pairs whose schema literals identify both V1 and V2. Commands and authored source create records; `tos-validation`, `tos-command` and `tos-compiler` validate or consume them. The table counts matching `schema_version` lines and files in tracked `ToS/**/*.json` and `*.jsonl`, excluding `ToS/contracts/`; it includes retained evidence and projections, and does not deduplicate logical records.

| Family | V1 lines / files | V2 lines / files |
| --- | ---: | ---: |
| artifact-source-witness | 23 / 21 | 20 / 19 |
| object-link-claim | 5 / 1 | 12 / 6 |
| provenance-event | 974 / 190 | 147 / 147 |
| source-anchor | 1,355 / 26 | 8 / 8 |
| transfer-route-readiness-projection | 1 / 1 | 0 / 0 |

Inspect external source stores, publication revisions, backups and validators by exact family and version. **Removal condition:** For each family, classify mutable records, immutable receipts and replaceable projections; prove a reviewed field mapping, migrate current records and rebuild projections, preserve exact historical bytes and verify all retained references and consumers. Provenance V2 is additive; preserve V1 evidence without reinterpretation. **Package:** One schema-family migration per owner, followed by deletion of a schema only when historical verification and all supported readers no longer require it.

## Concept request schema V1 and V2

`ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-request.v{1,2}.schema.json` defines two request contracts. The workbench producers and `tos-compiler/src/research_concept_workbench.rs` consume request inputs. One tracked request JSON carries each version. Inspect external requests, retained packets, workbench clients and reproducibility obligations.

**Removal condition:** Accepted V2 or successor mappings preserve request identity and semantics, all active clients switch, and retained V1 packets remain verifiable through a supported reader or preserved software/data pair. **Package:** Concept workbench request migration and schema retirement.

## Exploration and human-form selection schemas

`access/contracts/exploration-{request,result}.v{1,2}.schema.json` defines two request and result versions; `access/contracts/knowledge-graph.v1.schema.json` embeds human-form selection V1 and V2. `tos-access/src/exploration_contracts.rs` and access/web clients exchange exploration packets; `access/web/src/observatory/human-forms.mjs` reads both form selections. No matching data records were found in tracked `ToS` JSON/JSONL or non-schema `access` JSON for the exploration and human-form markers; a contract file lists both form versions. External sessions and packets are unknown. Inspect installed clients, saved exploration sessions, forms and retained responses.

**Removal condition:** All supported clients and producers switch to accepted successor contracts; saved sessions, form selections and responses remain readable or are migrated with verified identity and rights, and outstanding state expires. **Package:** Access contract and client cutover, separately for exploration and human forms.

## Retained builder inputs

`ToS/research-packets/retained-builder-inputs/` holds 46 tracked files: 41 exact Python source blobs, four provenance JSON files and a README. Historical builders produced those recipe bytes; `tos-command/src/source_foundation_reader.rs`, compiler research owners and validation resolve them by original path and digest as nonexecuted evidence. Inspect external receipts, captures and immutable corpus revisions for references to each path/digest.

**Removal condition:** Every referencing receipt and required historical lookup has an operator-approved immutable replacement with exact bytes, digest, historical locator, access rights and a verified restore/read through the actual resolver; update references without changing historical meaning. **Package:** Retained-input custody migration and resolver update. Preserve this archive while those conditions are unmet.

## Frozen Python conformance oracles

`tests/conformance/rust/legacy-python-oracles-v1/` contains 91 tracked files, including 14 expected JSON outputs and 14 corresponding provenance JSON files. Historical Python implementations produced the frozen outputs; `tests/conformance/rust/runner.rs` and command conformance cases read them without regeneration. Inspect external test suites, release evidence and archived fixture copies that rely on these exact hashes.

**Removal condition:** Every protected compatibility behavior and provenance claim has independently sourced, reviewed successor evidence; affected tests and downstream consumers switch, and any required historical oracle bytes remain available and verifiable. **Package:** Conformance evidence replacement and oracle retirement.
