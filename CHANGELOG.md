# Changelog

All notable changes to `Tree-of-Sophia` will be documented in this file.

The format is intentionally simple and human-first.
Tracking starts with the community-docs baseline for this repository.

## [Unreleased]

- Added Spanish, German and Simplified Chinese presentation for both public
  Bonsai stands, their research materials and shared reader. Language changes
  retain exact source locations and personal graph identities; the reader can
  change its interface language without changing its edition or passage.
- Simplified contributor documentation around current owner routes, removed
  stale implementation-status prose and duplicate explanations, and linked
  cumulative v0.5 campaign snapshots to their immutable Git history.

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

Consolidated the source-first corpus foundation, philosophy atlas, graph-review
preparation, multilingual planting and bounded public-entry routes into one
source release. The portable content-addressed KAG family and local statistics
remained derived from their declared sources. Corpus identity, provenance,
rights and review contracts did not promote all prepared material to canon.

### Historical release consolidation

The original release reconciled four same-day campaign snapshots (`v0.5.0`
through `v0.5.3`) under this one dated release. Their cumulative bodies, provider
corrections, validation statements, limitations, exact hashes and evidence
ledger are preserved in the
[immutable pre-cleanup changelog](https://github.com/8Dionysus/Tree-of-Sophia/blob/903e80ee515067a8f81f40cd1e6dd1df474cb113/CHANGELOG.md#050---2026-08-23).
They describe that historical release, not current installation requirements.
The same complete file is available offline from Git:

```sh
git show 903e80ee515067a8f81f40cd1e6dd1df474cb113:CHANGELOG.md
```

At consolidation, the direct providers were `aoa-stats@v0.2.0`
(`88ff38b1b38eef939f2c5b4541cbe8363a05fc8d`) and `aoa-kag@v0.5.0`
(`f46f146cc79a26fa81ad0f400b9c5774df293e57`).
[TOS-D-0062](docs/decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md)
and [TOS-D-0072](docs/decisions/TOS-D-0072-repository-independent-execution-and-exports.md)
later separated software, corpus and integration and removed those ecosystem
execution prerequisites. Historical provider identities remain provenance.

### Included changes

| Commit / PR | Contribution |
| --- | --- |
| `8126cc37` / #138 | Portable content-addressed KAG family |
| `54be6f25` / #139 | Source-first corpus foundation |
| `d95a5832` / #140 | Historical aoa-sdk consumer routing |
| `77f0cef4` / #141 | Accepted KAG provider-family pin |
| `afc87a39` / #142 | Source, schema, research and evidence completion |
| `5c96b0a4` / #143 | Original v0.5.0 release preparation |
| `6d000a8d` / #144 | Historical v0.5.1 provider correction |
| `6737f2cd` / #145 | Historical v0.5.2 provider revalidation |
| `0c6284f7` / #146 | Immutable budget-receipt preservation |
| `547ad88d` / #148 | Historical v0.5.3 provider revalidation |
| `6999d3ee` / #147 | Public README sanitization and compression |

### Evidence scope

The archived reconciliation records the exact first-parent range and the
original source, CI and publication claims. Those records do not establish
artifact admission, deployment, runtime health, semantic proof or human
acceptance. Current release commands belong to [RELEASING](docs/RELEASING.md).

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
