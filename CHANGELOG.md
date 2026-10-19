# Changelog

All notable changes to `Tree-of-Sophia` will be documented in this file.

The format is intentionally simple and human-first.
Tracking starts with the community-docs baseline for this repository.

## [Unreleased]

## [0.6.0] - 2026-10-19

This release covers changes since v0.5.0 through the accepted integration
base `ac184b9335396bec4d370ed8176c7919529cc2df`.

Tree-of-Sophia now has a native Rust foundation for source growth, exact history,
research and delivery. PostgreSQL transactions keep source changes and their
durable work queue; independently maintained projections serve navigation and
search. Portable models, research datasets and software remain separately
selected products. This software release does not activate the public site,
Worker, D1 or production data, accept all research inputs into canon, or qualify
the later large-capacity and 250-Luna exercises.

### Added

#### Native foundation and WebAssembly

- Added the Rust workspace and native products for exact source reads, source
  commands, schema execution, compilation, query, mechanics, research, packaging
  and installation. The installed `tos` entry and native MCP run without a Python
  host. Optional imported Python SDK adapters frame calls to selected native
  products; they do not supply another maintained query engine or a runtime
  fallback.
- Added shared native/WASM rules for codecs, knowledge requests, temporal
  comparison, inspection, lens execution, exploration and browser state. Browser
  DOM, Fetch, platform storage, presentation and asynchronous I/O retain their
  host roles. Generated binding checks cover actual Node/workerd/browser
  consumers rather than treating WASM compilation as product acceptance.
- Defined exact versioned JSON and identity profiles: strict published JSON
  rejects duplicate decoded keys; request compatibility retains last-value
  semantics where declared. Integer lexemes, finite float spelling, member order,
  historical whole-form serialization and trailing-newline rules remain distinct
  contracts. Unicode 16 lowercase, strip, full casefold and simple scalar folding
  have explicit profiles. Byte, Unicode code-point and browser UTF-16 offsets
  keep their own units.

#### Source custody and corpus admission

- Added descriptor-anchored exact corpus readers and a separate immutable
  segment store with content-bound frames, retained pin/attempt journals, exact
  locators and cold recovery. Path traversal rejects symlinks and substitution;
  the raw Linux reader stages verified bytes before stdout. Custody establishes
  exact bytes and retained identity; access and admission are supplied by their
  owning adapters.
- Added explicit native AdmissionStore V2 publication, authenticated trees,
  retained typed revisions, packed objects and bounded disk-backed closure
  checks. V1 remains an explicit compatible reader. V2 exact source-ID and
  member-path reads, warm successors, committed-revision verification and fresh
  target restoration preserve original source/history identities. A failed
  verification cannot publish a new revision or select an incomplete copy.
- Added native Git capture, bounded import/restore and chunk transport. A fresh
  source capture can compose disjoint explicitly retained subtrees into one
  content-addressed pool while preserving each original snapshot and receipt.
  Overlapping namespaces, changed membership and incompatible source modes are
  refused. Imported custody does not select a current corpus or grant semantic
  admission.
- Added declared `full-audit`, `selected-source-closure` and
  `selected-record-closure` validation profiles. The record profile binds exact
  logical roots and physical ledger slots, including row/offset/delimiter and
  raw/canonical digests. Required dependencies remain mandatory; unrelated rows
  in the same physical file receive no acceptance from the selected result.
  Profile and selection bytes participate in validator identity. Labs and
  Goldsets outside a selected scope are reported absent rather than passed.
- Added offline schema execution with pinned disposable native workers, exact
  schema/resource closure, finite request/response/state limits, deadlines and
  cancellation. Shared source rule kernels cover records, text, layers, forms,
  provenance, relations, registries, rights and reference closure. Reuse applies
  only to identical documents and schema selectors inside the held operation;
  source, authorization and publication checks still run.

#### Source records and human forms

- Expanded source-owned creation, preparation, correction, inspection and replay
  across Agent, Work, Expression, Edition, Item, File, Collection, Artifact, Link,
  descriptive metadata and qualified Claim families. Compound operations retain
  exact responsibility, membership, parent, source-home and provenance bindings.
  Collection order is bound to particular retained membership versions. Shared
  File identity preserves each Item's separate representation and rights context.
- Added versioned semantic entity/relation registries with labels, definitions,
  ownership, lifecycle, hierarchy, source crosswalks and relation domain/range,
  direction, cardinality and evidence rules. Stable semantic IDs accompany source
  kinds and predicates. Original public records remain lossless source returns;
  unknown mappings stay explicit. Graph joins preserve reified Claims, evidence,
  maker, review, version and supersession instead of inferring identity from names.
- Expanded source-declared profiles for people, collectives and institutions;
  authorship and other participant roles; events, processes, biographical phases,
  places and historical environment; intellectual objects, documents, letters,
  fragments and material carriers. Problems, positions, distinctions, arguments,
  methods, thought moves, imagery and values retain their own scoped descriptions.
  Production, reception and conceptual transformations are evidence-bearing
  relations rather than facts inferred from a label or a directory. Type, role,
  property, classification, identity and similarity remain distinguishable.
- Added source-described Conception and CrosscuttingConcept profiles, scoped
  structures, research corpora, Occurrences and qualified motif proposals. A
  Conception has a contestable scope and continuity criterion. A multi-member
  motif remains one Claim-scoped hypothesis with every selected participant in
  its mandatory reading closure; it does not issue a Sign identity. Sign
  promotion requires its separate assessment and source-owner delegation.
- Added exact semantic-identity and continuity proposal routes for explicitly
  supported subjects, including a separately granted semantic-subject profile.
  A proposal can report a merge, split or continuity question without executing
  that identity change. Unknown, absent and false values remain distinct; a
  versioned description correction does not silently change the referent.
- Added exact current and historical HumanForms, source-copy materialization,
  freeform forms and source-owned compact Claim display fields. Name, caption,
  hover, statement, grounds and history retain their exact subject, language,
  source/version/digest, assessment and required context. Source descriptions
  can be corrected through authorized versioned operations while preserving
  predecessors and rebinding forms.
- Added lossless shared-context HumanForm delivery v2 alongside inline v1.
  Common packet values and repeated limits are encoded once and reconstructed
  under separate expansion bounds. Missing, malformed or over-budget context
  withholds the affected wording with its exact inspection reference; it is
  never shortened by dropping qualifications. Consumers and restored saved
  selections must understand the declared envelope version.
- Added source-owned Claim navigation templates and explicit typed time readers.
  Historical time, source wording and a catalogue's attributed document date,
  origin or destination remain distinct. Comparisons retain precision, calendar,
  uncertainty, alternatives and exact operand versions; unsupported roles or
  unknown coordinates return an explicit undetermined result. Claim-scoped
  reference values declare their actual member slot instead of interpreting
  arbitrary JSON as references.
- Kept world/event time, witness time, acquisition and review time distinct in
  the source and history model. Exact retained-version reads and typed temporal
  comparison do not promise an arbitrary global as-of reconstruction.
- Added attributed Letter 705 carrier, catalogue date/place/direction and
  source-copy/display-form records, plus source-backed corrections and retained
  Claim histories. Catalogue attribution remains visible; these records do not
  turn a reported catalogue field into an unqualified historical fact.

#### Text, OCR and alignment

- Added native TextLayer and TextUnit creation, bounded exact public/private
  unit return, derived-layer capture and edit replay, public project Text and
  Occurrence construction, and physical-part composition. Exact source
  coordinates, layer identity, derivation, fixity, rights and source-visible
  assessment remain available through the resulting bindings.
- Added immutable native translation alignment, retained mapping inspection and
  bilingual capture/replay. Alignment records preserve the exact source and
  target units, selectors, mapping, lineage and grant; their existence alone
  does not establish equivalence or translation quality.
- Added retained-page OCR recording and a separate source-image comparison
  route. Signed OCR import verifies the independently pinned P-256/SHA-256
  signer, receipt, source/language binding and exact output bytes through native
  cryptography. Producer source/digests are provenance; no producer checkout,
  Python adapter, Cosign execution or network trust discovery is required.
  Signature verification and image/text comparison remain separate from
  fidelity, reuse rights, interpretation and canon judgments.

#### Assessment and access conditions

- Added source-visible assessment by competent authorized humans or agents and
  separately derived current scoped admission. Assessment binds the assertion,
  evidence/counterevidence, policy, reviewer, competence and execution profile.
  Reusable grants can cover a declared language/layer/purpose scope without
  fabricating a human review for each record. Disagreement, deferral,
  withdrawal, reviewer independence and dependency invalidation remain visible.
- Added purpose-specific text quality and dependent-use checks for citation,
  linguistic analysis, semantics and search. A changed layer, comparison or
  required source dependency can invalidate use even when the Claim wording
  stays unchanged. Rights, consent, publication and canon keep their actual
  owner decisions.
- Added protected confidential source contexts and native local Text, Profile,
  Claim, assessment and alignment profiles. Explicit private roots, owner-only
  files/directories, exact logical bindings, expiry and operation grants govern
  access. Historical private reads recheck current rights. Ordinary HTTP and
  source dispatch do not expose these private write profiles.
- Added native CMD-local PostgreSQL rights revocation with stable command retry
  and expected-rights-version checks. Revocation blocks pending writes, replay
  and current/historical delivery within that domain; it does not erase the
  immutable evidence or decide broader legal rights. Committed creation evidence
  remains inspectable after an issuing grant expires, without renewing access.

#### Search, lenses and exact reading

- Added the standalone `access/` product with native CLI, loopback HTTP, native
  stdio/Streamable HTTP MCP and read-only WebMCP contracts. Catalog, node/relation
  inspection, philosophy/corpus views, source descent, dossier, exact source,
  bounded CSV-row and native TextUnit reads preserve selected source versions
  and disclosure conditions. Capability discovery advertises available owners
  and supported contracts; it does not issue grants.
- Added a backend-defined knowledge catalog and LensSpec grammar shared by human
  and agent consumers: node/relation filters, exact focus, path witnesses,
  endpoint closure, grouping, sorting and stored views. Catalogs expose observed
  vocabulary, semantic mappings, facets, limits and unavailable entity routes.
  Lossless source packets stay separate from query attributes and clearly
  marked metadata-derived display descriptions.
- Added resumable bounded graph exploration with exact snapshot/configuration
  binding, deterministic replay, expiry and cancellation. Prepared local readers
  can explicitly select private SQLite checkpoints that survive restart; Worker
  continuations use separate TTL-bound D1 state and concurrent-winner admission.
  Process-local, prepared, managed-release and Worker checkpoint guarantees and
  cursors remain profile-specific.
- Added versioned knowledge search engines: maintained legacy substring/count
  behavior, indexed-v2 candidate verification/continuation, and explicitly
  prepared compressed-v3 storage. Unicode grams, rare-gram intersection,
  bounded verification prefixes and compressed posting blocks reduce unnecessary
  work without treating candidates as complete matches. Engine discovery drives
  the UI; offsets, native cursors and Worker tokens are not silently converted.
- Added durable PostgreSQL source-text search over declared titles, labels,
  quotations and HumanForms, with exact-ID lookup, weighted ranking and source
  revision returns. Its v2 profile adds current Agent source-copy forms through
  exact record/form dependencies. Technical keys, paths and arbitrary nested
  JSON are not indexed text. This first source-text profile returns bounded hits
  without promising pagination or a total count.
- Added independently authorized exact source and private text reading, including
  guarded history and current-policy rechecks through final native transport
  flush. MCP budgets account for both text and structured copies, escaping,
  request ID and framing; a fitting query packet is not automatically a fitting
  MCP frame. Native compact serialization and typed error/exit behavior are
  explicit compatibility contracts.

#### Research interface and constructor

- Added Evidence Lens, contextual WebMCP actions, visible connection/tool state,
  native-MCP fallback, bilingual examples and source-gap research routes. Research
  cards expose source facts, provenance, Claim qualifications, related material
  and exact exports with readable localized labels. Same-named materials and
  different predicates remain distinguishable; grouped links retain each relation.
- Added a persistent local research workspace with collections/shelves, saved
  passages, notes, drafts, reading positions, scene state and import/export.
  Pending notes, exact source paths and exits survive UI transitions. Missing
  text discovery does not hide an available exact source record. Catalog reuse,
  stale replay invalidation and atomic detach/update rules preserve selection.
- Added the constructor and original-language reader, source-bound material and
  fragment libraries, bounded live exploration, source commands and exact-reading
  handoffs. Rust/WASM owns authoring transitions, hierarchy, draft conditions,
  saved poses, atomic edits and bounded undo/redo; the browser owns interaction
  and storage. Live startup and demo initialization are separate entries.
  Native desktop delivery provides its local launcher and HTTP boundary.
- Replaced per-boolean browser/WASM observation sessions with whole held-value
  validation where applicable. Original JS object identity, UTF-16 keys,
  sparse arrays, getter order, callbacks and thrown errors retain their host
  behavior. This reduces bridge/state duplication without claiming an extra
  browser heap limit.

#### Research inputs and philosophy dossiers

- Completed the indexed prepared-dossier sets at 48, 58 and 84 for Tables I–III,
  with master-row/identity checks, multilingual extraction, source-anchor
  backlogs, language packets, candidate nodes/relations and post-planting audit.
  Readiness and planting use owner-declared package/route maps rather than one
  hard-coded table. Weak confidence and review dispositions remain explicit.
- Added non-era frontier treatment for cross-geography/cross-chronology
  khipu/rongorongo and a distinct Andean information-system route. Table III
  keeps a non-universalizing navigation window. The undeciphered-script frontier
  remains undeciphered; the prose-only dossier produces zero structured semantic
  candidates and needs a source-completed successor before semantic promotion.
- Added source-registry normalization, reconciliation, immutable snapshots,
  history and inspection. Corpus, document, logical record and versioned
  occurrence identities are separate. Raw DOCX/OOXML fields retain their exact
  original-file digests and locators; unsupported mappings and anonymous edited
  row identities require explicit treatment rather than title-based merging.
- Added the generic XML/UXLC A/B/C laboratory and native consumer. Its reviewed
  result prefers an exact-file generic element topology, retains opaque capture
  and a separate UXLC-specific derived projection, and rejects the specialized
  projection as a generic owner. Private source text stays withheld. This
  laboratory result does not itself promote a public inventory contract or Item.

#### Source-bound research tools

- Added source-bound Zarathustra word/concept search and a multilingual candidate
  workbench returning exact German occurrences, historical Antonovsky comparison,
  morphology, speaker/context candidates and English tasks on demand. Low-frequency
  forms and verse remain included; semantic neighbors and negative controls stay
  separate. Request/version identity, optional technical successor plans and
  explicit first-time identity issuance prevent unrelated requests sharing objects.
