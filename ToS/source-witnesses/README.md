# Source Witness Corpus

`ToS/source-witnesses/` is the physical and authored evidence root for works,
expressions, editions, collections, acquired items, source-near text, and
aligned witness packets.

The exact division between project evidence and host-managed caches/runtimes is
recorded in [`LOCAL_STORAGE_BOUNDARY.md`](LOCAL_STORAGE_BOUNDARY.md).

Semantic canon lives in `ToS/canon/`; laboratory runtimes belong to their host
owner; catalogs and graphs are derived from these source records.
Source-visible review evaluates meaning and quality, while the rights and
publication owners decide permitted use. Each section below records its actual
evidence and source-specific limits under that common law.

The standalone native source-catalog route is `tos-native-owner-command source-catalog`. Run `check --repo-root ABS --invocation ABS` for the protected published-root foundation audit and catalog parity check. Run `build --repo-root ABS --invocation ABS --max-candidate-bytes N --max-stage-read-bytes N --max-state-bytes N` only with an owned-cold foundation invocation; it emits a bounded `TOS_SOURCE_CATALOG_CANDIDATE_V1` frame stream to stdout after the final source and metadata fences pass. `max-state-bytes` must cover the selected profile's derived peak for the candidate frame, largest catalog file, bounded manifest working set, and report. Both commands require the private-tmpfs issuer ticket supplied by the launcher. The candidate is not admitted into the authored source tree by this command.