- Added the whole-work Reading workbench with exact code-point spans,
  sentence/clause anchors, quotation scopes, voice candidates, occurrence
  crosswalks, sparse repeated-formula families and explicit coverage/gap ledgers.
  It uses separately selected private source/analysis roots. Heuristic morphology,
  coreference, literary attribution, semantic similarity and translations retain
  candidate or deferred status; private datasets are not installed by software CI.
- Ported maintained lexical, structural, paragraph-alignment, source/target text,
  German triangulation, collation, recurrence, usage/morphology, transfer,
  source-visible review-receipt and visual-result recipes to bounded native
  `tos-research` producers/validators. DTA lexical build/validate has its own
  exact source/payload/schema and private SQLite contract. Frozen historical
  recipes, receipts and control outputs retain their original identities;
  equality claims are scoped to the checked logical outputs, not arbitrary
  physical SQLite bytes or every downstream consumer.
- Preserved the bibliography and acquisition distinctions needed for artifact
  images and multi-volume scholarly composites: physical witness, editorial
  composite, exact File and reusable text are separate. Prepared research,
  structural green checks and a generated graph do not accept their historical
  assertions or admit a complete philosophical corpus.

#### Discovery and acquired material

- Added the reviewed open-work candidate loop with immutable terminal receipts,
  frozen targets, exact queue snapshots, source-backed readiness and per-channel
  timing evidence. Chronological and readiness selection remain explicit modes.
  Zero-result or metadata-only outcomes can advance without claiming acquired
  bytes; a held-witness outcome requires a resolved actual witness/acquisition.
- Added bounded acquisition handoff verify/adapt and custody checks for one
  explicitly selected frozen handoff, including Item manifests, File membership,
  rights, provenance, payload digests, ownership and read-only file state.
  Credential-bearing provider URLs, duplicate evidence keys, FIFOs, hardlink or
  path races and incomplete identity closure are refused. Local-only Items no
  longer require a server plan; server transfer requires its separate scope.

### Changed

#### Transactional source updates

- Moved the central update direction to durable PostgreSQL source transactions
  with exact STO custody, atomic current/history/receipt/log/outbox changes,
  current rights/rule/schema/job fences, cancellation and retained replay.
  Explicit Agent creation and selected-record revision read addressed inputs and
  affected predicates rather than rebuilding the complete Site. Independent
  unchanged source updates can coexist under their readsets. Other command
  families retain their declared full-cut preparation costs.
- Added explicit protected PG source bootstrap, initial Agent/public Text and
  selected revision routes, including separate schema provisioning. Ordinary
  mutations do not run implicit DDL. Mixed-source commitments and complete
  audit coverage bind maintained record/Claim/Text/File families; older
  Agent-only commitments need a fresh compatible bootstrap.
- Separated initial work from routine updates. Streamed bootstrap retains
  bounded ORIGINAL batches and can resume assessment after their complete EOF,
  or finish addressed packing after a committed assessment. Partial original
  writes or changed inputs/authority refuse resume. Source-text initial build
  commits each selected source and its progress atomically; search remains
  pending until complete EOF publishes the captured head.
- Projection consumers apply exact committed source revisions and commit output
  with their durable progress position. Interrupted events remain pending and
  reopen resumes completed work. Queue telemetry distinguishes sequence lag,
  server-clock age and unknown historical enqueue time. Source commit,
  projection publication and current reader selection are different outcomes.

#### Projection publication and continuation

- Added guarded prepared-model bootstrap, addressed source/catalog/semantic/search
  deltas, exact dependency maintenance, coherent source/prepared pairing and
  explicit implementation/profile transitions. Changed rows and deletion keys
  are staged against an exact predecessor and published atomically. Replay,
  interruption, stale binding and A→B→A replacement checks preserve the last
  coherent result. Full bootstrap/schema replacement remains a separately
  selected maintenance operation.
- Added opt-in content-bound processing reuse and dependency lineage. Reuse keys
  include actual inputs, processor/schema/registry identities and relevant absent
  dependencies; global identity and closure checks remain. Completed reused
  work is distinguished from newly executed work. No cache result grants source
  acceptance or current disclosure.
- Made bounded event-range publication the maintained navigation writer.
  Complete selected Claim streams bind their own histories, causal dependencies
  and full predecessor inventory, including unchanged companion Claims. New
  Records require creation evidence; admitted original Records retain their
  bootstrap provenance. Exact empty files remain distinct from absence.
- Added authenticated navigation cohort maps, coverage/adjacency roots,
  verified-prefix checkpoints and resumable initial Original capture. Ordinary
  PG navigation reads authenticate bounded admitted metadata and current policy
  without reopening the SQLite anchor or scanning all historical events.
  Unchanged scopes can remain readable while unrelated source work lags;
  changed dependencies return `publication_pending`.
- Added receipt-sequence lower bounds for source descent across CLI, HTTP,
  MCP and WebMCP. Canonical decimal strings preserve 64-bit sequences in browser
  transports. A lagging publication refuses immediately without forcing
  processing. D1 has no equivalent CMD sequence and refuses that parameter.

#### Portable models and resource accounting

- Deduplicated graph view membership and partitioned growing projections into
  bounded content-addressed collections. Offline compilers produce explicit
  disposable query artifacts; ordinary reads consume completed snapshots and
  do not trigger a hidden rebuild.
- Added native carrier-once model formats through V4: exact original packets
  stored once, authenticated compression dictionaries, packed rank fields and
  compact posting runs. Decoded bytes and ordering preserve the selected
  semantics. Older inline and CarrierOnce ABIs retain their versioned readers;
  similar bytes never silently upgrade the selected format.
- Separated reader libraries from capture, source tools, full model producers,
  mechanics, research and native-model dependencies through explicit Cargo
  features. Lightweight reader and CI-executor builds omit those operations;
  installed roles select their required features and assets. Features select
  code availability, while runtime ownership and rights remain explicit.
- Reduced repeated source scans, tree/value copies, validation and retained
  buffers; added indexed branch-parent lookup, bounded keyset seeks, streamed
  metadata/cold audits and per-owner dependency forecasts. Parser/decoded state,
  logical reads, SQLite VM work, file limits, temporary-space quotas, process
  memory and device I/O remain separately accounted dimensions.
- Added prospective assessment-history capacity checks before prepare/publish,
  sharing the authenticated reader's byte/count predicate. Local raw history and
  canonical bytes are checked independently; PG packages also charge the new
  64-byte head while retaining old raw heads. Overflow and over-limit successors
  refuse before publication. Limits remain 1 MiB per batch, 32 MiB history and
  1,024 batches/events; source qualification, replay and current-head fences remain.
- Avoided redundant decoding when constructing already checked ASCII JSON
  strings without callbacks. UTF-8 preflight, checked/fallback parsing, work,
  cancellation and storage guards remain; quotas and worker CPU ceilings were
  not raised.

#### Cloudflare delivery

- Added repository-owned Worker/Static Assets and generated D1 delivery for the
  site, API, Evidence Lens, graph/lens operations and scale exports, with Tunnel
  retained as a recovery deployment profile. Exact source/schema revision
  comparison avoids D1 row writes for code-only rebuilds; addressed transitions
  include guarded catch-up, forward/rollback artifacts and integrity checks.
- Moved maintained Worker query/temporal/inspect/lens/exploration semantics to
  shared Rust/WASM while retaining D1 I/O and publication checks in the host.
  Snapshot/epoch checks guard final whole-packet body handoff; Worker Response
  construction alone does not establish delivery. Software contracts, web assets
  and schemas come from software, never a selected data root.
- Added native offline prepared-pair/navigation capture and bounded SQL framing
  and local import products. Capture writes unapplied local SQL/manifests; remote
  import and consumer switching require their separate operation.
- Added an optional authenticated source-text gateway to a separately selected
  native PostgreSQL consumer. Fixed origin/path, distinct inbound Access JWT
  verification and outbound service credentials, bounded JSON bodies and one
  deadline protect the transport. D1 does not become a source-text store, and
  configured capability reports are not runtime probes.
- Restored the production analytics beacon/referral collection path, prevented
  duplicate injection and retained geography/security/cache behavior. Local
  previews omit the production beacon. Previously uncollected referral history
  is not reconstructed, and this release performs no hosting activation.
- Added Social Connector service pages and retained service records as optional
  UI inputs. Research operation remains usable when those records are absent.

#### Independent execution and exports

- Separated software builds/tests/packages from corpus admission/data snapshots
  and independent KAG/stats/integration releases. Required software CI uses
  bounded fixtures and a software-only install. Each data or consumer release
  retains its own exact input, compatibility, currentness and acceptance evidence.
- Removed sibling checkout, ecosystem Python environment, host-service and
  admission-broker prerequisites from ToS execution. The native ToS launcher
  owns per-operation Linux containment, private storage, kernel checks, deadlines,
  cancellation and cleanup using explicitly selected paths. Useful libraries,
  PostgreSQL and optional deployment services retain their normal roles.
- Made KAG/source-return and stats exports ToS-owned native products.
  `tos-kag-release export-build/export-verify` and
  `tos-stats-release validate/build/verify/status` operate on selected ToS bytes.
  Consumers configure their own providers, indexes, additional grammar, trust,
  registries and admission. The earlier temporary KAG freeze and provider-pin
  campaigns are history, not current ToS release prerequisites.

#### Mechanics and contributor routes

- Reorganized human and agent entry routes around source owners, short inherited
  route cards and on-demand validation. Native builders/checkers maintain route,
  agent-surface, documentation-family, root-entry and decision companions.
  The validation manifest owns ordered commands; its shared command references
  avoid duplicate procedure bodies.
- Added native mechanics discovery/execution, Agon thresholds, Questbook,
  relation-pack, mirror, Experience contract and Growth assertion routes.
  Active parts/topology and contributor fixtures follow current native APIs;
  source-owner operation guidance names explicit read/prepare/write routes.
  Mirror sync, planting, generation and publication remain explicit writes.
- Added Linux pidfd/subreaper process custody, per-command/whole-lane deadlines,
  output caps, cancellation, first-failure propagation and bounded descendant
  cleanup for native command lanes. Unavailable facilities or incomplete cleanup
  return failure with residual/unknown custody evidence. External supervision
  remains necessary if the supervisor itself is killed or crashes.
- Preserved token-first active-naming checks, nearest authored branch-parent
  selection, optional external feedback-cache behavior and native topology and
  dossier checks. Cache failure falls back to live validation; CI/release do not
  select the feedback cache. Historical Python performance steps are consolidated
  into the current native behavior rather than retained as a pytest runtime gate.

#### Packaging, backup and recovery

- Added native deterministic software archive build/verify and fresh-prefix
  installation, binding commit/tree, lockfile, toolchain, Cargo features, native
  build profiles, exact executables, contracts and web assets. Optional roles are
  explicit. The current software package includes no working production corpus;
  installation neither chooses data nor starts services.
- Added bounded native PostgreSQL/STO backup and fresh-target restore with exact
  pinned PostgreSQL 16 tools and an independently retained receipt digest.
  Backup v11 covers navigation maps and resumable initial capture. Readers for
  v1–v10 preserve their original receipt/census rules. Restore verifies source,
  SQL ABI, dump/store bytes and complete cold closure before later selection.
- Added explicit installed Agent correction/publication inspection and retained
  recovery routes. After interrupted projection publication, inspect the saved
  outcome before retrying; recreating the source is not publication recovery.
  A verified restored store still needs current authorization and explicit
  reader activation.

### Removed

- Retired the maintained Python CLI, wheel/runtime fallback, source handlers,
  producers, validation/release/CI entrypoints and replaced executable tests.
  Native owners and independent frozen vectors retain the documented contracts.
  Exact historical builder/schema/metadata bytes remain provenance inputs under
  their original digests; they are not executed as a fallback.
- Removed the synthetic CMD1 PostgreSQL coordinator, its independent SQL schema
  and protocol-only target. Durable CMD/STO now owns the maintained commit,
  conflict, replay, cancellation and cold-history obligations. Former synthetic
  prefix/reverse/interval APIs did not establish a supported source reader or
  completeness law and receive no implied replacement guarantee.
- Removed the first-generation navigation-only SQLite pipeline, unused
  single-event publishers/backlog driver, unconsumed compiler producer generations,
  full-scan/materializing readers, closed admission/reader helpers and private
  D1-pair prototype. Current event-range, prepared and portable readers keep their
  explicit formats; no blanket equivalence is claimed for every removed API.
- Removed WEB.1, the source-codec feasibility product and its verification route.
  It was a migration probe, not the ToS website. Maintained Foundation and
  domain/WASM products have their own contracts.
- Removed the Antifragility, Checkpoint, Recurrence, Method Growth and RPG routing
  packages. Audit, Growth and Questbook retain their applicable responsibilities
  and active routes. Spark/legacy scaffolding and the copied foreign skill
  crosswalk leave active topology; shared skill discovery belongs to its owner
  user profile.
- Removed duplicate `.record-revisions` and `.metadata-transactions` trees from
  the current checkout in favor of retained-corpus bindings and a bounded shared
  native fixture capture. Historical locators and source bytes remain exact.
  Full historical corpus use requires a separately retained corpus or restoration
  of its pinned Git state; a software-only checkout is insufficient.
- Removed tracked reproducible browser bundles, bulk generated catalogs,
  projections, KAG family/receipt churn and superseded glue from ordinary source
  delivery. Those derived products are selected data/export artifacts. Authored
  sources, unique provenance and original records retain their owner routes.
- Removed obsolete `abyssos` / `abyss-sdk-host` diagnostic profiles, ToS-owned
  provider-control orchestration, ecosystem host discovery and producer-checkout
  OCR execution. Standalone diagnostics and ToS exports are the maintained routes.

### Compatibility and upgrade

- Install into a fresh prefix and retain the previous prefix plus its matching
  verifier. The current verifier deliberately rejects the previous mixed
  Python/native archive layout. Software/data rollback selects a compatible
  verified pair and cannot reverse a rights revocation. Other internal Cargo
  components may retain independent `0.0.0` / `0.1.0` versions. The `tos-access`
  CLI for this release is version `0.6.0`; the GitHub tag and source/build
  manifests identify the complete release.
- Select data and operations explicitly. An explicit local root takes precedence
  over an inherited release selection in its supported route; an explicitly
  selected managed release retains its own admission checks. Missing selected
  data never authorizes checkout discovery or substitution. Data cannot override
  packaged schemas or web assets. Source-only/headless reader roles and web roles
  have different asset requirements.
- Rebuild/rebootstrap only through the declared source-backed compatibility
  route when moving from older Agent-only inventory commitments, unsupported
  SQL/audit profiles, archive-file-owned bootstrap nodes or earlier navigation
  admission/checkpoint profiles. Reviewed cold transitions preserve the original
  activation/journal history and verify the complete source before a new cut.
  Unknown schema/DDL profiles remain incompatible; arbitrary automatic migration
  is not provided.