Curated authored records remain Git-backed. Bulk imports use immutable corpus
revisions in permanent local storage, with permitted private R2 backups.
Catalogs and projections are built into a separate data snapshot. The same
stable IDs, exact source bytes, provenance, rights and review posture survive
the move; authored records remain the authority behind generated navigation. Use the
[corpus and data release route](../../docs/RELEASING.md#data-and-corpus-operations)
for admission, compilation and explicit reader selection.

Historical `.record-revisions/` and `.metadata-transactions/` paths belong to
explicitly selected immutable corpus cuts.
[`record-revisions.storage.json`](record-revisions.storage.json) and
[`metadata-transactions.storage.json`](metadata-transactions.storage.json) bind
each unchanged subtree's membership, bytes, portable mode and exact Git restore
witness. The checkout retains these bindings; archived manifests, completions
and blobs keep their original logical paths in the corpus. The curated
`.metadata-publication.json` retains its exact publication reference, whose
transaction closure is available in the complete selected cut.

Prepare the current `ToS` Git capture with `source-capture capture-git`, excluding
both historical prefixes and the reserved `owner-local`, `payload` and `.git`
path parts. Then use `source-capture import-corpus` with each explicitly selected
retained store, revision and binding. The fresh store contains the complete source
cut, every exact old historical snapshot and its original receipt in one
immutable object pool. The command's `--help` owns the paired input flags and
limits. Select the complete cut for the source-to-reader operation; a curated Git
capture alone does not contain the historical package closure.

## Speaking topology

```text
source-witnesses/
├── artifacts/                              # physical witnesses outside the book ladder
│   └── <tradition>/<site>/<physical-identity>/
│       ├── artifact-witness.json           # physical/catalog/content-layer separation
│       └── rights.json                     # layer-specific public-use boundary
├── scholarly-composites/                   # modern reconstructions across witnesses
│   └── <genre-or-method>/<tradition>/<composition-identity>/
│       ├── composite-witness.json          # composition/member/representation separation
│       └── rights.json                     # project content versus external-media terms
├── agents/
│   └── <person-or-organization>/agent.json
├── places/
│   └── <place>/place.json
├── organizations/
│   └── <historical-organization>/organization.json
├── social-bodies/                          # source-described groups, communities, institutions
│   └── <kind>-<subject>/<kind>.json         # separate membership and activity Claims
├── intellectual-formations/               # source-described schools, traditions and movements
│   └── <kind>-<subject>/<kind>.json         # source-described continuity, separate association Claims
├── links/                                  # first-class observed web/API/file routes
│   └── <provider>/<object>/<route>/link.json
├── discovery/                              # reviewed candidate queue + ordered query/result evidence
├── access-requests/                        # public status + ignored private correspondence
├── server-import/                          # future no-upload/import boundary
├── chronology/                             # Work-owned, facet-specific chronology claims
│   └── <responsibility-or-tradition>/
│       └── first-publication/
│           ├── work-chronology-claims.jsonl
│           └── provenance.jsonl
├── catalog/
│   ├── agents.jsonl
│   ├── places.jsonl
│   ├── organizations.jsonl
│   ├── works.jsonl
│   ├── expressions.jsonl
│   ├── editions.jsonl
│   ├── collections.jsonl
│   ├── items.jsonl
│   ├── links.jsonl
│   ├── artifacts.jsonl                    # native physical IDs and witness metadata
│   └── claims.jsonl                       # generated source-returnable relation index
├── relations/                             # corpus-wide identity-ladder assertions
│   ├── work-expression/
│   │   └── work-expression-claims.jsonl   # Work --has_expression--> Expression
│   ├── expression-edition/
│   │   └── expression-edition-claims.jsonl # Expression --embodied_by--> Edition
│   ├── edition-item/
│   │   └── edition-item-claims.jsonl       # Edition --exemplified_by--> Item
│   ├── expression-derivation/
│   │   └── expression-derivation-claims.jsonl # Expression --is_derivative_of--> Expression
│   ├── object-link/
│   │   └── object-link-claims.jsonl       # Work/Expression/Edition/Item --> Link
│   └── provenance.jsonl                   # digest-bound batch materialization event
├── works/
│   └── <responsibility-or-tradition>/
│       └── <work>/
│           ├── work.json
│           ├── responsibility-claims.jsonl # evidence-bearing Work/child responsibility bundle
│           ├── expressions/
│           │   └── <language-and-responsibility>/
│           │       ├── expression.json
│           │       └── editions/
│           │           └── <edition>/
│           │               ├── edition.json
│           │               ├── publication-claims.jsonl # evidence-bearing, review-gated publication assertions
│           │               ├── provision-activity-claims.jsonl # grouped place/agent/date statements
│           │               ├── responsibility-claims.jsonl # optional Edition-local responsibility bundle
│           │               └── items/
│           │                   └── <item>/
│           │                       ├── item.manifest.json
│           │                       ├── resource-inventory.json # tracked pages/resources, no source text
│           │                       ├── fixity.sha256
│           │                       ├── provenance.jsonl
│           │                       ├── rights.json
│           │                       ├── forensic-report.md
│           │                       ├── structure/       # text-free item-specific maps + proposed anchors
│           │                       └── payload/        # gitignored bytes
│           ├── texts/                              # reviewed source-near text
│           ├── alignments/                         # multi-expression packets
│           └── lexical-indexes/                    # source-gated plans and text-free receipts
└── collections/
    └── <responsibility-or-tradition>/
        └── <collection>/
            ├── collection.json
            ├── editions/
            ├── membership-claims.jsonl
            ├── responsibility-claims.jsonl
            └── structure/
                └── work-boundaries/       # text-free member ranges + page anchors
```

The corpus-wide `provision-activities/` route holds digest-bound provenance
for bounded Edition-owned provision claims; a named subbranch may own one
multi-Edition research episode while every claim stays beside its exact
subject Edition. Shared discovery episodes are recorded under
`discovery/provenance.jsonl`; Claim assessment remains bound to each Claim’s source grounds.
The authored reviewed-candidate ledger owns only queue eligibility, ordering,
and the frozen pre-run target. Immutable candidate receipts own terminal loop
transitions, while `discovery/candidates/queue.current.json` is generated
navigation. Work identity, rights, acquired Items, claims, and review remain
with their stronger source-witness surfaces. The latest terminal receipt must
bind an external positive per-channel timing receipt; transport timing measures the recorded channel operations.

`<responsibility-or-tradition>` groups sources for navigation; responsibility
Claims state attributed authorship. Anonymous, disputed, collective, and tradition-owned works receive
speaking routes and explicit responsibility claims in the catalog.

## Identity and path boundary

Historical people and collective descriptions use the
[social profile and relationship contract](../doctrine/semantic-interchange/README.md#social-bodies-and-source-attributed-relationships).
Social bodies, places, intellectual formations and atlas navigation categories
each retain their own referent and source type. Adjacent native Corpus and
declared-profile human forms reach both source graph carriers with their exact
bindings and quality gaps; source-visible assessment evaluates their content.

Source-described reception, legacy, historical canon formation, forgetting and
rediscovery use the [reception profiles](../doctrine/semantic-interchange/README.md#reception-historical-recognition-and-later-life).
Their historical scope and receiving context remain content. Claims separately
identify targets, carriers, participants and dates; historical canon formation retains the receiving community’s criteria, and
forgetting requires evidence of diminished transmission in that context.

Object and claim records own stable ToS IDs. The catalog is their rebuildable
navigation projection: `claims.jsonl` makes tracked membership, responsibility,
publication, provision activity, chronology, and identity-ladder assertions
queryable while preserving exact source file, source line, canonical claim
digest, evidence, maker, provenance event, and review posture. Each catalog entry retains the original Claim’s review posture and source
authority. The tracked
projection admits only `public` or
`public_metadata_only` packets; less-visible claims require an explicitly
reviewed public-safe derivative. Filesystem paths are human navigation and may
improve through reviewed migrations. A path change never silently changes
object or claim identity.

### Native catalog and projection routes

The Rust `source-catalog` owner is the supported route for full source-witness
catalog validation. Its protected `source-catalog check` runs the complete
source-foundation audit and exact generated-catalog parity; `source-catalog
build` emits a bounded candidate and does not write authored source. The
invocation and sealed-stage requirements are documented by the native
[`source_witness_foundation` validation route](../../docs/validation/README.md).

The corpus index and bibliographic graph use the native corpus-projection
owner. Its exact request, finite budgets, tracked-product parity check, and
managed snapshot build are documented in
[`Data and corpus operations`](../../docs/RELEASING.md#data-and-corpus-operations).
The native owner composes the existing Rust catalog, Claim, navigation, and
bibliographic renderers; this README remains the source for ToS identity and
projection boundaries, not a second operational API.

The addressed-catalog schema and earlier Python APIs remain evidence of their
historical contract. They do not provide a runnable Python builder, transition
writer, Claim assembler, or source-row reader. Stable ToS IDs and authored
records remain authoritative; generated catalogs and graphs remain deletable
navigation products.

Evidence nodes carry a bounded readable `display` from owner metadata.
An identity can reuse its catalog `preferred_label`. Direct Markdown
notes in `ToS/review-ledger/` and in
`ToS/research-packets/foundation-laboratory-2026-07/` can supply their exact first
H1 when that exact reference is selected by the existing verified public Claim
catalog. The citing Claim, digest and public visibility must agree with its
catalog entry; an unselected local file is not eligible. Git tracking belongs to authoring; runtime reading uses the exported catalog
and source bytes: an exported source snapshot
with the same catalog and bytes yields the same title without `.git`.
This selected-public-Evidence route uses the protected owner metadata reader:
no symlink component or special file, at most 1 MiB, same-read file digest,
change/replacement detection, and a first-line UTF-8 header of at most 4096 bytes.
Research titles retain their research-lead provenance.
Other repository references or missing/overlong headings use the exact filename;
anchors and provenance events use their source-file slot, and external citations
use the declared address.
Fallbacks are explicitly marked as navigation. Descriptions explain the
reference role and source return; external content remains unobserved. This route
does not scan arbitrary document headings, open source payloads, fetch remote
titles, create HumanForms, translate names, or confer assessment authority.
Full addresses, source references, IDs and digests remain unchanged in their
existing fields even when the display's 240-character title or 1024-character
description is shortened.

The complete local source-witness audit and generated-catalog parity use the
native owner directly through this lane, run inside the native ToS
private-tmpfs stage issuer:

```sh
TOS_NATIVE_FOUNDATION_INVOCATION=/absolute/protected-invocation.json \
  tos-validation-lanes --repo-root /absolute/Tree-of-Sophia --run source_witness_foundation
```

The owner-issued `tos_local_native_foundation_invocation_v1` request must bind
the installed `tos-native-owner-command` and schema-worker digests and select
finite source, CPU, private-stage, state and output budgets; omit an explicit
validation profile so the route retains the complete declared audit. The
launcher-provided sealed `TOS_STAGE_TICKET_FD` and `TOS_STAGE_ROOT` remain
required and are verified by the native owner. The validation lane passes the
FD only to this exact command. Missing or mismatched source, worker, invocation,
or stage custody fails closed; no Python validation fallback is invoked.

For an explicit source-first migration observation, run
`tos source-projection-coverage --root /absolute/source-root --invocation /absolute/protected-invocation.json --rows`
from the repository. `--invocation` selects the actual Access executable digest,
native schema-worker image and original capture/resource controls using the
protected `tos_local_native_foundation_invocation_v1` contract. The maintained
Foundation kernel owns the complete catalogue and worker exchange; coverage
consumes its sealed stage in a borrowed callback and checks final source/root
and metadata-publication currentness before its complete summary. The Rust
`tos-access` command validates the exact public
catalog inputs and Claims, checks generated-catalog currentness, then compares
the full source JSON values (including unknown fields and explicit nulls) with
the ordinary normalized read model. Rows distinguish direct/adapted mapping,
missing carriers, conflicting records and unresolved source return. The final
`tos_source_projection_coverage_v1` summary alone confirms that enumeration
completed; an interrupted, stale-input or refused-budget stream is incomplete.
`--root`, `--max-input-bytes`, `--max-rows` and `--max-seconds` bound the scan;
`--graph /absolute/graph.json` supplies an explicit normalized graph snapshot
instead of reading the native held snapshot. `--native-prefix` may be used
instead of `TOS_NATIVE_PREFIX`. Imported callers retain `observe_record` and
`coverage_report(..., invocation=protected_path)` callbacks, which forward to the same installed native
command; their graph-currentness callback remains caller-owned and runs after
the native stream has reached successful EOF/status and child cleanup, before
the summary is returned. `coverage_report` accepts an encoded graph request up
to 256 MiB, depth 64, and 1,000,000 JSON visits; `observe_record` remains bounded
to a 16 MiB request. The imported call uses a 120-second work limit plus five
seconds for cleanup. Its clock starts before graph encoding and process setup,
so those steps use part of that limit. The command-line `--max-seconds` accepts
up to 3600; the native wrapper uses that value plus five cleanup seconds on one
original clock. Neither limit promises a runtime fit. Graph bytes also count
against the native command's combined selected-input budget, so 256 MiB is a
transport ceiling rather than an allowance in addition to source inputs.

This offline diagnostic reports source identities, source references, field
names and source-file SHA-256 values; it does not export source wording, private
native inventory or payload. Exact JSON-field retention is separate from
source-file formatting, semantic understanding, form quality, rights and
admission. Uncatalogued families and the rest of the ToS corpus remain outside
this bounded report; a missing carrier does not imply restricted access or
falsity.

A `Link` is a first-class observed route with provider, interface kind,
technical access status, observation timestamp, provenance event, and a stable
ToS ID independent of the mutable URL. Its association with a Work or another
bibliographic object is an evidence-bearing `object-link` claim. Link
availability never supplies a license: reusable-content conclusions remain in
the separately scoped rights record and its human review posture.

The corpus follows `work -> expression -> edition -> item -> file`. Aggregate
volumes route through `collections/` and point to contained works/expressions
with evidence-bearing membership claims. A collection is not duplicated under
every contained work.

When a philosophy backlog names one contribution inside an aggregate volume,
its source planting cites the contained Work, the Collection container, and
the exact `contains_work` claim together. A table of contents, review link, or
separately accessible chapter cannot silently stand in for the whole volume.

When the backlog names a standalone bibliographic Work, its planting may cite
that Work directly. It must not manufacture a Collection merely to satisfy a
route shape. Expression, Edition, Item, and file identities remain absent until
their own evidence supports them; publication metadata on the Work is not an
implicit complete ladder.

When the backlog names a thematic or historically recovered subset inside a
standalone Work, retain that wording and evidence as planting scope rather
than minting another Work. The parent Work identity, subset membership claims,
provider coverage observations, and later interpretations are separate
layers. Approximate publication totals and mutable corpus result counts cannot
silently become one exact membership set.

Physical artifacts use the parallel `artifacts/` spine. Its route is based on physical, excavation, or
custody identity rather than a current provider. The artifact, catalog record,
inscription or transliteration, scholarly composite, photograph, and line art
remain separate. A philosophy branch may cite an artifact through a tracked
source planting, but the planting records its source-return route and frontier qualifications.

An exact artifact photograph may be retained only as its own content-addressed
File-backed visual representation with exact provider, rights, acquisition,
fixity, and provenance closure. The photograph retains its own representation identity, source context and
rights posture. Text extraction, translation and philosophical interpretation
require separately grounded records. An artifact with no honest
backlog anchor remains unplanted rather than borrowing a neighboring source's
identity.

Modern scholarly composites use the parallel `scholarly-composites/` spine.
Their path follows composition and tradition rather than a current corpus or
database. A composite identity may gather physical or textual member
observations and stabilize editorial coordinates, but it remains distinct from
each member, its physical layout and transcription, the mutable provider
representation, and every reading or interpretation. Conflicting or
differently scoped provider counts remain dated coverage observations instead
of being flattened into one hidden membership truth.

Collection work-boundary maps bind the source-visible member order to one
exact item/file digest and its text-free page inventory. They may carry
whole-page title, table-of-contents, and non-work boundary anchors plus
an optional evidence-bearing expression-responsibility claim. Translation
responsibility remains explicit where applicable and is `null`, not
manufactured, for a source-language member without a separate responsibility
claim. Such a map describes bibliographic structure with its proposed anchors and
recorded review posture.

The first four identity levels also close over three explicit corpus-wide
claim families under `relations/`. Work records cite their exact
`has_expression` packets through `expression_claim_refs`; Expression records
cite `embodied_by` packets through `embodiment_claim_refs`; Edition records
cite `exemplified_by` packets through `exemplar_claim_refs`. The direct record
fields retain speaking structural topology, while the claim packets add stable
claim IDs, exact record/manifest evidence, maker, provenance, visibility, and
review posture. The validator requires bidirectional exact closure, not merely
that both surfaces happen to exist. These relations describe the bibliographic ladder; textual identity, critical
equivalence and translation quality require their own evidence and assessment.

Native growth retains those legacy streams unchanged. The separate
[local Item adoption command](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_ITEM_ADOPTION.md)
can retain one already obtained file at the canonical local payload root,
create its provisional Item and distinct `exemplified_by` Claim, and append
only the existing Edition's exact backlink. The source-safe metadata package
and private byte-copy continuation are separate; their receipts preserve the
compound history. The operation retains an already obtained file under its existing rights and
local visibility conditions. Generic Claim correction cannot
rewrite that compound topology independently.

Expression derivation is a separate evidence-bearing claim family rather than
a fourth structural rung. Its directed `is_derivative_of` packets identify a
reported source Expression, derivation kind, directness, statement basis, and
collation state. They are explicitly non-transitive, asymmetric, irreflexive,
and no-equivalence: chronology, edition numbering, a shared translator label,
or a generated graph cannot fill a missing historical edge.

Work, Expression, and Edition records may close over evidence-bearing
legacy `responsibility-claims.jsonl` rows through `responsibility_claim_refs`.
The initial typed predicates are `authored_by` and `contributed_by` on a Work,
`translated_by` on an Expression, and `edited_by`, `afterword_by`, or
`designed_by` on an Edition. Every object resolves to an Agent. Every claim is
referenced by its actual subject, its file is digest-bound by the cited
provenance event, and unreferenced or cross-subject claims fail validation. Each role statement remains a versioned Claim: author, contributor,
translator, copyist, editor, paratext author, designer, publisher and rights
holder retain their specific responsibilities.

The [native translator attachment](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_EXPRESSION_RESPONSIBILITY.md)
adds a separately scoped Expression-to-existing-Agent Claim in a new
`relations/<slug>/source-claims.jsonl` home. The foundation joins only its exact
committed, continuously retained evidence with the unchanged legacy carriers.
Competing translator reports are not collapsed by endpoint equality. External
URLs remain attribution citations, with local citing-Claim source return and
no implied remote reading; endpoint metadata bindings are not extra witnesses.

In the current bounded corpus, all seven Nietzsche Work records close over one
`authored_by` claim to the GND-backed Friedrich Nietzsche Agent. Each claim
returns to its recorded authorial-witness discovery and research route and is
digest-bound by an annotation event. All seven remain model-made,
`public_metadata_only`, and `unreviewed`; their authorship assessments remain bound to the cited evidence.

The exact 1911 fourth-edition Antonovsky Expression closes over its own
`translated_by` claim and a separate RSL/RuNEB Item. Proposed whole-page
anchors bind the title-page responsibility, edition and publication statement,
the distinct Energiya printer line, and Antonovsky's edition-history preface
to the exact local file digest. The preface names the 1900, 1903, 1907, and
current 1911 editions and says the three previous editions were not simple
reprints of their predecessors but new reworkings. It does not explicitly
establish a 1911-to-1907 derivation. That source-visible statement makes
edition separation foundational: no shared
translator label can establish an invariant Russian text. The 62,952,283-byte
PDF remains ignored and local-only; the tracked layer admits no embedded OCR,
equivalence, redistribution permission, or future-site payload.

The provisional 1900 Antonovsky Expression now closes over one exact
institutionally identified Edition. Current RSL and RNL records reconcile one
exact route through the shared `rc\\1717107` core, matching transaction value,
and bibliography. They report Saint Petersburg, year 1900,
`тип. Б.М. Вольфа`, extent `[4], XII, 624 с.; 16`, two RSL holdings, and RNL
holding `128/318`. The current LNB record agrees on title, responsibility,
place, year, and 624-page main extent and reports holding `1.40686`, but gives
`[s.n.]` and `[3], XI`. That discrepancy remains unresolved pending
source-visible pages. All four remote holdings are outside the ToS Item ladder:
no source copy or bytes were acquired, `exemplar_claim_refs` remains empty,
and no File, anchor, publisher identity, provision activity, source text,
rights clearance, collation, equivalence, or new derivation edge is admitted.
The tiered LNB request is refocused on the discrepancy but remains public-safe,
optional, and `draft-not-sent`.

The provisional 1903 Antonovsky Expression now likewise closes over one exact
institutionally identified Edition. The Russian State Library record reports
`2-е изд., испр.`, Saint Petersburg, `тип. Альтшулера`, year 1903, extent
`[2], III, [3], 432, [2], V`, and four physical holdings. A current RNL
crosswalk resolves the same bibliographic control core to Primo
`07NLR_LMS004843722`, system number `004843722`, and two additional holdings
under `129/5943` and `38.35.6.32`. The distinct identifier wrappers are not
collapsed, the six shelfmarks remain external coordinates rather than ToS
Items, and one undefined `TEMP` row is rejected. No record-specific digital
object, bytes, or source pages were acquired.
The typography string is not inferred to be a publisher and opens no
provision-activity claim. No responsibility claim, collation, equivalence,
new derivation edge, rights conclusion, accepted text, or semantic object is
created. Separate tiered RSL and RNL copy requests are public-safe and
`draft-not-sent`; the RNL digitization route remains recommendation-only.

The provisional 1907 Antonovsky Expression now closes over one exact
institutionally identified third Edition. The current RuNEB record reports
Saint Petersburg, `тип. Ф. Вайсберга и П. Гершунина`, year 1907, the RSL as
source institution, and code `000199_000009_003693382`; the live card also
says that the edition was removed or replaced. A later exact RNL Primo pass
supersedes the current-catalogue gap: record `07NLR_LMS004843723`, system
number `004843723`, and its public item rows expose three holdings in the
Russian Book Fund under `17.145.5.1`, `17.145.5.1а`, and `17.145.5.1 Б`, each
reported `В хранении`. The catalogue code `Web заказ` is preserved literally;
official RNL guidance does not establish actual online ordering for a 1907
edition, and the record-specific scan form is recommendation-only. The remote
holdings are not ToS Items, and no source pages or bytes were acquired. The
same exact RUSMARC response exposes `001` `v19\rc\1717109` and field
`899 |a RuMoRGB |j V 106/216`. Official RUSMARC and MARC organization-code
documentation support the narrow statement that RNL reports an RSL call
number; they do not confirm the current RSL copy. `V 106/216` is therefore
tracked as `RSL shelfmark reported by RNL`, while predicted RSL record
`01003693382` stays rejected at HTTP `404` and the separate `БАН` row without
a call number stays unresolved. The refocused RSL/RuNEB request asks for
institutional confirmation before any source-copy question and remains
`draft-not-sent`. The
typography string is not inferred to be a publisher and opens no provision
claim. No Item, File, exemplar claim, responsibility claim, collation,
equivalence, changed derivation, rights conclusion, accepted text, or semantic
object is created. All three 1907 requests remain public-safe and
`draft-not-sent`.

The exact 1913 Antonovsky Expression separately closes over one
`translated_by` claim. Its evidence is a proposed whole-page anchor on PDF
page 7, bound to the exact Item and file SHA-256 after direct source-visible
inspection. The source retains the displayed label `Ю. М. Антоновский` in
that historical claim evidence, but a later independent
identity pass resolves the Agent to `Юлий Михайлович Антоновский`: DNB GND
`123235553` joins the initials to `Julij Michajlovič`, translator occupation,
and *Tak govoril Zaratustra*, while RSL supplies the Cyrillic full-name bridge
and dates `1857-1913`. This does not establish textual equivalence among the
1911, 1913, 1981, 1996, 2007, and later Expressions. The anchor and Claim retain their model-made, unreviewed state.

The 2007 Cultural Revolution Expression now closes over a separate
`translated_by` claim to the same stable Agent. Proposed whole-page anchors on
PDF pages 3 and 4 preserve both the direct translator credit and the fuller
statement that the historical translation was checked and newly edited. The
source-visible general editor, scientific editor/checker, commentary
translator, and designer remain exact deferred-role evidence because the
current predicates cannot express them without loss. Textual equivalence across witnesses and translation quality remain open to
source-visible comparison and assessment.

The three other translator Agents named by the 1996 Mysl boundary map follow
the same separation between a source credit and a person identity. An ordered
authority pass verifies Карен Араевич Свасьян as GND `120452367` and Николай
Николаевич Полилов as GND `1012315509`; their original initials remain
source-bound variants and their existing stable Agent refs do not change.
`В. А. Флёрова` remains provisional with no external identifier because the
official record chain stops at initials. `Вера Александровна Флёрова`, born in
1913, is explicitly rejected as a chronologically impossible match to a 1907
translator credit. The existing responsibility Claims, Expression identities and review states
remain unchanged.

The exact 1996 Mysl operator PDF now has an eleven-layer RU/US rights record
rather than one undifferentiated unevaluated status. Nietzsche's seven Works
and the historical Antonovsky and Polilov translation substrates are
`public_domain_reviewed`; Svasyan's *Genealogie* translation and his
compilation, editing, and notes remain `in_copyright`; Flerova's unresolved
identity, publisher contribution, portrait/presentation, scan, and ABBYY text
remain `copyright_undetermined`; the mixed aggregate is therefore
`in_copyright`. This is model-authored legal-research evidence, not human legal
review or exact-1996-expression clearance. The payload remains local, public
discoverability is not permission, and all content-bearing future-server
derivatives remain prohibited.

Antonovsky also exercises the explicit identity/path boundary. The already
referenced `tos.agent.yuri-antonovsky` remains the stable object of four
responsibility claims, but the Agent file moves from the misleading human route
`agents/yuri-antonovsky/` to `agents/yuliy-antonovsky/`. `Yuri M. Antonovsky`
is rejected as a name expansion, and the surviving `yuri` token is documented
only as a legacy stable locator. The later 2007 claim reuses that verified
Agent without rehabilitating the rejected name expansion. The path migration
and claim reuse create no new Agent, text, or equivalence.

Those seven Work records also each close over one
`first_publication_chronology` claim under `chronology/`. This ordering facet records first publication; composition, manuscript, printing,
title-page dating, private issue, public sale, reception, and digitization
remain distinct chronologies. The staged *Zarathustra* profile preserves the
1883–1884 public parts and the private 1885 fourth part; *Götzen-Dämmerung*
keeps 1888 production separate from its 1889 public release; *Der Antichrist*
and *Ecce Homo* remain posthumous editorial publications. All seven claims are
model-made, `public_metadata_only`, and `unreviewed`.

An Edition may route its `publication_claim_refs` to a sibling
`publication-claims.jsonl`. Every row remains an evidence-bearing
`tos_claim_packet_v1`: the Edition is the subject, its provenance event and
evidence resolve, and its review state remains explicit. A title-page year,
printing date, authorial receipt, sale release, publication role, print-run
extent, and later edition or issue state are separate assertions. An issue
claim must preserve unresolved textual identity and difference when no
compared witness supports either conclusion. Each chronology retains its reported or observed assertion layer and its own
assessment state.

An Edition may separately close over sibling `provision-activity-claims.jsonl`
rows through `provision_activity_claim_refs`. A provision claim groups one
typed publication, production, distribution, or manufacture statement with
its literal wording, role-specific places and agents, bounded temporal
assertion, evidence, provenance, and review posture. Literal transcription or
report and normalized identity remain separate: an authority match cannot
overwrite the statement, and the statement alone cannot manufacture a Place
or Organization identity. The original two-case slice resolves one
provisional Leipzig Place and two provisional historical publisher
Organizations for the 1889 *Götzen-Dämmerung* and 1908 *Ecce Homo* Editions.
It explicitly rejects the C. G. Naumann printing company and modern Berlin
Insel successor as publisher shortcuts. A later exact-Edition extension adds
the 1883 first part of *Also sprach Zarathustra*, a provisional Chemnitz Place,
and the historical `Ernst Schmeitzner, Verlagsbuchhandlung` Organization. It
keeps that Corporate Body distinct from the Person Ernst Schmeitzner. A
following pass adds parts II and III only from their own DTA source
descriptions and separate Edition identities; repeated labels, the part-I
claim, and the shared holding shelfmark are not evidence for propagation. A
fourth-part pass adds only the exact DTA-reported `Naumann; Leipzig; 1891`
statement. It reuses the provisional Leipzig Place and C. G. Naumann Verlag
Organization while rejecting the printer and founder Person as substitutions;
the 1885 private print, 1890 printing, planned 1891 delivery, and actual March
1892 delivery remain distinct historical stages. The Antonovsky 1913 pass then
adds two source-visible claims with a different shape: publication joins the
page-5 cover imprint to the page-7 place/year, while manufacture keeps the
page-8 brothers-Linnik printer line separate. Both reuse one provisional Saint
Petersburg Place but resolve to distinct provisional Organizations. All eight
claims remain model-made, `public_metadata_only`, and `unreviewed`. A final
consolidated-Edition pass adds two more claims from the exact 1893 Naumann title
page: its one combined `Druck und Verlag` literal is shared by separate
publication and manufacture roles. The provisional GND-backed publisher and
printer Organizations remain distinct normalization targets without a legal-
identity or `same_as` assertion. All ten Edition claims remain model-made,
`public_metadata_only`, and `unreviewed`.

The exact 1886 *Jenseits von Gut und Böse* Item adds a second shared-literal
case without copying either 1893 claim. Its page-3 `Druck und Verlag` wording
supports separate publication and manufacture claims through the same
role-specific provisional publisher and printer Organizations. A separate
page-4 anchor preserves `Alle Rechte vorbehalten.` only as historical rights
evidence. All twelve Edition provision Claims retain their model-made,
`public_metadata_only`, `unreviewed` state and their stated
publication/manufacture scope. A later independent layer-specific assessment reviews
current rights evidence without changing any provision claim.

The 1892 second *Zur Genealogie der Moral* Item adds two more claims without
copying either shared-literal case. Page 5 anchors the publisher and Edition
year; page 204 anchors a separate, undated printer line. The title-page year is
retained as an explicitly bounded Edition statement year rather than an exact
manufacture or release date. All fourteen Edition provision claims remain
model-made, `public_metadata_only`, and `unreviewed`.

Provision chronology is also facet-specific. The 1889 title-page statement is
not the reported printing completion, authorial receipt, or public-sale date;
the bracketed 1908 DNB year is not an exact release date; and the part-IV 1891
DTA year is not its private-print, printing, planned-delivery, or
actual-delivery date. The generated graph
may connect a reified claim to normalized Place and Organization nodes, but it
must never emit a direct Edition-to-identity fact edge. The frozen two-case
A/B/C inspection and its negative controls are recorded in
`ToS/research-packets/foundation-laboratory-2026-07/PROVISION_ACTIVITY_ABC_EXPERIMENT.md`.
The post-experiment first-part identity admission is separately recorded in
`ToS/research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_PART1_PROVISION_IDENTITY_RESEARCH.md`.
The independently researched part-IV boundary is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_PART4_PROVISION_IDENTITY_RESEARCH.md`.
The source-visible Antonovsky publication/manufacture split is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1913_PROVISION_IDENTITY_RESEARCH.md`.
The exact consolidated-Edition statement, shared title-page provision literal,
role-specific Naumann authorities, and historical translation-right boundary
are recorded in
`ToS/research-packets/foundation-laboratory-2026-07/NAUMANN_1893_PROVISION_IDENTITY_RESEARCH.md`.
The independent 1886 *Jenseits* title-page and historical-rights pass is
recorded in
`ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_PROVISION_IDENTITY_RESEARCH.md`.
The distributed 1892 *Genealogie* publisher/printer pass is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/GENEALOGIE_1892_PROVISION_IDENTITY_RESEARCH.md`.

## Payload boundary

Only an item's `payload/` content is ignored by Git. Everything required to
identify, verify, understand, authorize, and reacquire that payload remains
tracked:

- item and file identity;
- original basename and media type;
- size and cryptographic digest;
- deterministic page, container-resource, or TEI-structure inventory without
  copied source text;
- acquisition/source record;
- forensic inspection;
- rights and visibility posture;
- preservation and transformation events;
- references from accepted source-near text and anchors.

Payload files are immutable after intake. Changed bytes create a new file
record and, when materially distinct, a new item or item version.

`resource-inventory.json` is a tracked mechanical companion generated from the
exact payload digest. It enumerates PDF page geometry and image counts, bundled
DjVu page order and geometry, EPUB member/spine order and member fixity, TEI
page-break/division structure, OSIS chapter/verse order, top-level JSON
members/counts, or provider DjVu/ABBYY OCR page geometry and counts. Text-bearing
EPUB, TEI, OSIS, JSON, plain-text, and OCR resources may carry only one-way
fingerprints and bounded counts. The inventory supplies text-free mechanical
structure; reading, edition identification and rights assessment retain their
owner routes.

The maintained resource-inventory route is `tos-native-owner-command acquisition`
with a bounded one-line JSON request on stdin. It selects repository and payload
roots explicitly, writes or checks each Item's declared `resource-inventory.json`,
and verifies payload fixity and the source-resource-inventory schema before
writing. Replace the example roots and date with the selected local source and
run date. For a read-only check, set `check` to `true`:

```json
{"family":"inventory","operation":"build","repo_root":"/absolute/Tree-of-Sophia","payload_source_root":"/absolute/local/payload-root","event_date":"YYYY-MM-DD","check":true}
```

Omit `check` to regenerate declared inventories from the same verified payloads.
The Rust owner covers PDF, DjVu, DjVu XML, ABBYY XML gzip, JP2 ZIP, scandata XML,
EPUB, plain UTF-8/text, JSON, TEI, and OSIS profiles. The source-resource-inventory
contract retains `build_source_resource_inventories.py` as its historical
generator identifier; the Rust owner is the active producer. The preserved
Python implementation is retained at `tests/oracles/source_resource_inventory/`
for migration fixtures and historical reference behavior, not as a production
fallback.

Payload custody verification and copy receipts use the same native acquisition
command (`family: custody`, `operation: cli`, `command: verify` or `copy`) with
explicit payload roots and manifest/inventory selectors. The Rust owner checks
fixity, no-follow path custody and no-clobber publication, then writes an
immutable metadata-only receipt; it does not change rights, source assessment or
admission status.
The bounded `plain_utf8_file_v1` profile adds one inert complete plain-text or
Markdown file: exact raw-byte extent/fixity and UTF-8/BOM, code-point, newline
and observed Unicode-form facts only. It neither rewrites bytes nor interprets
markup, follows links, executes code or supplies a TextLayer/segmentation.

Large working derivatives, model caches, OCR scratch, page renders, and
benchmark outputs belong to the `abyss-stack` laboratory or host-managed cache,
in their designated storage homes. Reviewed text or annotation small enough to be
authored may return through `texts/`, `alignments/`, candidate intake, or canon
according to its authority layer and rights posture.

The first whole-work lexical observation index is a deliberately split
exception with no runtime authority. Its source-bearing SQLite/FTS5 database
stays inside the foundation pilot's ignored `local-content/`; its tracked
companion contains only form hashes, counts, and references to existing TEI
page/division resources. The form hashes are dictionary-recoverable navigational fingerprints and
retain the source’s access restrictions. The route remains `local_only` and
blocked from the future site until rights review, with language and semantic assessment still pending.

Question-scoped exact context lives in a separate private source-bearing
layer. Its first control preserves a complete page-bounded KWIC
census under ignored mode-0600 `local-content/usage-context/`; only a
string-free plan, fixity/count/selector receipt, and provenance remain tracked
beside the lexical plan. The tracked records describe the extraction’s fixity, scope and provenance.
Linguistic units and semantic assertions require their own source-grounded
analysis; reuse and publication retain the source’s recorded rights
conditions.

## Seed source routes

This is a navigation index, not a parallel record of each item's assessment.
Stable Work, Collection, Edition, Item, and File IDs remain the identity
authority. Each linked item manifest returns to its exact fixity, rights,
provenance, resource inventory, and forensic report. Dated research packets
preserve detailed evidence and uncertainty; source-owned claim packets retain
relation authority. These links do not merge witnesses or promote proposals.

### *Also sprach Zarathustra*

Work: [tos.work.friedrich-nietzsche.also-sprach-zarathustra](works/friedrich-nietzsche/also-sprach-zarathustra/work.json).

- **German DTA parts I–IV:** [tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-schmeitzner-1883-part-1.dta-sbb-corrected-tei-p5](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/item.manifest.json), [tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-schmeitzner-1883-part-2.dta-sbb-corrected-tei-p5](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/editions/chemnitz-schmeitzner-1883-part-2/items/dta-sbb-corrected-tei-p5/item.manifest.json), [tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-schmeitzner-1884-part-3.dta-sbb-corrected-tei-p5](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/items/dta-sbb-corrected-tei-p5/item.manifest.json), and [tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-naumann-1891-part-4.dta-sub-goettingen-corrected-tei-p5](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1891-part-4/editions/leipzig-naumann-1891-part-4/items/dta-sub-goettingen-corrected-tei-p5/item.manifest.json). The exact part witnesses and external quality, rights, and scope evidence remain distinct: [DTA rights assessment](../research-packets/foundation-laboratory-2026-07/DTA_ZARATHUSTRA_PARTS_1_4_LAYERED_RIGHTS_ASSESSMENT.md) and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_AUTHORIAL_WITNESS_ROUTE.md).

- **Russian witnesses and fragments:** [1911 RSL/RuNEB scan](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/items/rsl-neb-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1911.rsl-neb-scan-pdf), [1913 Commons/Penza scan](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1913/editions/saint-petersburg-zhizn-dlya-vsekh-1913/items/wikimedia-commons-penza-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1913.wikimedia-commons-penza-scan-pdf), [2007 Cultural Revolution PDF](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-cultural-revolution/editions/moscow-cultural-revolution-2007-volume-4/items/operator-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-cultural-revolution-2007.operator-pdf), [Nani 1899 fragments](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-nani-1899-nine-fragments/editions/saint-petersburg-stasyulevich-1899-nine-fragments/items/rsl-rusneb-parallel-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.ru-nani-1899-nine-fragments.rsl-rusneb-parallel-scan-pdf), and [Reader 1899 fragment](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-reader-1899-uncredited/editions/moscow-reader-editorial-office-1899/items/rnl-rusneb-fragment-pdf-parts/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.ru-reader-1899-uncredited.rnl-rusneb-fragment-pdf-parts). Findings remain in the [1911 source research](../research-packets/foundation-laboratory-2026-07/ANTONOVSKY_PROMETEY_1911_SOURCE_WITNESS_RESEARCH.md) and [rights assessment](../research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1911_RSL_RUNEB_LAYERED_RIGHTS_ASSESSMENT.md), [1913 provision research](../research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1913_PROVISION_IDENTITY_RESEARCH.md) and [rights assessment](../research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1913_LAYERED_RIGHTS_ASSESSMENT.md), [2007 responsibility research](../research-packets/foundation-laboratory-2026-07/ANTONOVSKY_CULTURAL_REVOLUTION_2007_TRANSLATION_RESPONSIBILITY_RESEARCH.md) and [rights assessment](../research-packets/foundation-laboratory-2026-07/ANTONOVSKY_2007_CULTURAL_REVOLUTION_LAYERED_RIGHTS_ASSESSMENT.md), and the [Nani](../research-packets/foundation-laboratory-2026-07/NANI_1899_PARALLEL_FRAGMENT_SOURCE_WITNESS_RESEARCH.md) and [Reader](../research-packets/foundation-laboratory-2026-07/READER_1899_FRAGMENT_SOURCE_WITNESS_RESEARCH.md) fragment packets.

- **Naumann 1893 scan family:** the image-container PDF [Item manifest](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/items/internet-archive-image-container-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-naumann-1893.internet-archive-image-container-pdf) and automatic EPUB [Item manifest](works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/items/internet-archive-cornell-auto-epub/item.manifest.json) (tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-naumann-1893.internet-archive-cornell-auto-epub) are distinct acquired Items. See the [rights assessment](../research-packets/foundation-laboratory-2026-07/NAUMANN_1893_IA_PDF_EPUB_LAYERED_RIGHTS_ASSESSMENT.md) and [Edition identity research](../research-packets/foundation-laboratory-2026-07/NAUMANN_1893_PROVISION_IDENTITY_RESEARCH.md).

### Other standalone Works

- *Jenseits von Gut und Böse*: [Work](works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json); exact 1886 Google/Harvard package [Item manifest](works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.internet-archive-google-harvard-scan-pdf). See the [rights assessment](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_IA_GOOGLE_HARVARD_LAYERED_RIGHTS_ASSESSMENT.md), [Edition provision research](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_PROVISION_IDENTITY_RESEARCH.md), and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/JENSEITS_AUTHORIAL_WITNESS_ROUTE.md).

- *Zur Genealogie der Moral*: [Work](works/friedrich-nietzsche/zur-genealogie-der-moral/work.json); exact Commons/UNC scan [Item manifest](works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.zur-genealogie-der-moral.de-naumann-1892-second.wikimedia-commons-unc-scan-pdf). Keep the Edition, acquired Item, related Russian collection member, and other provider revisions distinct. See the [rights assessment](../research-packets/foundation-laboratory-2026-07/GENEALOGIE_1892_COMMONS_UNC_LAYERED_RIGHTS_ASSESSMENT.md), [provision research](../research-packets/foundation-laboratory-2026-07/GENEALOGIE_1892_PROVISION_IDENTITY_RESEARCH.md), and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/GENEALOGIE_AUTHORIAL_WITNESS_ROUTE.md).

- *Der Fall Wagner*: [Work](works/friedrich-nietzsche/der-fall-wagner/work.json); exact standalone 1888 Naumann [Item manifest](works/friedrich-nietzsche/der-fall-wagner/expressions/de-naumann-1888/editions/leipzig-c-g-naumann-1888/items/mdz-bamberg-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.der-fall-wagner.de-naumann-1888.mdz-bamberg-scan-pdf). See the combined [MDZ rights assessment](../research-packets/foundation-laboratory-2026-07/MDZ_FALL_WAGNER_1888_GOETZEN_1889_LAYERED_RIGHTS_ASSESSMENT.md) and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/FALL_WAGNER_AUTHORIAL_WITNESS_ROUTE.md).

- *Götzen-Dämmerung*: [Work](works/friedrich-nietzsche/goetzen-daemmerung/work.json); exact standalone 1889 Naumann [Item manifest](works/friedrich-nietzsche/goetzen-daemmerung/expressions/de-naumann-1889/editions/leipzig-c-g-naumann-1889/items/mdz-bsb-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.goetzen-daemmerung.de-naumann-1889.mdz-bsb-scan-pdf). Keep it distinct from the altered 1893 Edition. See the combined [MDZ rights assessment](../research-packets/foundation-laboratory-2026-07/MDZ_FALL_WAGNER_1888_GOETZEN_1889_LAYERED_RIGHTS_ASSESSMENT.md) and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/GOETZEN_DAEMMERUNG_AUTHORIAL_WITNESS_ROUTE.md).

- *Ecce homo*: [Work](works/friedrich-nietzsche/ecce-homo/work.json); exact 1908 Insel [Item manifest](works/friedrich-nietzsche/ecce-homo/expressions/de-richter-insel-1908/editions/leipzig-insel-verlag-1908/items/wikimedia-commons-getty-scan-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.ecce-homo.de-richter-insel-1908.wikimedia-commons-getty-scan-pdf). Keep Nietzsche's Work, Richter/Köselitz's editorial roles, and van de Velde design as separate evidence layers. See the [rights assessment](../research-packets/foundation-laboratory-2026-07/ECCE_HOMO_1908_COMMONS_GETTY_LAYERED_RIGHTS_ASSESSMENT.md) and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/ECCE_HOMO_AUTHORIAL_WITNESS_ROUTE.md).

### Aggregate publications

- 1996 Russian Volume II Collection [record](collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json) (tos.collection.friedrich-nietzsche.works-in-two-volumes-volume-2-mysl-1996): exact operator PDF [Item manifest](collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/item.manifest.json) (tos.item.friedrich-nietzsche.works-in-two-volumes-volume-2-mysl-1996.operator-pdf) and [work-boundary map](collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/work-boundary-map.json). See the [Volume II rights assessment](../research-packets/foundation-laboratory-2026-07/MYSL_1996_VOLUME_2_LAYERED_RIGHTS_ASSESSMENT.md) and [translator identity research](../research-packets/foundation-laboratory-2026-07/MYSL_TRANSLATOR_IDENTITY_RECONCILIATION_RESEARCH.md).

- 1906 Naumann *Nietzsche's Werke*, Band VIII Collection [record](collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/collection.json) (tos.collection.friedrich-nietzsche.nietzsches-werke-band-8.naumann-1906): Commons/Stanford scan [Item manifest](collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/item.manifest.json) (tos.item.friedrich-nietzsche.nietzsches-werke-band-8.naumann-1906.wikimedia-commons-stanford-scan-djvu) and separate IA DjVuXML derivative [Item manifest](collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/item.manifest.json) (tos.item.friedrich-nietzsche.nietzsches-werke-band-8.naumann-1906.internet-archive-google-stanford-djvu-xml) stay separate Items. The partial *Der Antichrist* boundary remains a proposal in the [work-boundary map](collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/structure/work-boundaries/work-boundary-map.json); see its [rights assessment](../research-packets/foundation-laboratory-2026-07/ANTICHRIST_1906_COMMONS_STANFORD_LAYERED_RIGHTS_ASSESSMENT.md) and [authorial-witness route](../research-packets/foundation-laboratory-2026-07/ANTICHRIST_AUTHORIAL_WITNESS_ROUTE.md).

Item manifests lead to exact item-level files. Source-owned identity and
relation packets remain with each Work, Expression, Edition, Collection,
membership, and boundary map. Case-specific findings stay in their packets.

## Source-near movement

```text
payload + receipt
  -> forensic inspection
    -> raw extraction or OCR proposal
      -> anchored manual correction
        -> reviewed text / alignment
          -> candidate observation or claim
```

Semantic admission follows the source-visible review stages in this route.

## Additive source and transformation contracts

The contracts below separate identity, content, execution, interpretation, and
alignment. Each synthetic fixture is a mechanics check on invented or otherwise
public-synthetic input; it is not corpus evidence, a source migration, or an
acceptance decision. The contract and fixture links are the authoritative
detail for their fields and controls.

| Contract | Synthetic fixture |
| --- | --- |
| [Source Anchor v2](../contracts/source-anchor-v2.schema.json): stable anchor identity, exact Item/File representation and digest, selector composition, and publication posture | [A/B/C lab manifest](../research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/lab.manifest.json) |
| [Source Text Layer](../contracts/source-text-layer.schema.json): immutable raw, diplomatic, reviewed, or normalized layer with explicit predecessor, policy, and replayable edits | [A/B/C lab manifest](../research-packets/foundation-laboratory-2026-07/source-text-layer-abc/lab.manifest.json) |
| [Provenance Event v2](../contracts/provenance-event-v2.schema.json): input/output/byproduct entities, derivation, execution, failure, fixity, authentication, review, rights, and replay posture | [A/B/C lab manifest](../research-packets/foundation-laboratory-2026-07/provenance-event-v2-abc/lab.manifest.json) |
| [Semantic Annotation Packet v2](../contracts/semantic-annotation-packet-v2.schema.json): source-anchored occurrence and annotation proposals with separate claim, review, and projection states | [A/B/C lab manifest](../research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/lab.manifest.json) |
| [Translation Alignment Packet v1](../contracts/translation-alignment-packet-v1.schema.json): exact source/target layers, units, anchors, competing mappings, and review posture | [A/B/C lab manifest](../research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/lab.manifest.json) |

Existing v1 anchors, provenance, and corpus text were not migrated. A v2
successor belongs to a concrete source-return or replay question. Mechanical
validation does not establish source truth, language competence, accepted
reading, translation equivalence, semantic promotion, rights, publication, or
human approval; those remain with the [knowledge assessment route](../doctrine/KNOWLEDGE_ASSESSMENT.md)
and the exact source review.

### Bounded real source-text and alignment routes

The active source-text routes and their limits are maintained with the
[foundation pilot records](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/README.md)
and exact tracked plans:

- [DTA Part I source-text foundation plan](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-source-text-foundation.plan.v1.json).
- [Antonovsky 1911 target-text foundation plan](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-antonovsky-1911-target-text-foundation.plan.v1.json).
- [Opening-sentence alignment proposal plan](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json).
- [Antonovsky 2007/1911 witness-collation plan](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/antonovsky-2007-1911-opening-sentence-collation.plan.v1.json).
- [Authored-canon evidence bridge plan](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/authored-canon-evidence-bridge.plan.v1.json) and [bridge record](works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/authored-route-evidence-bridge.za-i-vorrede-1.v1.json).

Exact private text remains in owner-local ignored custody; tracked records carry
selectors, digests, inventories, and posture. A route's result stays limited to
its recorded source, representation, and review state.

## Rebuild local resource inventories

The payloads may be absent from a public clone, so inventory generation is a
focused local operation rather than a release-gate download. The native
`family: inventory` request takes an explicit repository and payload root; the
source-witness foundation lane remains the declared whole-tree validation route.

The native owner preserves the maintained selector table below. Rows are
separate branches where the same media type selects a distinct profile. The
registry JSON/XML branches use the existing JSON, TEI, and OSIS inventory
owners; remaining profiles stay with the source-item inventory extension.

| # | Maintained selector | Native owner and resulting profile |
| --- | --- | --- |
| 1 | `application/json` | `source_item_inventory::json_inventory` → `json_members_v1` |
| 2 | `text/plain` with `plain_text_v1` | `plain_text_inventory` → `plain_text_v1` |
| 3 | `text/plain` with `plain_utf8_file_v1` | `source_item_inventory::observe` → `plain_utf8_file_v1` |
| 4 | `text/markdown` | `source_item_inventory::observe` → `plain_utf8_file_v1` |
| 5 | `application/osis+xml` | `source_item_inventory::osis_inventory` → `osis_structure_v1` |
| 6 | `application/pdf` | `pdf_inventory` → `pdf_pages_v1` |
| 7 | `application/epub+zip` | `source_item_inventory::observe` → `epub_resources_v1` |
| 8 | `application/zip` with `_jp2.zip` path | `jp2_zip_inventory` → `jp2_zip_pages_v1` |
| 9 | `image/vnd.djvu` | `djvu_inventory` → `djvu_pages_v1` |
| 10 | `application/vnd.djvu+xml` | `djvu_xml_inventory` → `djvu_xml_pages_v1` |
| 11 | `application/gzip` with `.abbyy.xml.gz` path | `abbyy_inventory` → `abbyy_xml_pages_v1` |
| 12 | `application/xml` or `text/xml` with `_scandata.xml` path | `scandata_inventory` → `scandata_pages_v1` |
| 13 | `application/xml` or `text/xml` with an OSIS root | `source_item_inventory::osis_inventory` → `osis_structure_v1` |
| 14 | `application/tei+xml` | `source_item_inventory::tei_inventory` → `tei_structure_v1` |
| 15 | `application/xml` or `text/xml` containing TEI text | `source_item_inventory::tei_inventory` → `tei_structure_v1` |

Only PDF geometry and image enumeration call external parsers. Under the
bounded runner's fixed `/usr/bin` search path, the required programs are
`/usr/bin/pdfinfo` and `/usr/bin/pdfimages`; the runner sends the exact
fixity-checked PDF bytes on stdin to `pdfinfo -`,
`pdfinfo -f 1 -l N -box -`, and `pdfimages -list -`. The binary DjVu parser,
DjVu XML parser, scandata/TEI/OSIS XML parsers, ABBYY gzip/XML parser, and EPUB
or JP2 ZIP member walkers run in the native owner and require no external
DjVu or archive command.

Omit `--check` only when intentionally regenerating tracked inventories from
the same fixity-verified local payloads. The source-foundation validator checks
their schema, manifest closure, counts, and digest-bound provenance without
claiming content correctness.

## Witness structure correspondence

The maintained producer is `tos-research witness-structure-correspondence`. Supply
`--source-root ABS --local-input-root ABS --check` to reconstruct the retained
82 correspondence candidates and 246 proposed anchors without writes. The
local input root is the repository-shaped payload custody root. A fresh run uses
`--build --generation NAME --event-at RFC3339 --scratch-bytes RESERVED_BYTES`
and creates a separate `native-NAME` directory. It verifies all six payloads,
including the PDF, and binds native provenance to their metadata and rights.


The tracked Naumann 1893 ↔ DTA-parts map under
`works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/`
uses named primary TEI division starts and exact EPUB/PDF resource locators.
Its generator transiently reads the local payloads but writes only resource
IDs, paths inside the containers, one-way fingerprints, and match metrics.

The companion `structure-anchor-set.json` and `structure-anchors.jsonl`
materialize three stable proposed addresses for every correspondence: the DTA
TEI structural path, the exact Naumann EPUB member, and the whole Naumann scan
page. These reusable, text-free IDs remain `proposed` for later source review. A binding between the three addresses means only
that the structure-map method selected them as one candidate route. Exact passage boundaries require their own resolved anchors and review.

Every result remains a `mechanical_candidate_only` locator. Headings, context scores, monotonic routes and page numbers supply locator
evidence. Textual identity, translation correspondence and semantic
interpretation require source-visible assessment. The local rebuild
and tracked validation routes live in [`scripts/AGENTS.md`](../../scripts/AGENTS.md).

The Jenseits Naumann 1886 ↔ Polilov/Mysl 1996 map under
`works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/`
uses the more general parallel-PDF contract. It binds eleven source-visible
division starts—preface, nine numbered main divisions, and aftersong—to exact
pages in both fixity-bound witnesses. The nine main divisions cover the
contiguous integer ranges 1–296 and preserve the source supplemental units
`65a`, `73a`, and `237a`.

The source item now carries a separate, text-free
`structure/numbered-unit-page-map.json`. An A/B/C comparison rejected embedded
PDF text as a complete structure source, retained DjVu XML as a secondary
coordinate layer, and selected ABBYY XML for ordered candidates. Bounded
source-visible review closed the remaining numeral gaps and exposed a repeated
printed `237.` on PDF page 189, retained locally as `237a`. The result is 299
monotonic proposed start-page candidates for §§1–296 plus 65a, 73a, and 237a,
with 299 whole-page proposed source anchors.

`tos-research jenseits-numbered-structure --source-root ABS --local-input-root ABS --check`
reconstructs that retained map through the native bounded ABBYY reader. A new
`--build` requires `--generation NAME --event-at RFC3339 --scratch-bytes BYTES`
and writes a separate `structure/native-NAME/` map, anchors and Rust provenance.
The local input root names the corpus root containing `ToS/source-witnesses/`;
original payloads and the historical generation remain unchanged.

The Polilov/Mysl target uses
`tos-research jenseits-polilov-numbered-structure --source-root ABS --local-input-root ABS --check`
with the exact held PDF and its metadata. `--check` verifies the retained
298 proposed page anchors and their provenance. A fresh `--build` requires a
writable selected source root, `--generation`, `--event-at` and an explicit
`--scratch-bytes` allowance; replay compares all output bytes before writing.
Historical reconstruction preserves its original provenance and the
unmaterialized source-only label `237a`.

The OCR supplies proposed page starts; exact line boundaries require
separately resolved anchors.
The parallel German ↔ Russian map still stops at division granularity and
materializes zero exact target-unit starts. In the Mysl witness, the prose
corresponding to source `237a` follows the Seven Sayings without a repeated
237/237a label; that visible asymmetry is preserved instead of being forced
into an equivalence.

The Polilov/Mysl expression now also carries its own target-only
`structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json`.
Its A/B/C route rejects plain embedded text as a complete map source, rejects
unordered bbox numerals without an order constraint, and selects ordered bbox
candidates plus bounded source-visible gap review. It materializes 298
monotonic proposed target-label starts for 1–296, `65a`, and `73a`, with the
absent target label `237a` recorded only as a source-map asymmetry. This target-only layer records proposed label starts. Source-to-target passage
correspondence remains a separate research question.

A release-safe companion under the existing parallel-structure route now
intersects only the number-label keys already materialized independently by
those two maps. It yields 298 proposed shared-label pairing candidates and
keeps source-only `237a` unpaired. The builder reads tracked maps and rights
records, not either payload or text. A pairing records that both witnesses materialize the same structural label,
with passage alignment and translation assessment left to their source-visible
routes.

The same exact Mysl Item supplies two additional target structure routes
without exposing its prose. *Zur Genealogie der Moral* preserves 78 proposed
starts across four independently resetting series—preface and three essays—
while *Der Antichrist* preserves 62 starts in one series. Their series-qualified
identities prevent repeated numerals from collapsing into a false flat unit
namespace. The original target-only crosswalks narrow the twelve frozen pages
to twenty possible target routes and remain unchanged as historical inputs.

`tos-research mysl-transfer-target-structure --source-root ABS --local-input-root ABS --check`
reconstructs both retained target maps, their anchors and frozen-page crosswalks.
Historical events keep the input and output digests from their original revision,
including events that predate later plan or rights changes. New `--build`
generations bind current inputs and cite the retained scan-review decisions.
Use `--generation NAME --event-at RFC3339 --scratch-bytes BYTES`; the producer
checks every existing destination before writing, and matching replay writes nothing.

Separate German source maps now materialize the same 140 structural labels.
The 1892 *Genealogie* Item supplies 78 proposed PDF starts. The 1906
*Antichrist* route preserves a stronger boundary: the 523-page Commons DjVu is
the address witness, while a separately registered 525-page Internet Archive
DjVuXML Item is navigation evidence under a source-visible, bounded two-page
offset. This relation is not full-container or textual identity. A payload-free
intersection pairs only identical `series:unit` keys, and a second composition
gives all twelve frozen pages twenty possible German structural routes. These routes provide proposed German structural starts; passage ends and
source-to-target correspondences remain unresolved.

`tos-research nietzsche-transfer-source-structure --source-root ABS --local-input-root ABS --check`
reconstructs the two retained German maps, anchors and provenance through Rust.
It holds the PDF, DjVu and DjVuXML witnesses separately, verifies their fixity
and keeps the declared page offset. The same explicit generation, timestamp
and admitted-byte options create separate native generations. Numbered-page
ranges remain inside their wider work boundary, which can also include title pages.

The frozen target side now has a separate exact-layer boundary preparation
step over the ignored Mysl PDF. It expands the twenty page candidates into
thirty-five numbered-unit passage candidates, using ordered PDF word geometry
from one visible unit label to the next same-series label. Thirty-two candidates
intersect their frozen page; three conservative spill routes (`Jenseits` 284 on
page 399, `Jenseits` 46 on page 279, and `Antichrist` 35 on page 661) are kept
as explicit rejected nonintersections instead of being silently discarded.
Private strings remain mode 0600 below ignored `local-content/`; Git receives
only selectors, geometry, counts, digests, proposed/rejected anchors, and
provenance. These candidates have exact target-layer selectors and retain their proposed
or rejected status pending textual and boundary assessment.

The German side now has a matching but deliberately partial exact-layer pass.
It keeps all thirty-five conservative routes and materializes thirty-two
private mode-0600 source candidates. Twenty-eight resolve inside one ABBYY,
DjVuXML, or PDF-bbox layer; four *Antichrist* routes use three exact visible
section markers in the same Internet Archive JP2 package, bound through its
scandata leaves to the first following DjVuXML line. Three routes remain
explicit unresolved-boundary records: *Jenseits* 32 and 201 and *Genealogie*
`essay-1:10`. Tracked artifacts remain text-free and retain content/address
witness separation; the *Antichrist* two-page navigation offset still asserts
no textual identity. The JP2 ZIP and scandata remain ignored local source
payloads. These candidates retain the source layers’ quality limits and their
provisional boundary states.

The Zarathustra foundation pilot also carries one deliberately narrower
German source-triangulation packet. A deterministic local builder compares an
ignored eKGWB HTTP response, the exact DTA part-I TEI, and two exact members of
the Naumann automatic EPUB, then emits only fixity, selectors, aggregate
counts, and one-way fingerprints. It preserves the failed HTTPS route,
unencrypted-transport risk, a source-aware dehyphenation control, and the OCR
residual. Machine agreement supplies reproducible comparison evidence for later
source-visible assessment.

A later tracked discovery receipt adds a distinct evidence layer: Arquivo.pt
preserves a 2023 snapshot of the same owner URL with capture timestamp,
collection, archive digest, WARC file, and offset. Its exact target block
matches the current owner object and include. This is independent historical
fixity corroboration; because the archive explicitly does not validate
archived source or content, it is not publisher-origin authentication, source
admission, German acceptance, or publication authority.

One derived sentence from that route is now preserved below the pilot's
ignored `local-content/translation/research-inputs/` branch. Its tracked
companion records the DTA selector, transformations, fixity, token count,
three-witness corroboration, rights posture, and closed authority gates
without copying the sentence into that packet. A matching string and authored
translations already exist in the older canon node and public compatibility
mirror; the calibration packet binds and seals those surfaces instead of
claiming global text absence. This makes the exact local string eligible for
blind `abyss-stack` model/prompt/runtime calibration only. It does not make
the string philologically accepted, revalidate the older canon role, open any
lane in the frozen translation plan, reveal a translation surface, authorize
redistribution, or create semantic, graph, or canon evidence.

The additive `translation-exposure-aware-plan.v1.json` now corrects one
historical method assumption without rewriting that frozen plan. Comparator
exposure is recorded per actor and event: the current operator's five-year
Work-level lived route remains witness-unspecified, while the exact Antonovsky
2007 page already seen in the current project independently prohibits a
`blind-human-only` label; a run-specific AI
producer may claim only operational prompt/retrieval/session isolation while
training exposure remains unknown; and post-freeze comparator analysis is
comparison evidence rather than a candidate. The current `ai_human` route is
an exposure-aware assisted retranslation. It creates no German competence,
accepted translation, comparator reveal, human task, semantic or graph
promotion, publication, or server transfer.