- Upgrade range producers and readers together for the new 14-field manifests
  with complete stream predecessors. The exact older 13-field shape remains
  readable by its historical contract; an older reader rejects the new shape.
  Reified navigation producer/profile changes require explicit derived rebuilds,
  preserving source Claim/Agent IDs and command history.
- Discover search and continuation capabilities on the selected consumer.
  PostgreSQL navigation anchors intentionally omit portable offline search
  indexes; those indexes still belong to full portable models. Source-text v2
  requires a new consumer/profile build, not relabeling v1. Prepared restart
  checkpoints do not establish managed-release restart survival or cursor
  portability between services.
- Update automation to native executables and the separate `tos-research`
  command. Replace private-stage environment names with `TOS_STAGE_TICKET_FD`
  and `TOS_STAGE_ROOT`. Select `verify --profile standalone`; removed integration
  profile names are no longer accepted. Consumers import ToS exports from their
  own side instead of invoking retired provider orchestration.
- Preserve exact OCR receipt/signature/output and independently selected signer
  key. Historical `owner_source_root` is inert provenance. A valid signature
  neither renews an expired grant nor accepts the text for a new use.
- The descriptor-relative raw reader requires Linux 5.6+ `openat2` and has no
  weaker path fallback. Managed fs-verity/containment profiles require their
  actual supported kernel, filesystem, private mounts, finite process/cgroup
  limits and selected custody. A finite file cap is not a total temporary-space
  quota, and a successful build is not measured whole-corpus fit.
- Current backup transport is PostgreSQL 16 over numeric loopback NoTls with
  exact tools, a truly quiescent source and a fresh independent private target.
  Historical receipt wire support is not arbitrary old SQL-ABI or remote-TLS
  restore support. SQL restoration and copied filesystem state are separate
  effects; preserve a failed target rather than treating it as an atomic rollback
  or retrying into it.
- Native CLI compact JSON, framing, errors, profile limits and frozen cursor
  formats are explicit contracts; Python pretty output/FastMCP serialization and
  unsupported external APIs are not compatibility promises. New finite bounds
  can refuse inputs that older unbounded helpers attempted to read.

### Validation and software evidence

- Accepted `main` passed all five selected post-merge Repo Validation jobs in
  run `39175345318`: plan/document checks, software/browser/installed package,
  Rust/reader/WASM, Worker and the aggregate gate. The saved software evidence
  records 801 browser unit tests, 20 browser scenarios and 297 Worker tests.
- The exact main software job built the 19-product native package twice,
  compared archive and sidecar bytes, verified it and installed into a fresh
  prefix. Current receipts bind tree `d4648c3ba4c297511daec7bc70edce5502279220`.
  Earlier twenty-product/private-image receipts keep their original scope.
- Selected PostgreSQL 16 evidence covers 45 active tests, with four child-entry
  controls intentionally ignored, plus the explicit navigation, backup-cut and
  owner-metadata checks and addressed current-writer/V1-wire restoration.
  The V1-wire leg uses a current SQL dump in an old receipt envelope; it does
  not qualify an authentic historical SQL ABI. Unselected real-Parmenides,
  extended OCR, independent-source and capacity profiles retain separate
  return conditions.
- Natural assessment-echo/quota and terminal-history diagnostics retain their
  exact source/image, worker and observed frontier. Receipt-quota refusal,
  byte-boundary refusal, arithmetic checks and cold reopen are different evidence.
  The prospective guard does not by itself establish every 32 MiB or PostgreSQL
  operational capacity target.
- CI selection now uses native changed-path planning and a fail-closed aggregate
  gate; unknown/shared changes select the full software route. Exact sparse
  runtime inputs include the assessment policy, owner cards, quest Markdown and
  retained-storage bindings. Required assertions and security/deadline controls
  are preserved. This changelog preparation ran no new build, test or CI job.
- PR #250's queued multi-handoff proposal remains open/unmerged and is excluded
  from shipped capabilities. Software acceptance does not close corpus semantic
  admission, public activation, research-registry core evaluation, the parallel
  ToC dialogue, the common starting point or the later 250-Luna check. No new
  ontology decisions from that parallel dialogue are included here.


## [0.5.0] - 2026-08-23

### Summary

- Consolidates the four same-day campaign Releases and tag snapshots into one dated, human-readable `v0.5.0` source release.
- The canonical source is the exact landed `main` selected by the owner PR/CI route; its final commit, tag object, and GitHub Release identity are recorded only after landing and postpublish verification.
- The direct provider edges are bound to the verified final provider identities: `aoa-stats@v0.2.0` at `88ff38b1b38eef939f2c5b4541cbe8363a05fc8d` and `aoa-kag@v0.5.0` at `f46f146cc79a26fa81ad0f400b9c5774df293e57`.
- This is a source release. It does not imply artifact admission, package or runtime deployment, runtime health, semantic proof, transport delivery, owner acceptance, or human acceptance.

### Highlights

- Tree-of-Sophia retains its source-first corpus, witness, provenance, rights, review, and bounded public-entry architecture.
- The portable content-addressed KAG family, generated readmodels, owner-local statistics boundary, and exact provider/action identities remain explicit derived or validation surfaces rather than new ToS authority.
- Historical provider corrections and release-only/generated churn are reconciled into this section without rewriting their evidence or promoting a historical pin to the current source contract.
- The full first-parent range, same-day Release bodies, tag-scoped changelog snapshots, validation results, limitations, and non-claims are accounted for in the task-local ledger.

### Changed

- All active version-bearing source surfaces name `v0.5.0`; historical `v0.5.1`, `v0.5.2`, and `v0.5.3` identities remain labeled as superseded campaign evidence.
- The release contract and Repo Validation workflow require exact final `aoa-stats` and `aoa-kag` commits; the immutable KAG workflow action commit remains a distinct identity.
- The single dated section below retains the prior release material as historical snapshots and adds the full 11/11 first-parent reconciliation.

### Validation

- Source, owner validators, CI, PR/merge, strict preflight, tag creation, GitHub Release publication, postpublish, artifact admission, runtime, proof, delivery, closure, and acceptance are separate claims.
- A green validator proves only its declared lane. Provider release identity proves source/CI dependency order, not runtime health or semantic acceptance.
- The final report must bind the exact landed source, canonical body digest, content ledger, pretruth corpus, preserved pre-campaign refs, and postpublish identity.

### Security, privacy, operator, generated, limitation, and non-claim boundaries

- Authored ToS meaning remains owner-local; generated KAG/index/export surfaces remain subordinate, reproducible, and source-returning.
- No secret, private session body, sibling repository, active dirty worktree, or protected GitHub repository is part of this cleanup effect.
- Security and privacy notes retain their historical limits: no public attestation, runtime activation, semantic proof, or human acceptance is inferred from source, CI, provider, artifact, or release evidence.
- Operator recovery, rollback, artifact, delivery, closure, and acceptance evidence remains named separately; unknown, warning, and manual-review states are not upgraded to green.

### Canonical delivery markers

Released: 2026-08-23
Canonical changelog: [CHANGELOG.md](https://github.com/8Dionysus/Tree-of-Sophia/blob/main/CHANGELOG.md)

### Historical campaign snapshots

The following four bodies are preserved as source material under one active dated section. Their old `## [version]` headings were version-bearing churn and are mapped to this canonical heading; their body lines remain verbatim except for this container heading.

#### Historical snapshot: `## [0.5.3] - 2026-08-23`


### Summary

- Revalidated the Tree consumer against the exact published `aoa-kag@v0.5.2`
  commit `8136d3eb629da28cea1206d13a8f1df52ee14739` after the exact published
  `aoa-stats@v0.2.2` commit `f119805cda69b3edeb2a4c5e407368d70e68650d`.
- Published the required immutable successor for the corrected provider and
  generated-family identities without rewriting the immutable `v0.5.2`
  release.

### Changed

- Repo Validation now fetches `aoa-stats` at the exact v0.2.2 commit and
  asserts `HEAD` equality before running the release gate.
- The release contract now names the exact v0.5.2 KAG provider commit while
  keeping the published provider release distinct from the immutable
  `repo-local-kag-index` workflow action commit `6a79e62c7d20b6b11406dee78f409ada4a51bb3f`.
- Regenerated the portable KAG family and its owner budget receipt against
  the landed Tree source after the final KAG provider revalidation.
- Aligned the current README, roadmap, release contract, tests, and changelog
  with the v0.5.3 successor contour.

### Security

- The existing v0.5.2 tag, GitHub Release, and official artifact record remain
  immutable and are not retagged, reused, or rewritten.
- A new source commit does not inherit the predecessor artifact identity; any
  v0.5.3 artifact record must bind its own exact landed Tree source ref.
- Exact provider pins and generated parity do not prove runtime health,
  artifact admission, semantic proof, or owner/human acceptance. Artifact
  gates remain separate, including `manual_review_required` where the trust
  root requires it.

### First-Parent Reconciliation (1/1)

1. This bounded successor updates exact provider identities, release markers,
   and generated KAG parity after the final KAG provider landed. It does not
   change authored ToS meaning, sibling repositories, the prior v0.5.2 tag or
   Release, or the predecessor artifact record.

### Validation

- The exact published stats and KAG provider refs, immutable action identity,
  local provider/generation checks, source release gate, hosted Repo
  Validation, and artifact route remain separate owner checks.
- The compatibility canary pins `aoa-stats` exactly but its other scheduled
  sibling inputs remain a moving lane; it is not immutable release proof.
- Source, CI, merge, tag, GitHub Release, artifact trust, runtime, proof,
  delivery, closure, and acceptance remain separate claims.

### Notes

- `v0.5.2` remains immutable historical release state, including its exact
  provider identities and artifact boundary.
- This correction changes provider/currentness and derived release evidence,
  not philosophical source meaning or canon.

#### Historical snapshot: `## [0.5.2] - 2026-08-23`


### Summary

- Revalidated the direct `aoa-stats` consumer edge against the exact published
  `aoa-stats@v0.2.1` commit
  `339ecb2db22ac4552fa88756b650896ebbff5b56`.
- Published the compatible successor needed to make the current provider
  identity truthful without rewriting the immutable `v0.5.1` release.

### Changed

- Repo Validation now fetches `aoa-stats` at the exact v0.2.1 commit and
  asserts `HEAD` equality before running the release gate.
- The release contract now names the v0.2.1 provider identity while keeping
  the published `aoa-kag` provider commit and immutable workflow action
  identity distinct.
- Revalidated the owner-local stats port, generated readmodels, local KAG
  provider, KAG export edge, and OS Abyss generated-readmodel bundle route.

### Security

- The existing `v0.5.1` tag, GitHub Release, and official artifact record
  remain immutable and are not retagged or reused for this successor.
- The exact v0.5.1 artifact record remains agent-allow and
  release/public-manual-review-required; no manual verdict is inferred or
  overwritten by this source correction.
- Exact provider pins do not prove runtime health, KAG freshness, artifact
  admission for the successor, semantic acceptance, or human owner
  acceptance.

### First-Parent Reconciliation (1/1)

1. This single successor commit updates the direct stats provider identity,
   release contract, current release markers, and generated KAG parity for
   the revalidated source surface. It does not change ToS meaning, sibling
   providers, the v0.5.1 tag or Release, or the prior artifact record.

### Validation

- The exact `aoa-stats@v0.2.1` checkout, local stats port, local KAG provider,
  generated readmodels, source release gate, and artifact bundle validator
  remain separate owner checks.
- PR/CI, merge, tag, GitHub Release, artifact trust, runtime, proof,
  delivery, closure, and acceptance remain separate claims.
- The KAG MCP owner snapshot is stale and the bounded edge search returned no
  traversable IDs; the repo-local source edge and validators remain the
  authoritative Tree evidence for this release.

### Notes

- `v0.5.1` remains immutable historical release state, including its exact
  `aoa-stats@v0.2.0` pin and official artifact record.
- A new successor source commit does not inherit the old artifact identity;
  any future v0.5.2 artifact admission must bind its own exact source ref.

#### Historical snapshot: `## [0.5.1] - 2026-08-23`


### Summary

- Corrected the direct `aoa-stats` consumer pin from an ancestor revision to
  the exact published `aoa-stats@v0.2.0` commit
  `dc608fd5de3fcaf0301f356c9efd52e2bdd350ce`.
- Made the direct `aoa-stats` and `aoa-kag` provider edges explicit in the
  release contract, including the distinction between the published KAG
  provider commit and the immutable KAG workflow action commit.
- Corrected the release evidence identity for the KAG action and preserved the
  production artifact disposition as `manual_review_required`.

### Changed

- Repo Validation now fetches `aoa-stats` at the published commit and asserts
  exact `HEAD` equality before running the release gate.
- Regenerated the pinned `aoa-kag` portable family for the corrected release
  surface and recorded its owner budget receipt; the 63 changed shard/manifest
  files are derived parity evidence, not new ToS meaning or KAG authority.
- The release contract records `aoa-kag@v0.5.0` at commit
  `813a7f69dc96ec031dad9b897a6991792cc48b7a` and its workflow action at the
  separate immutable commit
  `6a79e62c7d20b6b11406dee78f409ada4a51bb3f`.

### Security

- This corrective patch changes source/CI dependency identity only. It does
  not rewrite `v0.5.0`, publish an artifact asset, add an attestation, or
  change the production trust verdict. The production consumer remains
  `manual_review_required` until an independent owner trust gate changes it.
- Exact provider pins do not prove runtime health, KAG freshness, artifact
  admission, semantic acceptance, or human owner acceptance.

### First-Parent Reconciliation (1/1)

1. The single landed corrective-release commit updates the direct provider pin,
   records the release/federation dependency identities, and aligns the
   release markers and changelog. It is one bounded release correction, not a
   new source-meaning or sibling-repository change.

### Validation

- The local `aoa-stats` port, local KAG provider, source release gate, and
  generated readmodel artifact validator are separate owner checks.
- Repo Validation must fetch the exact published stats commit and retain the
  immutable KAG action ref; PR/CI, merge, tag, GitHub Release, artifact trust,
  runtime, proof, delivery, closure, and acceptance remain separate claims.
- The production artifact trust result remains `manual_review_required`; no
  `allow` or `warn` inference is made from the source correction.

### Notes

- `v0.5.0` remains immutable historical release state. The published provider
  identities above are evidence for this corrective source release, not moving
  sibling branches or unpublished candidates.

#### Historical snapshot: `## [0.5.0] - 2026-08-22`


### Summary

- Tree-of-Sophia grows from the v0.4.0 root-entry/statistics contour into a
  source-first corpus foundation with stable witness identity, provenance,
  rights, review, and bounded derivation routes.
- The repository adopts a portable content-addressed KAG index family with
  deterministic compatibility views and explicit generated-size/family
  receipts.
- The active ToS routing consumer now targets the `aoa-sdk` control plane
  while preserving the `aoa-routing` compatibility namespace.
- The exact v0.4.0-to-current-main range contains five first-parent commits;
  each is reconciled below rather than inferred from the former `[Unreleased]`
  prose alone.

### Changed

- Reconciled the five-commit release range, including the KAG storage-family
  migration, source-first corpus foundation, routing-consumer migration,
  accepted provider-family pin, and completion of the foundation source tree.
- The release consumer contract now names ABI, source/trust-root matching,
  durable evidence, materialized subject stores, fail-closed admission, and
  the distinction between authored ToS meaning and generated readmodels.

### Added

- Planted the exact A01 `ATU 5 numerical texts layer [53]` backlog row through
  Robert K. Englund's 1994 *Archaic Administrative Texts from Uruk: The Early
  Campaigns*, one Work carrying both ATU 5 and ADFU 15 identifiers. Nissen's
  2024 publication history proves that numerical-only tablets omitted from
  ATU 1 were published in ATU 5, so the layer remains a source-backed subset
  rather than a fabricated second Work or the distinct ATU 2 numerical-systems
  analysis. Publisher `ca. 820` and current CDLI `897` observations remain
  dated and non-equivalent; Boehmer's exact contribution wording, direct
  reviews, current PCSL/Unicode controls, layered CDLI rights, and an unsent
  DAI research-access route are preserved. No source bytes, exact tablet set,
  sign/value claim, image, transcription, semantics, graph/canon promotion,
  external message, human task, or redistribution permission was created.
- Planted the exact A01 `ATU 3 — Die Lexikalischen Listen der archaischen
  Texte aus Uruk [24]` backlog row as a standalone bibliographic Work. Ordered
  official/classical, established, and current 2025–2026 research preserves
  Englund and Nissen's authorship separately from Damerow's reported
  contribution, identifies LLATU as a page-and-entry coordinate inside the
  1993 volume, and keeps LLATU labels, ZATU identities, PCSL OIDs, proposed
  Unicode values, readings, glosses, and concepts distinct. A current
  Heidelberg holding and reproduction route supports one tiered, unsent local
  research-access request. No book bytes, list text, sign inventory,
  concordance, semantics, graph/canon promotion, human task, external message,
  or redistribution permission was created.
- Planted the exact A01 `ATU 2 — Zeichenliste der archaischen Texte aus
  Uruk [24]` backlog row through the existing bibliographic spine. Ordered
  official/classical, established, and fresh research resolves the 377-page
  1987 volume as an aggregate Collection, keeps Margaret W. Green's Sign List
  at pp. 167-345 separate from Damerow and Englund's numerical-systems Work at
  pp. 117-166, rejects a review and a 56-page chapter as false full-book
  routes, and prepares one tiered library access request without sending it.
  The planting now binds an exact Work, Collection, and `contains_work` claim;
  ZATU coordinates, PCSL OIDs, and provisional Unicode values remain distinct.
  No source payload, sign entry, reading, concordance, semantics, graph/canon
  promotion, human task, or redistribution permission was created.
- Added a provider-independent scholarly-composite witness spine and planted
  the exact A01 `DCCLT link from P000015 [24]` backlog row through DCCLT
  `Q000023` / CDLI `P471693`. Ordered official/classical, established, and
  fresh research now separates the modern documentary composite, physical
  members, member transliterations, editorial coordinates, provider pages,
  rights, and interpretation. The exact CDLI API reports 98 members and
  includes P000015 through relation 31, while the current DCCLT Q-page exposes
  36 source links without P000015; both remain dated representation-coverage
  observations instead of a flattened contradiction. DCCLT's explicit CC
  BY-SA 3.0 route is retained positively for future attributed/share-alike
  content, while external media remain separately governed. The first packet
  stays text-free and public-metadata-only; it creates no ancient-original,
  source-text, sign, translation, semantic, graph, canon, publication, or
  human-review authority.
- Added a participant-scoped translation-exposure contract and text-free
  Zarathustra pilot plan without rewriting the frozen historical translation
  lifecycle. Ordered official/classical, established, and fresh 2025–2026
  research now separates five-year Work-level lived exposure from the exact
  Antonovsky 2007 page already seen in the current project, keeps the
  historical Expression/Item/wording unconfirmed, treats model-training
  exposure as unknown, permits only operational AI context-isolation claims, and labels
  the mixed lane `exposure-aware-assisted-retranslation`. Source-, target-,
  process-, and influence-facing evidence stay separate; a human episode is
  triggered at most once for a concrete candidate and requires no retyping or
  UI work. Digest-bound provenance and negative controls keep German
  competence, accepted translation, comparator reveal, model execution,
  human tasks, semantics, graph/canon effects, publication, and server
  transfer at zero.
- Added `tos_source_text_unit_packet_v1` as the owner of stable segmentation
  and text-unit identity over one exact frozen text layer. Ordered Unicode,
  ICU, TEI, Web Annotation, CoNLL-U, ISO/LAF, established segmentation work,
  and fresh 2025–2026 historical-text evidence now keep physical layout,
  source-observed structure, orthographic and linguistic units, model-input
  pieces, and non-surface analytic nodes distinct. Opaque IDs, exact anchors,
  coverage, gaps, overlap, whitespace, punctuation, line breaks, hyphenation,
  reciprocal alternatives, scoped review, projection, and visibility remain
  explicit. A public-synthetic A/B/C exercises two competing sentence and two
  competing hyphen-token analyses and rejects model-shaped acceptance plus
  twenty-five negative controls. It creates no real boundary, token, German
  competence, human review, semantic identity, translation, graph, canon,
  migration, or publication authority; the legacy
  `tos-local-sentence-segmentation-v1` label is unchanged.
- Added `tos_translation_alignment_packet_v1` without mutating the existing
  translation-packet v2 lifecycle. Ordered TEI, Web Annotation, XLIFF, TMX,
  OntoLex/VarTrans, PROV-O, classical/established alignment research, and fresh
  2025–2026 historical and literary evidence now bind exact source/target
  witness and frozen text-layer states, segmentation/tokenization, ordered
  anchors, opaque alignment/claim identity, cardinality/order/unaligned states,
  competition, review, projection, and most-restrictive visibility. A
  public-synthetic invented-language A/B/C validates one-to-one and reciprocal
  one-to-one/one-to-many proposals, rejects false acceptance, and exercises
  seventeen controls. It creates no translation, aligner/model act, human
  review, accepted alignment, lexical equivalence, export authority, graph,
  canon, corpus migration, or publication authority.
- Added `tos_semantic_annotation_packet_v2` as a source-first, stand-off
  semantic identity and assertion foundation. Ordered Web Annotation, SKOS,
  OntoLex-Lemon/VarTrans, TEI, CIDOC/CRMinf, Wikibase, established ontology and
  nanopublication work, and fresh 2025–2026 historical-language and
  human-in-the-loop research now keep occurrences, lexemes, senses, proposed
  annotations, promoted signs, concepts, versioned claims, relations, reviews,
  and graph projections distinct. A public-synthetic A/B/C preserves exact
  anchors, two reciprocal competing sign proposals, and one deliberately
  invalid model-shaped promotion; twelve negative controls fail closed. The
  fixture creates no real model act, human review, semantic truth, graph edge,
  publication authority, or canon object, and existing sign ladders remain
  unchanged.
- Added `tos_provenance_event_v2` as an additive immutable execution-receipt
  foundation while preserving all legacy v1 records. Ordered W3C PROV, PREMIS,
  in-toto/SLSA, RO-Crate/Croissant, established provenance research, and fresh
  2025–2026 reproducibility, agent-trace, digital-humanities, and inspectable-AI
  work now separates exact inputs, authoritative outputs, diagnostic
  byproducts, derivation, command/configuration/runtime/model capture,
  responsibility, manual changes, measurements, authentication, review,
  rights, and bounded replay posture. A public-synthetic A/B/C executed an
  exact copy, NFC transformation, and honest exit-7 failure; fourteen negative
  controls failed closed. The receipts remain unsigned and prove neither
  execution truth, content quality, human competence, publication, semantics,
  graph truth, nor canon.
- Added `tos_source_text_layer_v1` as the immutable textual-representation
  floor above exact source anchors. Ordered TEI, OCR-D, Web Annotation,
  PROV-O, IIIF, Unicode, established digital-edition, and fresh 2025–2026
  correction-provenance research now separates raw OCR, diplomatic candidate,
  reviewed reading, and normalization; binds predecessor, content, editorial
  policy, and explicit code-point edits; and keeps mechanical, review,
  language-competence, accepted-use, rights-record, and publication-authority
  gates distinct.
  A public-synthetic real-byte A/B/C replayed both transitions exactly and nine
  negative controls failed closed. No existing record was migrated and no
  accepted text, competence, translation, semantic claim, graph truth, human
  task, publication, or canon authority was added.
- Added an explicitly additive `tos_source_anchor_v2` foundation without
  reinterpreting or migrating current v1 anchors. Ordered W3C Web Annotation,
  TEI, IIIF, EPUB/IETF, established robust-anchoring, and fresh 2025–2026
  provenance research now separates immutable target identity, exact
  representation state, Unicode-code-point half-open text positions, byte
  positions, selector alternatives, ordered refinement, copied-text
  publication posture, mechanical resolution, and human review. A public-only
  synthetic JSON/text/container A/B/C resolved three exact selections; direct
  `jq`, `rg`, `xmllint`, and Python inspection confirmed the actual material,
  while six negative controls rejected offset-unit substitution, divergent
  alternatives, digest drift, reversed refinement, nonpublic quote leakage,
  and region overflow. No private source, accepted text, translation, semantic
  claim, graph truth, human task, publication, or canon effect was added.
- Closed the declared-content gap in future goal-attempt receipts. Reviewed
  `abyss-stack` checkpoint `55e58d0a` adds the strict
  `tos_goal_attempt_run_receipt_v1` contract and binds each exact receipt to
  its ledger identity, ordinal, status, UTC interval, terminal event, and
  applicable process exit code while rejecting duplicate keys and non-JSON
  constants. Ordered provenance/run-lifecycle research is retained. A
  conforming receipt remains unsigned declared evidence: it does not prove
  execution, producer identity, environment completeness, result truth,
  content quality, human acceptance, publication, or canon authority.
- Bound future monetary accounting to exact evidence without rewriting history.
  The reviewed `abyss-stack` v2 route verifies relative regular-file bytes,
  byte lengths, SHA-256, component-reference closure, and one run receipt for
  every ordered attempt while preserving v1 and labeling local integrity
  `unsigned-not-attested`. Ordered W3C PROV, in-toto, SLSA, RO-Crate, FAIR,
  reproducibility, and fresh agent-trace research is retained with explicit
  limits. No old run gains a price, no evidence statement gains truth or
  producer authenticity, and no source, model, human task, semantic claim,
  publication, or canon authority is added.
- Reconciled the foundation completion audit with the existing initial-sign
  owner route: the complete four-part exact-form recurrence remains a
  queryable, source-observed, non-promoting sidecar. It does not change
  semantic-ladder v5, open stages 4–15, create human work, or establish a
  lemma, sense, motif, sign, semantic claim, relation, graph truth, or canon.
- Preserved the already completed 2026-07-26 source-visible, method-blind,
  solo-human OCR candidate-review pass as one append-only aggregate manual
  error-ledger episode. It closes 10 source pages, 30 candidate dispositions,
  aggregate outcome/error counts, and 8,334 seconds of explicitly confounded
  human time to the governed private handoff, public-safe derivative, and
  provenance digests. No source text, unit judgment, reviewer identity,
  private locator, accepted transcription, independent gold, method winner,
  content authority, routine human backlog, or publication authority is added.
- Preserved the first contextual morphology B artifact attempt as an additive,
  text- and path-free denial receipt without rewriting its digest-bound
  pre-output plan. The exact ZDL `de_zdl_lg 4.0.0` wheel and 41-wheel CPython
  3.12 closure were acquired privately under the owner resource gate, but the
  required signature, model-license evidence, non-local trust root, release
  lifecycle, full verification, and subject closure did not exist. The exact
  runtime trust gate denied admission; no runtime was built, no source packet
  was consumed, and no model output, German judgment, morphology, lemma,
  lexeme, semantic effect, human task, publication route, or canon effect was
  created.
- Recorded the later contextual morphology B execution without rewriting that
  negative evidence. An admitted successor first exposed a source-free
  spaCy/Typer/Click import gap; the corrected 42-wheel subject then closed all
  five `local_dev` controls, a 44-file subject store, exact `agent` admission,
  and an isolated Python 3.12 smoke before reading the three frozen contexts.
  Both provider passes are byte-identical and all three targets align to one
  token, but the tracked result remains text-free and
  `unmeasured-no-german-competent-gold`. Private source/output redistribution,
  accepted tokenization, morphology, lemma, lexeme, sign, semantics, graph,
  canon, publication, and automatic human work remain closed.
- Reconciled Antonovsky's exact 1900 *Zarathustra* Edition across current RSL,
  RNL, and LNB records. RSL and RNL share the `rc\\1717107` control core,
  transaction value, and bibliography; the crosswalk resolves RSL
  `01003693380`, RNL Primo `07NLR_LMS004843721`, system number `004843721`,
  and three Russian remote holdings alongside LNB `1.40686`. LNB's `[s.n.]`,
  `[3], XI` description remains explicitly discrepant from RSL/RNL's
  `тип. Б.М. Вольфа`, `[4], XII` until source pages can adjudicate it. The LNB
  request is refocused but unsent. No source page, digital object, ToS Item,
  exemplar claim, publisher, right, human task, accepted text, semantic object,
  publication, or canon state was added.
- Reconciled Antonovsky's exact 1903 second corrected *Zarathustra* Edition
  across current RSL and RNL records. The shared `rc\\1717108` control core,
  identical transaction value, and matching bibliography resolve RNL Primo
  record `07NLR_LMS004843722` and system number `004843722` without equating
  physical copies or texts. Two current RNL shelfmarks join the four RSL
  remote holdings; one undefined `TEMP` row and a generic RSL online-reading
  modal are retained as rejected evidence. A separate RNL request is prepared
  but unsent. No source page, digital object, ToS Item, exemplar claim, right,
  human task, accepted text, semantic object, publication, or canon state was
  added.
- Resolved the current RNL holding route for Antonovsky's exact 1907 third
  *Zarathustra* Edition. RNL Primo record `07NLR_LMS004843723`, system number
  `004843723`, tagged RUSMARC, the locations tab, and public ALEPH item rows
  expose three Russian Book Fund shelfmarks and `В хранении` states. The
  catalogue's `Web заказ` code is not promoted to actual 1907 online-order
  eligibility, and the exact scan form remains recommendation-only. The RNL
  request is revised but unsent; remote holdings remain outside the ToS Item
  ladder, and no source page, payload, exemplar claim, right, human task,
  accepted text, semantic object, publication, or canon state was added.
- Executed one separately admitted resident Gemma E4B successor capacity probe
  without reopening the closed historical translation A/B/C. A first
  no-candidate runner-contract failure is retained independently; the corrected
  route produced one schema-valid private candidate, which AI Russian-surface
  triage rejected before human review for obvious defects. A new reusable,
  text-free experimental-episode contract separates failure, fixity, quality,
  speed, resource, monetary, energy, and human-cost evidence. No source or
  candidate text, private path, human task, accepted German, translation,
  semantics, graph truth, publication, or canon effect was added.
- Admitted the exact eKGWB `Za-I-Vorrede-1` locator and local-only observed
  reading as one edition-typed citation witness for private non-commercial
  AI-only and AI+human experiments. The additive decision keeps accepted
  German, human-only translation, publisher authentication, public sharing,
  semantics, graph truth, and canon effects closed. The exact frozen Gemma E2B
  A run was integrity-verified rather than rerun, recorded through a new
  text-free experimental-candidate contract, and rejected before human review
  because of obvious Russian-surface defects. No source or candidate text was
  added to Git and no human correction task was created.
- Refreshed the Stanford Loeb–Tinsley *Thus Spoke Zarathustra* release
  frontier on 2026-08-08 in official -> established -> freshest order. The
  register now distinguishes cloth, paper, and eBook ISBNs, preserves the
  publisher's August/424-page baseline beside the retailer-supplied
  anticipated 2026-08-11 date and conflicting 424/552 extents, and records
  specialist review only for the translators' earlier volume 14. The stable
  v1 entry remains metadata-only, unacquired, blind to translation lanes, and
  blocked on exact manifestation, rights, reception, and suitability review;
  no request, source text, translation authority, human task, semantics,
  publication, or canon was added.
- Added an exact source-returnable `translated_by` claim for the separately
  identified 2007 Cultural Revolution *Zarathustra* Expression. Direct visual
  inspection of the fixity-verified local PDF anchors the translator credit on
  page 3 and the fuller responsibility/revision block on page 4. Ordered IFLA
  and BIBFRAME documentation, current RSL/NEL records, established translation
  scholarship and the contemporary 2007 review, fresh 2025–2026 work, and
  general web last establish the boundary: the Antonovsky lineage is not one
  invariant text. General editing, scientific editing/checking, commentary
  translation, and design remain exact deferred-role evidence rather than
  inaccurate generic edges. The local payload and fail-closed rights posture
  are unchanged; no source text, equivalence, accepted translation, quality,
  semantics, human review, publication, or canon was added.
- Added exact Edition-owned provision identity for the fixity-verified 1892
  second edition of *Zur Genealogie der Moral*. PDF page 5 anchors `LEIPZIG /
  Verlag von C. G. Naumann. / 1892.`, while PDF page 204 independently anchors
  the undated `LEIPZIG / Druck von C. G. Naumann.` line. Separate publication
  and manufacture claims reuse the provisional GND-backed publisher, printer,
  and Leipzig identities without asserting legal unity or separation. Ordered
  exact-item -> official-authority -> independent institutional copy ->
  established scholarship -> freshest 2025-2026 -> general-web-last research
  found no later correction to the two source-visible statements. No exact
  printing-completion or release date, source text, accepted German, human
  review, rights clearance, semantics, publication, or canon was added.
- Added exact Edition-owned provision identity for the fixity-verified 1886
  *Jenseits von Gut und Böse* Item. Direct inspection of PDF pages 3 and 4
  preserves `Leipzig / Druck und Verlag von C. G. Naumann. / 1886.` as one
  source literal supporting separate publication and manufacture claims, with
  distinct provisional GND-backed publisher and printer Organizations. An
  ordered exact-source -> official-authority -> independent e-rara ->
  established-genetics -> fresh-2025 -> general-web-last pass supplies the
  research trail. `Alle Rechte vorbehalten.` remains historical evidence, not
  a current legal result; rights stay undetermined and local-only, with
  redistribution and derivative use unknown. No payload, Edition statement,
  exact release date, accepted German, human review, semantics, publication,
  or canon was added.
- Added exact Edition and role-separated provision identity for the consolidated
  1893 Naumann *Also sprach Zarathustra*. Direct inspection of the
  fixity-verified PDF page 46 records `Zweite Auflage, mit Portrait und
  Brieffacsimile des Autors.` and the combined literal `LEIPZIG / Druck und
  Verlag von C. G. Naumann. / 1893.` as one Edition statement plus separate
  publication and manufacture claims. Current GND records supply distinct
  provisional publisher and printer Organizations without asserting legal
  identity, separation, succession, or `same_as`. Page 47's
  `Uebersetzungsrecht vorbehalten.` is retained only as historical rights
  evidence; current rights remain undetermined, local-only, and not authorized
  for redistribution. No source payload, accepted German, universal Edition
  ordinal, release date, human review, semantics, publication, or canon was
  added.
- Added separate publication and manufacture provision claims for the exact
  1913 Antonovsky *Zarathustra* Edition. Direct inspection of the
  fixity-verified local research copy returns to three distinct source
  surfaces: the cover names `ИЗДАНІЕ «ЖИЗНЬ ДЛЯ ВСѢХЪ»`, the title page names
  `С. ПЕТЕРБУРГЪ` and `1913` without a publisher, and the following page names
  `Типографія Бр. В. и И. Линникъ`. Three proposed whole-page anchors, one
  provisional Saint Petersburg Place, and two distinct provisional
  Organizations preserve those roles. Current Commons, GND, RSL, NEL,
  Presidential Library, and established scholarship were checked before
  general web search. Vladimir Posse is not substituted for the displayed
  imprint, the printer is not the publisher, and no edition number, release,
  text, translation judgment, source payload, human review, rights clearance,
  semantics, publication, or canon was added.
- Added one independently evidenced Edition-owned provision claim for
  *Also sprach Zarathustra*, part IV. The exact DTA record supplies only the
  reported `Naumann; Leipzig; 1891` statement; current GND records keep the
  historical publisher Organization, Leipzig Place, related printer, and
  founder Person distinct. DLA, HAAB, and HAdW evidence preserves the 1885
  private print, November-1890 printing, planned spring-1891 delivery, and
  March-1892 actual delivery as separate stages, while the forty-versus-
  forty-five private-copy conflict remains unresolved. The source catalog and
  claim-first graph expose the new unreviewed public-metadata route without a
  direct fact edge. No physical-title-page transcription, release date,
  accepted bibliography, payload, rights clearance, human review, semantics,
  publication, or canon was added.
- Added two independently evidenced Edition-owned provision claims for
  *Also sprach Zarathustra*, parts II and III. Each returns to its own DTA
  source description, URN, volume, holding-copy route, publisher, place, and
  statement year; e-rara supplies an independent holding route with separate
  Band 2 and Band 3 title-page structures. The claims reuse the provisional
  Chemnitz Place and historical Schmeitzner publishing Organization but are
  not copied from part I or from repeated labels. Exact subject queries return
  three distinct part-I/II/III claims; the Person GND remains a negative
  control. No physical-title-page transcription, printing or release date,
  Edition/Item/textual equivalence, accepted bibliography, human review,
  source payload, semantics, publication right, or canon was added.
- Added an exact source-returnable translation-responsibility claim for the
  1913 Russian Antonovsky *Zarathustra* Expression. Direct inspection of the
  fixity-verified PDF page 7 supplies one proposed whole-page source anchor;
  the claim reaches the existing provisional Antonovsky Agent without trusting
  the ABBYY OCR layer, expanding the displayed initials into an accepted
  person identity, or equating the 1913, 1996, and 2007 Expressions. Standalone
  source anchors beside an Expression are now schema- and provenance-validated
  as responsibility evidence. No source text, page image, accepted translation,
  quality judgment, human review, rights clearance, semantics, or canon was
  added.
- Extended the retained provision-activity mechanics with one independently
  researched, exact-Edition case for *Also sprach Zarathustra*, part I
  (Schmeitzner, Chemnitz, 1883). The official identity route distinguishes the
  historical publishing Organization (GND `1063670306`) from the Person Ernst
  Schmeitzner (GND `118823698`), binds the authority statement to the existing
  fixity-verified DTA first-part witness, and at that checkpoint left parts II
  and III unpopulated. Positive Chemnitz/Organization queries and negative
  part-II, part-III, and Person-GND queries were inspected directly before the
  later exact-record extension above. The original
  two-case A/B/C measurements remain frozen; this extension adds no human
  review, source payload, accepted bibliography, semantics, rights, or canon.
- Added the first bounded Edition-owned `provision_activity` layer after an
  ordered standards, established-work, and 2025-2026 freshness review. The
  1889 *Götzen-Dämmerung* and 1908 *Ecce Homo* claims preserve their exact
  transcribed or reported Leipzig statements, typed publisher roles, and
  statement-date postures while resolving one provisional Place and two
  distinct provisional historical publisher Organizations. Catalog v3,
  validation, graph projection, and exact normalized-identity queries keep
  every path claim-originating and source-returnable. A direct A/B/C inspection
  rejects flat fields, invalid dates, printer/successor substitution,
  statement/public-release collapse, and direct fact edges. All records remain
  model-made, provisional or `unreviewed`; no source payload, bibliography,
  semantics, graph truth, or canon was promoted.
- Added a first-publication chronology branch for all seven current Nietzsche
  Works. A dedicated object contract and digest-bound provenance event preserve
  interval precision, sequence stages, public/private availability,
  author-supervised versus posthumous posture, and explicit ordering warnings.
  The staged *Zarathustra* profile distinguishes its 1883–1884 public parts
  from the private 1885 fourth part; *Götzen-Dämmerung* distinguishes 1888
  production from 1889 release. These are model-made, unreviewed ordering
  claims—not composition dates, accepted bibliography, or a universal
  philosophical timeline.
- Added an authored bibliographic-topology spine under
  `ToS/source-witnesses/relations/`. It gives all 20 Work→Expression, 20
  Expression→Edition, and 15 Edition→Item links stable claim IDs, exact
  record/manifest evidence, a digest-bound model provenance event, public
  metadata visibility, and explicit `unreviewed` posture. Work, Expression,
  and Edition records now close over the exact outgoing claims; validation
  rejects missing, extra, duplicated, cross-typed, or misbound links. These 55
  packets materialize declared topology only: `embodied_by` does not establish
  textual identity, critical equivalence, accepted text, semantics, rights, or
  canon.
- Added a deterministic read-only query route for the source-witness
  bibliographic graph. Exact claim, subject, identity-object, normalized
  identity, predicate, review-status, and visibility selectors use AND
  semantics and return the
  exact source JSONL object, file/line/digest, reified claim, typed
  subject/object, evidence, maker, provenance, review nodes, and all
  claim-centered edges. Every query first validates and source-rebuilds the
  complete graph. Selector-free requests and silent over-limit truncation are
  rejected; results are stdout-only and create no truth, review, runtime,
  service, rights, or canon authority.
- Closed the current Work-authorship surface across all seven Nietzsche Works.
  Six new Work-local `authored_by` packets join the existing *Ecce Homo* claim;
  every Work now cites exactly one claim to the GND-backed Friedrich Nietzsche
  Agent. Each new packet returns to its own documentary authorial-witness route
  and digest-bound annotation event, remains model-made,
  `public_metadata_only`, and `unreviewed`, and creates no generic creator
  shortcut, author-final text, semantic relation, graph truth, or canon.
- Added the first corpus-wide source-returnable bibliographic graph under
  `ToS/derived-exports/graph/`. It now deterministically reifies all 115 current
  public-metadata-only source-witness claims into 393 nodes, 801 claim-centered
  edges, and 115 complete trace packets. Every edge carries the canonical claim
  digest, exact source file/line, evidence nodes, maker, provenance event with
  time and method, and explicit review status. Thirty-three publication,
  chronology, and provision objects remain claim-scoped literals; no direct
  subject-to-object truth edge is emitted. The graph is a deletable JSON read
  model, separate from the
  atlas/view projection and subordinate to source claim packets; it creates no
  review, bibliography, generic creator relation, semantics, rights,
  Neo4j/Oxigraph, runtime, or canon authority.
- Extended the generated source-witness catalog from object-only navigation to
  an exact source-returnable object-and-claim projection. The new
  `claims.jsonl` indexes all 115 tracked membership, responsibility,
  publication, provision, chronology, and identity-ladder packets with their subject, predicate, object, evidence, maker,
  provenance event, review posture, exact source file/line, and canonical
  claim digest. Catalog v3 separates 74 object entries from 115 claim entries,
  validates both entry classes, and fails parity when an authored packet or
  projection drifts. It also rejects local, restricted, or
  permission-pending claims rather than copying them into the tracked
  projection. All current claims remain bibliographic, model-made,
  public-metadata-only, and unreviewed; the catalog is navigation rather than
  relation truth, review, graph authority, semantics, or canon.
- Added the first multi-role responsibility-claim prototype without generic
  creator flattening. Four model-made, unreviewed `tos_claim_packet_v1`
  records keep Nietzsche's `authored_by` relation on the *Ecce Homo* Work
  separate from Raoul Richter's `edited_by` and `afterword_by` relations and
  Henry van de Velde's `designed_by` relation on the exact 1908 Insel Edition.
  All three Agents resolve to current GND identities; Work and Edition records
  close over their own claims; and an annotation event digest-binds the claim
  file to evidence. Source validation now enforces predicate-specific subject
  classes, Agent closure, provenance, and orphan/cross-subject rejection for
  both the new roles and existing translation claims. This establishes
  responsibility mechanics, not accepted bibliography, creator equivalence,
  author-finality, editorial extent, rights, graph truth, or canon.
- Added a contrasting Edition-owned publication claim family for the 1888
  *Der Fall Wagner* Edition. Six unreviewed, evidence-bearing
  `tos_claim_packet_v1` records separate the observed title-page year from
  reported author-supervised first publication, month-precision printing
  completion, official publication date, one 1000-copy run, and its nominal
  `Zweite Auflage` latter half. The final object keeps textual identity and
  textual difference independently `unresolved`, records that the current
  Item is unlabelled, and creates no second Item. The Edition closes over the
  exact claim set and an annotation event digest-binds it to evidence. This
  proves uncertainty-preserving claim mechanics, not human acceptance,
  collation, critical equivalence, publication authority, graph truth, canon,
  or a mandate to populate every Edition.
- Added the first Edition-owned publication claim family: six unreviewed,
  evidence-bearing `tos_claim_packet_v1` records for the 1889
  *Götzen-Dämmerung* Edition. They separate the observed title-page year from
  reported printing, authorial receipt, sale release, first-publication role,
  and the changed non-author-approved 1893 editorial state. The sibling
  Edition references every claim, the annotation event digest-binds the
  claim file, and source validation rejects unresolved, cross-edition, or
  orphaned publication claims. This creates neither a remote 1893 Item nor
  accepted bibliography, text, semantics, graph truth, or canon.
- Added the metadata-only *Götzen-Dämmerung* authorial, publication, and
  critical route through current GSA ORES/Kalliope, the existing 1889
  MDZ/BSB Item, Nietzsche Source, HAdW, KSA/KGW, Montinari, Pichler, current
  chronology/method research, the 2024/2025 *Magnum in parvo*
  reconstruction/review, and the 2025 direct interpretive cluster. It keeps
  mixed W II material region-only, positively identifies D 21/GSA 71/28,
  preserves late additions and section migration, separates Köselitz's title
  suggestion from Nietzsche's adoption, distinguishes 1888 production from
  the 1889 title-page and sale dates, and keeps the changed,
  non-author-approved 1893 second edition separate. No remote source body,
  new Item, accepted German, reconstructed author-final text, semantic or
  transfer unit, human backlog, rights clearance, or publication permission
  is created.
- Added the metadata-only *Der Fall Wagner* authorial, publication, and
  critical route through current GSA ORES, University Library
  Basel/e-manuscripta, the existing 1888 MDZ/Bamberg Item, Nietzsche Source,
  HAdW/KGW IX, Röllin's 2024 layer chronology, 2025 current records, and a
  2026 metadata-only chapter. It preserves exact-region-only W II 6/W II 7
  attribution, explicitly rejects D 21/GSA 71/28 for this work, distinguishes
  two lost complete manuscript states from the sole surviving Basel correction
  leaf, separates Nietzsche's changes from Köselitz/Lauterbach provenance
  notes, and refuses to infer textual identity or difference between the
  unlabelled and nominal `Zweite Auflage` print-run halves. No remote source
  body, new Item, accepted German, reconstructed manuscript, semantic or
  transfer unit, human backlog, rights clearance, or publication permission
  is created.
- Added the metadata-only *Der Antichrist* authorial, editorial, and critical
  route through current GSA ORES/Kalliope, University Library Basel A 311,
  the Koegel/Naumann 1895 publication history, Nietzsche Source, HAdW/KGW IX,
  2025 direct genesis and method work, and a 2026 intertextual lead. It
  preserves exact-region-only W II attribution, D 22's material and
  responsibility layers, A 311 as an independent copy, four 1895
  interventions, the later 1906 aggregate Item, and GSA 71/32 fol. 47 as a
  contested associated adjunct rather than `AC 63`. No remote source body,
  new Item, accepted German, author-final reconstruction, semantic or transfer
  unit, human backlog, rights clearance, or publication permission is created.
- Added the metadata-only *Zur Genealogie der Moral* authorial and critical
  route through current GSA ORES/Kalliope, HAAB correction evidence, the
  University of Basel D 20/K 11/E 40 documentary edition, DFGA/eKGWB,
  established HAdW/genetic research, and 2024-2026 freshness evidence. It
  preserves region-only W II 1 attribution, split D 20a/D 20b responsibility,
  partial C 4616 coverage, unresolved later copies, first-print versus 1892
  versus critical-text separation, the current Nietzsche Source timeout, and
  open-view/rights asymmetries. The named document stages remain outside the
  frozen transfer A/B/C variants and create no remote Item, source payload,
  accepted German, semantic or transfer unit, human backlog, or publication
  permission.
- Added the metadata-only *Jenseits von Gut und Böse* authorial and critical
  route through current GSA ORES/Kalliope, HAAB correction witnesses, DFGA,
  eKGWB, an independent e-rara first print, established genesis scholarship,
  and 2024-2025 textual-genetic work. It preserves region-only W I/Mp
  membership, D 18's internally layered assembly, C 4615/C 4619 correction
  responsibility, representation-specific counts, the current Nietzsche
  Source timeout, 302 critical addresses, and a deferred §22
  early-region -> print-manuscript -> publication/critical route. Those stages
  are explicitly separate from the golden-kernel A/B/C variants. The route
  creates no remote Item, source payload, accepted German, author-final
  synthesis, transfer unit, semantics, human backlog, or publication
  permission.
- Added the metadata-only *Also sprach Zarathustra* authorial and critical
  route through current GSA ORES, DFGA, eKGWB, KSA/HAdW, the corrected Peter
  Gast private-print ensemble, and 2025-2026 freshness evidence. The route
  separates mixed notebooks, the unpreserved print manuscripts for parts
  I-III, D 17 (`GSA 71/25`) for part IV, GSA 71/25a, 181 critical addresses,
  and the forthcoming August 2026 Loeb/Tinsley Stanford translation. It
  creates no remote Item, manuscript or critical-text payload, accepted
  German, translation authority, human backlog, semantics, or publication
  permission.
- Added the metadata-only *Ecce Homo* authorial and critical route through
  current GSA ORES, DFGA, eKGWB, Kalliope, HAdW, and 2024-2025 philology:
  D 25 (`GSA 71/32`), D 25a (`GSA 71/33`), the surviving replacement copy,
  the undigitized deletion/change file, four related draft complexes, stable
  critical addresses, layered access/rights evidence, two exact identifier
  conflicts, and one false-2026 freshness correction. No manuscript or
  critical source body, local Item, accepted German, human backlog, semantics,
  or publication authority is created.
- Added the first exact historical German source tree for *Ecce homo*: the
  posthumous 1908 Richter/Insel edition, Getty copy 965, through permanent
  Wikimedia Commons revision `1074063624`; exact local fixity; a text-free
  166-page inventory; DNB/IA/Open Library pagination-conflict preservation;
  source-visible separation of Nietzsche's body from Richter's afterword;
  ordered official, established, 2025, and restricted 2022-lead evidence; fail-closed
  rights; and a metadata-only future-server plan. The item is historical soil,
  not an author-final or critical text, accepted OCR/German, semantics, or
  publishable local payload.
- Added exact standalone German historical source routes for *Der Fall Wagner*
  (the unlabelled 1888 Naumann issue, Bamberg copy `30.1972`) and
  *Götzen-Dämmerung* (the 1889 Naumann first-publication witness, BSB copy
  `Ph.pr. 881 kc`): ordered authority/scholarly/institutional discovery,
  official MDZ PDFs with local fixity, text-free 75- and 165-page inventories,
  source-visible issue and edition-risk records, fail-closed rights, focused
  regression coverage, and metadata-only future-server plans. Neither item
  accepts OCR or German text, becomes a critical edition, authorizes upload of
  the operator-held PDF, or makes prepared transfer pages eligible for A/B/C.
- Added an exact German historical source route for *Der Antichrist* inside
  the 1906 Naumann *Nietzsche's Werke*, Band VIII aggregate: ordered
  discovery, a checksum-reconciled Wikimedia Commons/Stanford DjVu, a
  text-free 523-page bundled-DjVu inventory, an explicitly partial
  work-boundary map, fail-closed rights, and a metadata-only future-server
  plan. The 1906 witness is not equated with Nietzsche's manuscript, the 1895
  first printing, a critical text, accepted German, or publishable content.
- Added an exact German source tree for the 1892 second Naumann *Zur
  Genealogie der Moral*: ordered discovery evidence, a checksum-reconciled
  Wikimedia Commons/UNC scan revision, text-free 208-page inventory,
  fail-closed rights record, and metadata-only future-server plan. The local
  PDF, embedded OCR, German text, and all content derivatives remain
  unpublished and unaccepted.
- Added a current eKGWB rights and transport assessment that separates
  permitted private non-commercial adaptation from prohibited sharing of
  adapted material while keeping the HTTP witness unauthenticated, local-only,
  unadmitted, and publication-blocked.
- Added protocol-native institutional corroboration for the eKGWB target:
  Arquivo.pt WARC lineage and an exact block match across 2023, 2024-dated, and
  2026 owner surfaces, while preserving the archive's non-validation disclaimer
  and leaving publisher-origin authentication and source admission open.
- Added a text-free, schema-bound return receipt and independent recorder for
  the exact owner-local Zarathustra direct-page-image retrieval C run,
  preserving private artifact fixity, normalization, resource cost, r8/r9
  audit lineage, and rare-review triggers without source/query content or
  promotion claims.
- Added the Zarathustra golden-growth-kernel architecture and accepted
  `TOS-D-0019`, preserving the founder-borne five-year lived provenance while
  keeping lived witness below source, scholarly, and canon authority.

### Changed

- Reconciled the foundation retrieval, cost, limitations, completion-audit,
  and promotion reports with the unforced audit-complete Qwen3-VL r9 result;
  human relevance, method adoption, winner, and publication remain open.
- Coupled the world-philosophy corpus-soil direction to a first complete
  Zarathustra learning cycle across root design, source-home doctrine, roadmap,
  route cards, and review law.
- Clarified that agents transfer source-return and review method—not
  Nietzsche's ontology—and that gold material must preserve rejected,
  ambiguous, and counter-reading outcomes rather than accepted labels alone.

### Fixed

- Fixed source-witness inventory admission so manifest payload byte-size and
  SHA-256 fixity are required before inventory membership.
- Fixed empty philosophy-branch manifest consistency and added a stale-manifest
  negative control.
- Fixed lexical local-database verification and regenerated the landed
  portable owner-family receipt against the source tree.
- Fixed the release archaeology gap by recovering all five first-parent
  commits, including commits that did not touch `CHANGELOG.md`.

### Deprecated

- The tracked v2 monolithic KAG representation is no longer the authored
  storage form. Its deterministic v2 logical view remains a compatibility
  route; downstream consumer migration is not claimed.
- Direct predecessor checkout consumption is no longer the active ToS route.
  The `aoa-routing` namespace remains compatibility support until its separate
  consumer-zero and retirement gates are complete.

### Removed

- Removed the tracked v2 monolith snapshot implementation in favor of the
  portable content-addressed shard family and bounded compatibility assembly.
- Removed the active direct predecessor checkout dependency from the ToS
  routing consumer. No predecessor repository was archived or deleted.

### Security

- Private source payload bytes remain outside Git; tracked records carry
  metadata, fixity, provenance, rights, visibility, and fail-closed
  publication posture.
- GitHub checkout/setup and the accepted `aoa-kag` owner-family action use
  immutable revisions; `aoa-stats` is fetched at revision
  `9ae714d21f9ea13e7b67404e769ad42d31d36c8d`.
- Generated readmodels are subordinate to ToS-authored meaning and require
  ABI/trust-root matching, durable evidence, subject-store verification, and
  fail-closed consumer selection before use.
- No public C2PA, SLSA, in-toto, Sigstore, or Cosign attestation is claimed
  for this source release. Those controls apply when a generated PDF, media,
  or export bundle becomes a separately published artifact.
- The Repo Validation workflow still clones `abyss-machine` from mutable
  `main`; this release does not claim an immutable verifier-input pin.

### First-Parent Reconciliation (5/5)

1. `8126cc3` — Adopt portable KAG index family (#138): admitted the
   content-addressed shard family, compatibility assembly, family digests, and
   generated-size receipts. KAG generated churn is grouped with this capability
   rather than counted as duplicate product work.
2. `54be6f2` — Build the Tree of Sophia corpus foundation (#139): added the
   source-home identity ladder, witness/provenance/rights contracts, source
   inventories, bounded research routes, and their generated companions.
3. `d95a583` — Retarget ToS routing consumer to `aoa-sdk` (#140): changed the
   active provider route while retaining `aoa-routing` as compatibility support;
   no sibling release or retirement is implied.
4. `77f0cef` — Pin accepted `aoa-kag` owner-family DAG (#141): bound the
   consumer to the accepted provider-family action and refreshed the relevant
   generated validation surface.
5. `afc87a3` — Land source-first corpus foundation completion (#142): completed
   the source, schema, research, validator, generated-index, and evidence
   surfaces that make the preceding foundation reviewable. Large generated
   KAG/readmodel churn is an explicit derived companion, not a separate
   semantic source.

No first-parent commit in the exact v0.4.0 range is an internal/noise-only,
duplicate, or intentionally excluded product commit. Release-only markers and
this reconciliation are metadata for the release and are not counted as a
sixth product capability.

### Validation

- The repository release gate and the generated readmodel artifact validator
  are required before publication; federation preflight and postpublish audit
  are separate release-owner checks.
- The consumer dependency is `aoa-kag@v0.5.0`; this section does not substitute
  a moving sibling branch or an unpublished candidate.
- CI, artifact receipts, runtime receipts, and publication delivery each prove
  only their declared layer. No human semantic, canon, rights, runtime health,
  deployment, proof/eval, or owner acceptance claim is made here.

### Notes

- Existing detailed human-first entries below remain part of this release;
  they retain their source, review, rights, and non-promotion limits.
- New schema families are additive and versioned. Consumers must reject
  unsupported schema/ABI versions rather than infer meaning.
- KAG consumers should pin the portable family manifest/digest and use the
  deterministic v2 compatibility route during migration.
- There is no package-level upgrade/downgrade matrix in Tree-of-Sophia, and
  this release does not claim one.
- ToS/canon remains unchanged by the completion range; generated exports and
  KAG indexes remain downstream readmodels, not source authority.

### Superseded release-body lines retained and mapped

The tag-scoped snapshots are cumulative and preserve the release-body lines that already occur in the canonical changelog. The source-only lines below are retained verbatim in fenced evidence blocks; structural/date lines are explicitly mapped to the single canonical delivery markers or to the merged snapshots above.

#### `v0.5.0` body-only source lines

~~~text
Released: 2026-08-22
Canonical changelog: [CHANGELOG.md](https://github.com/8Dionysus/Tree-of-Sophia/blob/main/CHANGELOG.md)
## Highlights
- Tree-of-Sophia grows from the v0.4.0 root-entry/statistics contour into a source-first corpus foundation with stable witness identity, provenance, rights, review, and bounded derivation routes.
- The repository adopts a portable content-addressed KAG index family with deterministic compatibility views and explicit generated-size/family receipts.
- The active ToS routing consumer now targets the `aoa-sdk` control plane while preserving the `aoa-routing` compatibility namespace.
- The exact v0.4.0-to-current-main range contains five first-parent commits; each is reconciled below rather than inferred from the former `[Unreleased]` prose alone.
## Full Release Notes
~~~

#### `v0.5.1` body-only source lines

~~~text
Released: 2026-08-23
Canonical changelog: [CHANGELOG.md](https://github.com/8Dionysus/Tree-of-Sophia/blob/main/CHANGELOG.md)
## Highlights
- Corrected the direct `aoa-stats` consumer pin from an ancestor revision to the exact published `aoa-stats@v0.2.0` commit `dc608fd5de3fcaf0301f356c9efd52e2bdd350ce`.
- Made the direct `aoa-stats` and `aoa-kag` provider edges explicit in the release contract, including the distinction between the published KAG provider commit and the immutable KAG workflow action commit.
- Corrected the release evidence identity for the KAG action and preserved the production artifact disposition as `manual_review_required`.
## Full Release Notes
~~~

#### `v0.5.2` body-only source lines

~~~text
Released: 2026-08-23
Canonical changelog: [CHANGELOG.md](https://github.com/8Dionysus/Tree-of-Sophia/blob/main/CHANGELOG.md)
## Highlights
- Revalidated the direct `aoa-stats` consumer edge against the exact published `aoa-stats@v0.2.1` commit `339ecb2db22ac4552fa88756b650896ebbff5b56`.
- Published the compatible successor needed to make the current provider identity truthful without rewriting the immutable `v0.5.1` release.
## Full Release Notes
~~~

#### `v0.5.3` body-only source lines

~~~text
Released: 2026-08-23
Canonical changelog: [CHANGELOG.md](https://github.com/8Dionysus/Tree-of-Sophia/blob/main/CHANGELOG.md)
## Highlights
- Revalidated the Tree consumer against the exact published `aoa-kag@v0.5.2` commit `8136d3eb629da28cea1206d13a8f1df52ee14739` after the exact published `aoa-stats@v0.2.2` commit `f119805cda69b3edeb2a4c5e407368d70e68650d`.
- Published the required immutable successor for the corrected provider and generated-family identities without rewriting the immutable `v0.5.2` release.
## Full Release Notes
~~~

### Historical source-heading mapping

- `## [0.5.3] - 2026-08-23` from the captured `0.5.3` source maps to `## [0.5.0] - 2026-08-23` plus `#### Historical snapshot: `## [0.5.3] - 2026-08-23``; it is retained as historical version metadata, not an active release surface.
- `## [0.5.2] - 2026-08-23` from the captured `0.5.2` source maps to `## [0.5.0] - 2026-08-23` plus `#### Historical snapshot: `## [0.5.2] - 2026-08-23``; it is retained as historical version metadata, not an active release surface.
- `## [0.5.1] - 2026-08-23` from the captured `0.5.1` source maps to `## [0.5.0] - 2026-08-23` plus `#### Historical snapshot: `## [0.5.1] - 2026-08-23``; it is retained as historical version metadata, not an active release surface.
- `## [0.5.0] - 2026-08-22` from the captured `0.5.0` source maps to `## [0.5.0] - 2026-08-23` plus `#### Historical snapshot: `## [0.5.0] - 2026-08-22``; it is retained as historical version metadata, not an active release surface.

### First-Parent Reconciliation (11/11)

Every first-parent commit from peeled `v0.4.0` through the observed pre-consolidation `origin/main` is listed below. A classification records whether its material is human-facing capability, owner/provider contract, generated/internal churn, or release-only metadata; no commit is silently omitted.

1. `8126cc37` (`8126cc379a759990358728bbcedadbd3b24a7b6d`) — PR #138; Adopt portable KAG index family (#138) — **capability: portable content-addressed KAG index family**.
2. `54be6f25` (`54be6f25a2272d02f119a16b1c75e8756decf900`) — PR #139; Build the Tree of Sophia corpus foundation (#139) — **capability: source-first corpus foundation**.
3. `d95a5832` (`d95a583272acb84dcb5b7e6896801f0364c7b234`) — PR #140; Retarget ToS routing consumer to aoa-sdk (#140) — **owner/routing: aoa-sdk consumer boundary**.
4. `77f0cef4` (`77f0cef4dfc64183ab6bdc76121c5242ca1472f0`) — PR #141; Pin accepted aoa-kag owner-family DAG (#141) — **provider contract: accepted aoa-kag owner-family DAG pin**.
5. `afc87a39` (`afc87a39cd2398738c56f72dcb21509661dd2832`) — PR #142; Land source-first corpus foundation completion — **capability: source/evidence and generated-family completion**.
6. `5c96b0a4` (`5c96b0a4a65d739c81614893d5c83129049d9285`) — PR #143; chore(release): prepare Tree-of-Sophia v0.5.0 (#143) — **release-only/merged: prior v0.5.0 preparation and landing metadata**.
7. `6d000a8d` (`6d000a8de8f3873033c7e711c509cbfc77381e91`) — PR #144; fix(release): pin exact federated providers for v0.5.1 — **provider-currentness: historical v0.5.1 exact provider correction**.
8. `6737f2cd` (`6737f2cd80ef12bc409ceb276099bac15d8e3a50`) — PR #145; fix(release): revalidate exact aoa-stats v0.2.1 for v0.5.2 — **provider-currentness: historical v0.5.2 aoa-stats correction**.
9. `0c6284f7` (`0c6284f7890c27932d8d8b406f0b1d1d0a5b13e7`) — PR #146; fix(release): preserve immutable v0.5.1 budget receipt — **generated/internal: historical immutable budget-receipt preservation**.
10. `547ad88d` (`547ad88d770e4967fa2ff0bd6c391d0154a7726e`) — PR #148; fix(release): revalidate exact-current KAG provider for v0.5.3 — **provider-currentness/generated: historical v0.5.3 KAG correction**.
11. `6999d3ee` (`6999d3ee840fd92fa97b0e9c6ca9a4152691d9cd`) — PR #147; docs: sanitize and compress public README (#147) — **owner/public route: README sanitization and compression**.

### Material coverage and claim ledger

- Capability: corpus foundation, portable KAG family, public entry, graph-review, multilingual planting, and source-returning projections remain represented in the merged snapshots.
- Schema/ABI: versioned node, relation, evidence, provider, generated-family, compatibility, and consumer-boundary statements remain represented; unsupported versions must be rejected rather than inferred.
- Owner/routing: ToS authored meaning, `aoa-sdk` consumer routing, `aoa-kag` provider edges, and `aoa-stats` validation ownership remain distinct.
- Security/privacy: provenance, rights, trust-root, attestation, local-vs-public evidence, privacy, and no-secret/non-claim limits remain represented.
- Operator/generated/validation: generated KAG/index receipts, owner builders, release checks, CI lanes, strict preflight/postpublish, recovery/rollback, and clean-source requirements remain represented.
- Limitation/non-claim: artifact admission, deployment, runtime health, semantic proof, transport delivery, closure, downstream acceptance, owner acceptance, and human acceptance are not inferred.
- Historical report inventory and raw release/tag snapshots remain preserved outside this authored source in the immutable pretruth corpus and task-local ledger; the report inventory is not promoted to source authority.

### Campaign evidence references

- Historical report retained: `/srv/abyss-machine/tmp/ai/release-campaign-goal-01a008db-20260821/reports/closure-repair-tree-stats-v021-release-exec-report.md` (`sha256:70311db733aadde3124a0f7471b8973c707a43e374f46e869319e3791e37646d`, 12809 bytes); its declared source/CI/artifact/runtime/proof/delivery/closure/acceptance boundaries remain evidence-only.
- Historical report retained: `/srv/abyss-machine/tmp/ai/release-campaign-goal-01a008db-20260821/reports/postaudit-repair-tree-v0-5-1-release-exec-report.md` (`sha256:73abbc54bba476717d8641aa8e477d032c012fb405c590b6a5e12035613abb81`, 9059 bytes); its declared source/CI/artifact/runtime/proof/delivery/closure/acceptance boundaries remain evidence-only.
- Historical report retained: `/srv/abyss-machine/tmp/ai/release-campaign-goal-01a008db-20260821/reports/provider-consumer-repair-tree-after-kag-release-exec-report.md` (`sha256:54c66404aaafd27927c6c37503190142ee4763b978d0d9db2fcbfc987fc2463d`, 12036 bytes); its declared source/CI/artifact/runtime/proof/delivery/closure/acceptance boundaries remain evidence-only.
- Historical report retained: `/srv/abyss-machine/tmp/ai/release-campaign-goal-01a008db-20260821/reports/terminal-repair-tree-artifact-registry-release-exec-report.md` (`sha256:9a42b2b6c96349f9b3ebf855eb63effb9b067e145e87359f8eaeed349b0cd571`, 14531 bytes); its declared source/CI/artifact/runtime/proof/delivery/closure/acceptance boundaries remain evidence-only.
- Historical report retained: `/srv/abyss-machine/tmp/ai/release-campaign-goal-01a008db-20260821/reports/tree-of-sophia-release-exec-report.md` (`sha256:cbd629e662f24680bd746d0edee3beaf93bfad0d80270490f36a3680c9464008`, 16966 bytes); its declared source/CI/artifact/runtime/proof/delivery/closure/acceptance boundaries remain evidence-only.
- Historical report retained: `/srv/abyss-machine/tmp/ai/release-campaign-goal-01a008db-20260821/reports/tree-of-sophia-release-recon-report.md` (`sha256:aa624112739a83cd927c3a47c623df927b3a09aaecbabbc5111226fbedd968d2`, 48184 bytes); its declared source/CI/artifact/runtime/proof/delivery/closure/acceptance boundaries remain evidence-only.

## [0.4.0] - 2026-07-14

### Summary

- This release adds a bounded owner-local statistics port for the Table I
  prepared-dossier route while keeping shared statistical grammar and
  cross-owner composition in `aoa-stats`.
- Executable validation authority is now concentrated in the validation-lane
  manifest, local route cards, and owner scripts instead of being repeated
  through weaker explanatory Markdown.
- The release was reconstructed from the complete post-`v0.3.0` Git range:
  its single first-parent commit changed 57 paths with 1,286 additions and 628
  deletions, and that 1/1 commit is accounted for below rather than inferred
  only from the old `[Unreleased]` prose.

### Added

- Added a root `stats/` port for the reference-only Table I prepared-dossier
  route ratio, preserving the complete row population, numerator and
  denominator, owner evidence, missingness, and authority ceiling.
- Added the owner-delegating stats validator, regression coverage, CODEOWNERS
  route, and CI checkout of the pinned `aoa-stats` contract owner.

### Changed

- Centralized the local stats check in the executable validation-lane manifest
  and removed shell command blocks from ordinary Markdown documentation.
- Refreshed root, doctrine, decision, mechanics, and validation routes so
  runnable commands return to their executable owners or nearest route cards.
- Regenerated the Table I corpus companion and all seven repository-local KAG
  indexes after the authored and validation surfaces changed.

### First-Parent Reconciliation (1/1)

1. `0407ad4` — Add Tree of Sophia local stats port (#136).

### Validation

- Release preparation reconciled the exact `v0.3.0..0407ad4` first-parent
  history and all 57 changed paths, including the stats contract, validation
  authority cleanup, doctrine/decision routes, generated ToS companion, and
  complete KAG index family.
- The repository release gate validates source and mechanics topology,
  validation-lane authority, generated parity, graph/canon/intake contracts,
  the pinned central stats contract, and the full test suite.

### Notes

- The ratio is a reference measurement over an explicitly bounded local
  population; it is not a quality verdict or a replacement for ToS review and
  canon authority.
- `aoa-stats` retains shared grammar and cross-owner composition, while
  runtime, proof, memory, and graph-substrate authority remain with their
  stronger owners.
- Release-only version markers, changelog reconciliation, and regenerated
  companions follow the one reconciled product commit and are described here
  rather than misclassified as another product change.

## [0.3.0] - 2026-07-13

### Summary

- This release grows `Tree-of-Sophia` from one hardened golden route into a
  source-home-backed world-philosophy tree: the philosophy atlas, Table I
  planting, graph workbench, review packets, multilingual contracts, and
  source-returning projections now form one checked pre-canon contour.
- Repository operations now have explicit source/mechanics topology,
  decision rationale, validation-lane authority, local memo/eval/KAG ports,
  portable support skills, and OS Abyss artifact trust without transferring
  ToS meaning to runtime, proof, memory, or graph-substrate owners.
- The release was reconstructed from Git and owner surfaces rather than from
  the old `[Unreleased]` prose: all 70 first-parent commits from `v0.2.2`
  through `359c817` are accounted for below, spanning 1,154 changed paths and
  67,171 additions / 3,666 deletions. Only 7 of those 70 commits had touched
  this changelog before release preparation.

### Added

- A canonical `ToS/` source home and philosophy tree with branch law, atlas
  tables, era/region/tradition planting, prepared dossiers, source-anchor
  backlogs, and graph-workbench surfaces that remain visibly pre-canon.
- Whole-corpus, philosophy-atlas, graph-view, graph-projection, and
  post-planting-audit builders, validators, schemas, tests, and derived
  exports for review and downstream visualization handoff.
- Semantic graph layers, review packets, clustering/view contracts, a
  planting interface, Table I prepared branches, and runtime-readiness
  packets that preserve source and promotion boundaries.
- Multilingual atlas labels, language registry, text-bearing-node contract,
  and generated language packets for reviewable language-aware planting.
- A durable `TOS-D-####` decision-rationale lane with generated indexes and
  fail-closed modeled-surface validation.
- Explicit source-home and mechanics topology, operational nested route cards,
  command-authoritative validation lanes, script/test inventories, and a
  local eval port.
- Local memo and KAG provider homes, including the canonical seven-index
  repository family and source-return routes for downstream consumers.
- A generated-readmodel artifact identity, OS Abyss bundle, evidence roots,
  subject-store handoff, and consumer trust gate.
- A checked portable `.agents/skills/` support foundation and session-growth
  surfaces without making copied skills a ToS authority surface.

### Changed

- Root and source-home guidance now routes contributors to the owning
  philosophical branch, mechanic, decision, validator, or generated companion
  instead of duplicating broad inventories in entrypoint docs.
- Generated readmodels and graph packets now preserve source refs, semantic
  layer meaning, multilingual labels, review posture, and deterministic
  builder parity across the full philosophy contour.
- Repo-local KAG publication now uses pinned deterministic full and
  incremental parity checks and exposes entity, artifact, anchor, event,
  assertion, and relation indexes alongside the source-surface index.
- Canon, intake, Experience, Questbook, public-entry, graph-export, artifact,
  decision, and route-card checks now execute from explicit owner lanes.

- compacted root `README.md`, `AGENTS.md`, `DESIGN.md`, `DESIGN.AGENTS.md`,
  `BOUNDARIES.md`, and `ROADMAP.md` into route-oriented owner surfaces while
  keeping the current ToS root-entry and export contracts visible
- relaxed root-doc validation pressure so README and ROADMAP route to owner
  surfaces instead of carrying command inventories or script-path lists
- added explicit validation-lane command authority, script and test inventories,
  and release-check delegation through `docs/validation/validation_lanes.json`
- split Questbook obligation and dispatch validation into its own
  `questbook_surface` lane while keeping KAG export validation focused on the
  bounded export seam
- moved canon and intake checks out of `validate_kag_export.py` and into the
  explicit release lane command sequence
- renamed migration-era Experience boundary batch tests by the mechanic
  contracts they protect
- moved Experience boundary contract tests into an explicit mechanics-local
  `experience_contracts` validation lane
- reduced route-card and tiny-entry validators toward structural route tokens
  instead of ordinary prose inventories
- moved the Codex Spark fast-loop lane from root `Spark/` to
  `.agents/spark/`, keeping Spark agent-facing rather than a public root organ
- reshaped root and package roadmap surfaces around AoA-style direction,
  horizon posture, future triggers, and package-local growth pressure without
  turning roadmaps into validation inventories or landing logs

### Fixed

- Restored strict ToS node-contract enforcement and source-side smoke canon
  routes, including valid cron syntax.
- Rejected path traversal through research packets, child-page label leakage,
  unmodeled or malformed decision surfaces, and retired active naming.
- Hardened dry-run helper shape handling, source-home scan scope, current
  direction locks, semantic graph layers, generated projections, artifact
  admission, and local KAG parity.

### First-Parent Reconciliation (70/70)

The ordered pre-release history is recorded explicitly so the 63 commits that
were absent from the old changelog remain discoverable:

1. `2e2c9e5` — Slim root AGENTS route card (#65).
2. `adb90c0` — chore: retarget shared skill links to AbyssOS.
3. `549ddae` — Install portable AoA skill foundation.
4. `9aecd91` — Roll out session-growth skills and GitHub landing (#68).
5. `5752259` — [codex] harden portable skills and traceability (#69).
6. `7cae0d1` — Refresh session growth refs and readiness guard (#70).
7. `a240784` — Guard dry run preview step shape (#71).
8. `b934ed0` — Preserve dry run helper malformed shapes (#72).
9. `ac83750` — Refresh shared AoA skill pack (#73).
10. `7d0402a` — Refresh shared AoA skill pack (#74).
11. `cd77aa4` — Close Tree-of-Sophia audit feedback (#75).
12. `48cf342` — Restore strict ToS node contract (#76).
13. `7a28de8` — Refresh aoa-summon skill export (#77).
14. `0278961` — Refresh automation opportunity skill contracts (#78).
15. `4f297b3` — Add memory route trigger law (#79).
16. `6bba937` — Use canonical aoa-memo route label (#80).
17. `40a44a4` — Add local memo port (#81).
18. `d59766f` — Make memo validation route portable (#82).
19. `5db2e28` — Add ToS decision rationale lane (#83).
20. `aecfdbe` — Detect unmodeled decision lane surfaces (#84).
21. `dcb43aa` — Honor modeled decision lane surfaces (#85).
22. `130c436` — Normalize modeled decision surface paths (#86).
23. `ee4abe8` — Require modeled surfaces to be explicit lists (#87).
24. `b41f783` — Add ToS source home and philosophy tree (#88).
25. `4163f0a` — Use operational route cards for ToS instructions (#89).
26. `158a96d` — Refactor ToS mechanics topology (#90).
27. `924542a` — Correct ToS authority and mechanics routing (#91).
28. `a2ecee5` — Add ToS corpus index resource (#92).
29. `d7603cc` — Unload ToS root validation routes.
30. `3b65872` — Add local eval port skeleton.
31. `8b53f39` — Tighten active naming path references (#95).
32. `e92a7e2` — Reject traversing research packet routes (#96).
33. `3844fd7` — Guard philosophy paths from child page labels.
34. `e073eb8` — Add source home schema and root-scoped philosophy scan.
35. `37743a1` — Add validation lane command authority (#99).
36. `9bc702e` — Separate KAG export validation from release lanes (#100).
37. `c54a20c` — Restore source-side smoke canon validators (#101).
38. `efdb645` — Add node contract smoke validator (#102).
39. `dfc1ae5` — Guard child page title metadata labels (#103).
40. `cc0e325` — Add Experience contract validation lane (#104).
41. `32c6f89` — Refactor ToS source home mechanics topology.
42. `349b566` — Add OS Abyss readmodel artifact gate (#106).
43. `9d71bc3` — Refactor ToS topology and Spark lane.
44. `25bd6ff` — Add ToS graph-readiness skeleton (#108).
45. `0ae78da` — Strengthen generated readmodel artifact trust profile (#109).
46. `382abaf` — Add ToS philosophy atlas (#110).
47. `5811bd9` — Add ToS philosophy atlas projection.
48. `bb876c5` — Promote ToS readmodel evidence with trust roots (#112).
49. `57d28d7` — Add ToS philosophy graph views.
50. `44bff7d` — Add philosophy graph projection export (#114).
51. `0496f56` — Add philosophy graph review contracts (#115).
52. `2a79e5b` — Plant Table I prepared philosophy tree (#116).
53. `3a7abdd` — Fix source-side smoke validator routes (#117).
54. `6ec0545` — Fix source-side smoke cron syntax (#118).
55. `7267bfd` — Prepare philosophy graph review runtime readiness (#119).
56. `c85b51d` — Make philosophy graph layers semantic (#120).
57. `038cead` — Align ToS roadmap surfaces.
58. `c6151cc` — Add local KAG provider home (#122).
59. `84372b1` — Align KAG provider validation route (#123).
60. `afd4e1e` — Define philosophy graph planting interface (#124).
61. `592f4b8` — Add repo-local KAG indexes (#125).
62. `1b301cd` — Add ToS multilingual philosophy labels.
63. `33d843e` — Add ToS multilingual planting contract (#127).
64. `9763449` — Emit ToS text-bearing language packets (#128).
65. `56a7109` — Strengthen current direction route lock (#129).
66. `7e57c27` — Enforce repo-local KAG index parity (#130).
67. `e868585` — Pin deterministic repo-local KAG index gate (#131).
68. `c3633f4` — Add repository KAG index family (#132).
69. `0b8312b` — Publish canonical repository KAG indexes (#133).
70. `359c817` — Harden philosophy projections and local KAG parity (#134).

### Validation

- Release preparation reconciled the exact `v0.2.2..359c817` first-parent
  history, changed-path inventory, source and mechanics topology, decisions,
  atlas and graph sources, generated companions, local ports, artifact trust,
  and KAG provider family rather than trusting `[Unreleased]` alone.
- The release gate checks explicit validation authority, source/philosophy and
  mechanics topology, mechanics-local and Experience contracts, active naming,
  all generated graph/readmodel parity, OS Abyss artifact admission, canon and
  intake contracts, public entry/export, local KAG and Questbook surfaces,
  nested route cards, decisions, and the full repo-local test suite.

### Notes

- Atlas rows, planted Table I dossiers, graph-workbench nodes/relations, review
  packets, multilingual packets, and derived projections remain source-linked
  pre-canon or readmodel material until their owning review/canon routes
  promote them.
- Runtime visualization, proof verdicts, memory truth, shared KAG substrate,
  and skill source authority stay with their stronger owners.
- Release-only marker, guide, changelog, and regenerated-index commits follow
  the 70 reconciled product and maintenance commits and are not hidden inside
  that count.

## [0.2.2] - 2026-04-23

### Summary

- this patch lands Agon Sophian threshold intake posture, ToS candidate
  dossier review, canon-restraint checks, rejection/branching posture, and
  follow-up guards while keeping ToS on source-owned interpretation and intake
  boundaries
- Experience wave3-wave5 boundary contracts are planted for adoption,
  governance, installation, service dossiers, runtime stop-lines, and
  sovereign office intake without allowing direct arena, runtime, or assistant
  office writes into the tree
- `Tree-of-Sophia` remains the source-first knowledge authority and does not
  turn installation, governance, runtime, or assistant-office artifacts into
  direct authored tree writes

### Added

- Agon Sophian threshold intake registry surfaces, ToS candidate dossier
  review notes, pattern review notes, canon-restraint guidance, and
  rejection/branching boundaries
- wave3 Experience adoption and no-runtime-adoption boundary dossiers, wave4
  governance and constitution-runtime no-direct-write guards, and wave5
  installation, service, and sovereign-office intake boundaries
- governance, installation, service, and sovereign-office dossier contracts
  with explicit decision-reference requirements

### Changed

- schema and contract guards now enforce stronger no-direct-write,
  no-runtime-adoption, no-runtime-office-write, and governance decision-ref
  stop-lines
- post-merge review guards are tightened around Experience boundary surfaces
  so ToS can receive dossiers without becoming a runtime adoption mechanism

### Validation

- `python scripts/release_check.py`

### Notes

- this patch is a source-boundary release for ToS; runtime, release
  certification, and office-operation authority remain in their owning AoA
  repositories

## [0.2.1] - 2026-04-19

### Summary

- this patch tightens release posture, roadmap routing, and current-direction
  locks around the active root-entry line
- README front-page, release-audit baseline, PR template coverage, and
  required-check plus Node24 CI surfaces now agree
- `Tree-of-Sophia` remains the source-first authority while keeping
  release-facing entry surfaces bounded and legible

### Added

- a release-audit baseline, current-direction route locks, and a GitHub pull
  request template for the repository

### Changed

- roadmap entry-routing docs, README release front page, changelog contract,
  and CI/protection refs are aligned with the current root-entry contour

### Validation

- `python scripts/release_check.py`

### Notes

- this patch is release-contract and route-legibility work for the source
  repository; it does not widen Tree-of-Sophia ownership boundaries

## [0.2.0] - 2026-04-10

### Summary

- this release adds a source-owned zero-entry root capsule, reentry doctrine, stronger routed-language validation, and repo-local follow-through skill surfaces
- witness handling, schema-level uniqueness, and source-first review guidance are tightened across the current trilingual tree contour
- `Tree-of-Sophia` remains source-first authority while exposing stronger downstream-safe entry and export seams

### Validation

- `python scripts/release_check.py`

### Notes

- detailed route, corpus, and generated-surface coverage for this release remains enumerated below under `Added`, `Changed`, and `Included in this release`

### Added

- source-owned tiny-entry route validation and reentry doctrine together with a
  zero-entry `ToS/derived-exports/root_entry_map.min.json` capsule
- stronger routed-language witness handling through generalized language
  validation and restored schema-level uniqueness
- repo-local project-foundation, session-harvest, and automation-opportunity
  skill surfaces for source-first review and closeout follow-through

### Changed

- tightened AGENTS and review guidance around source-first boundaries,
  validator scope, and the current knowledge-first route posture

### Included in this release

- root-route and contributor-posture refreshes across `README.md`,
  `BOUNDARIES.md`, `ROADMAP.md`, `AGENTS.md`, `.agents/`, `.github/`, and
  `docs/`, including public-route clarity, review guidance, and follow-through
  installs
- source, intake, and tree route artifacts across `ToS/source-witnesses/`, `ToS/candidate-intake/`,
  `ToS/canon/`, `ToS/public-compatibility/`, `ToS/derived-exports/`, `ToS/contracts/`, `scripts/`, `tests/`, and
  `Spark/`, including routed-language witness handling and the zero-entry root
  capsule

## [0.1.0] - 2026-04-01

First public baseline release of `Tree-of-Sophia` as the source-first knowledge architecture repository in the AoA / ToS ecosystem.

This changelog entry uses the release-prep merge date.

### Summary

- first public baseline release of `Tree-of-Sophia` as the canonical home of ToS knowledge architecture and authored tree posture
- the public baseline now includes source, intake, tree, example, and generated export layers around one bounded trilingual Zarathustra entry route
- the release keeps ToS authority source-first while exposing one downstream-safe KAG export and one source-owned tiny-entry seam

### Added

- community-docs baseline established for this repository
- `CHANGELOG.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, and `CONTRIBUTING.md`
- `ToS/doctrine/TINY_ENTRY_ROUTE.md` as the first public doctrine note for the tree-first tiny-entry seam
- `ToS/contracts/tos-tiny-entry-route.schema.json` and `ToS/public-compatibility/tos_tiny_entry_route.example.json`
- `ToS/review-ledger/2026-03-23-tree-first-tiny-entry-review.md` for the manual review result of this phase
- bounded witness-provenance fields and optional segment locators in the public multilingual source-node contract
- `ToS/review-ledger/2026-03-25-zarathustra-witness-provenance-and-tiny-hop-review.md`
- `ToS/review-ledger/2026-03-25-zarathustra-pass-001-stabilization-review.md`

### Changed

- `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now also serves as the first worked tiny-entry capsule
- `README.md`, `ROADMAP.md`, and `ToS/doctrine/KNOWLEDGE_MODEL.md` now expose the tree-first tiny-entry seam in the public entry path
- `ToS/doctrine/REVIEW_CHECKLIST.md` now includes tiny-entry-specific boundary checks
- tiny-entry doctrine now records the current downstream consumption posture in `aoa-kag` and `aoa-routing` without changing ToS authority boundaries
- `ToS/doctrine/NODE_CONTRACT.md`, `ToS/doctrine/SOURCE_NODE_TEMPLATE.md`, `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, `ToS/contracts/tos-node-contract.schema.json`, and `ToS/public-compatibility/source_node.example.json` now keep multilingual witness provenance explicit for the bounded Zarathustra route
- `ToS/doctrine/TINY_ENTRY_ROUTE.md`, `ToS/contracts/tos-tiny-entry-route.schema.json`, and `ToS/public-compatibility/tos_tiny_entry_route.example.json` now prefer `bounded_hop` while keeping `lineage_or_context_hop` as a bounded compatibility alias during transition
- `ToS/public-compatibility/source_node.example.json` now makes Russian and English maintainer-curated witness posture more explicit and adds a second opening-movement translation-tension note
- `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now records pass-based stabilization posture and the non-canonical boundary for restartable checkpoint sidecars

### Included in this release

- the current source-first repository layers under `ToS/source-witnesses/`, `ToS/candidate-intake/`, `ToS/canon/`, `ToS/public-compatibility/`, and `ToS/derived-exports/`
- the bounded Zarathustra route carried by `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, `ToS/doctrine/TINY_ENTRY_ROUTE.md`, `ToS/public-compatibility/source_node.example.json`, and `ToS/derived-exports/kag_export.min.json`
- the current canonical tree surface validated as `92` node payloads plus the route-local canonical relation pack

### Validation

- `python scripts/validate_tree_example_sync.py`
- `python scripts/validate_tree_node_contracts.py`
- `python scripts/validate_tree_relation_pack.py`
- `python scripts/validate_intake_pack.py`
- `python scripts/generate_kag_export.py`
- `python scripts/validate_kag_export.py`

### Notes

- this release exposes a bounded public entry path and export seam without weakening ToS source authority
