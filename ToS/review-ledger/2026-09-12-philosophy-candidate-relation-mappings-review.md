# Philosophy candidate relation mapping review

Date: 2026-09-12 UTC. Immutable pre-change baseline:
`08050783bd1fffb434e1d91927e64c728808861d`.
Reviewer: delegated Codex implementation helper; integration master retains
independent semantic review and adoption. Scope: a source-visible registry
crosswalk review, not historical assertion assessment, candidate promotion,
canon, rights, publication or a human review event.

## Exact source basis

The reviewed records are already-present prepared research candidates. No
original DOCX or historical witness was opened or reinterpreted. Source JSONL
physical lines below are locators, distinct from the original document's table
and row fields. Complete record fields, including unknown members, remain
source-owned.

| Source stream under `ToS/philosophy/graph-workbench/` | File SHA-256 |
| --- | --- |
| `proposed-relations/table-i-prepared-dossiers.jsonl` | `75890483a34f4b41e2b45f592abdbe2a5df6212bf178289a05753421d57f24fa` |
| `proposed-relations/table-ii-prepared-dossiers.jsonl` | `578e140fa8720304540bc636852e0ae1781f8c00dce210abbfadb056b3160d60` |
| `proposed-nodes/table-i-prepared-dossiers.jsonl` | `e486e8ed6c320bb5bc065b793beba6f61dab968dfafcf65f491a0f3147539eb6` |
| `proposed-nodes/table-ii-prepared-dossiers.jsonl` | `3e9b6f773ee20d3a210fbee943f756e7e3bfc81bf55e1b027a409a9f0beb923a` |

The baseline relation registry is version 43, file SHA-256
`93614250e1ad43647a19db22fab6cab234861ab623d46a79bf9fd2cfd0f74841`;
the unchanged entity registry file SHA-256 is
`c6b393113a4c293436cc79ff0c1d6691b83fa1215bb75da2f66224201b6c5e1d`.

| Relations and physical lines | Exact endpoints and existing candidate types | Source-visible reading and limit |
| --- | --- | --- |
| Table I lines 1170–1172: `table-i-a35-relation-019`, `-020`, `-021`; predicate `figure_anchor` | `table-i-a35-node-008` Corpus Hermeticum, `-014` Discourse on the Eighth and Ninth, `-016` Asclepius (`tos.entity.text-corpus`) → `-018` Hermes Trismegistus (`tos.entity.figure`). Node lines 1242, 1248, 1250, 1252. | Comments say “Авторизован Hermes Trismegistus”, “Внутритекстовый герметический авторитет”, and “То же”. The common boundary is a reported authorizing/intratextual figure, not verified authorship or a normalized historical Agent. |
| Table II line 187: `table-ii-t2-05-relation-027`; predicate `translates_into` | `table-ii-t2-05-node-033` Ioane Petritsi (`tos.entity.figure`) → `-028` Petritsi–Proclus corpus (`tos.entity.text-corpus`). Node lines 189 and 184. | The comment reports Petritsi translating Elements of Theology into Georgian. The source direction is figure → corpus, not translation result → language/Expression. Master status B, confidence 4 and `manual_review_required` remain unchanged. |
| Table II line 2202: `table-ii-t2-56-relation-002`; predicate `uses_medium` | `table-ii-t2-56-node-002` Khipu three-dimensional sign system (`tos.entity.language-script`) → `-005` Primary/pendant/subsidiary cords (`tos.entity.medium`). Node lines 2053 and 2056. | The comment describes organization by primary and pendant cords. Status C, confidence 4, manual review and all frontier restrictions remain unchanged: information-system evidence is not a readable philosophical corpus, colonial descriptions remain later witness layers, and the Table I A48 frontier stays distinct. |

The normalized relation IDs keep the prefix
`philosophy:edge:candidate-relation:` followed by each full candidate ID. The
five carrier pointers at the baseline are `/edges/1200`, `/edges/1201`,
`/edges/1202`, `/edges/1805`, `/edges/3820` in
`ToS/derived-exports/philosophy_graph_projection.min.json`, file SHA-256
`c3eb2241fbab1bc80098a027ce2c25dbb13e37acdb8ccc912705d1fa59d2fc18`.
Pointers locate this baseline, not permanent source identity.

## Judgment and rejected shortcuts

The [2026-09-09 source-return review](2026-09-09-philosophy-atlas-source-return-review.md)
correctly retained these records without silently amending the registry. The
present, separately delegated review adds three explicit candidate-only
relations in registry 44:

- `tos.relation.candidate-authorizing-figure`: text-corpus → figure.
- `tos.relation.candidate-translator-involvement`: figure → text-corpus.
- `tos.relation.candidate-material-realization`: language-script → medium.

All are directed, nontransitive, evidence-bearing, recorded-review relations
with exact `philosophy` / `edge` mappings. Qualified RU/EN labels distinguish
dossier reporting from reviewer agreement. Their stable IDs identify the
interchange vocabulary, not accepted historical facts. Existing direct
candidate edges remain direct carrier representations; no source Claim
profile, source writer or reification is invented.

The review rejects broad Thing-to-Thing catchalls, inferred authorship,
normalizing a mythic/intratextual figure into an Agent, renaming
`translates_into` to `translated_into`, inferring translated Expression or
Artifact identity, and treating khipu as a deciphered text corpus. Existing
source comments support the narrower reports but cannot settle those stronger
claims. No independent historical corroboration or counterevidence search was
performed; the outcome authorizes no historical-use admission.

## Checklist, validation and remaining owner

The review checklist's source traceability, authored/derived distinction,
candidate versus canon boundary, endpoint identity, exact language/direction,
uncertainty and stronger-owner separation are preserved. No new branch,
lineage, template, golden-kernel, lived-witness, rights or publication route is
in scope. Original source and generated carrier bytes are unchanged; the
earlier review is preserved rather than retrospectively relabeled.

Seventeen focused tests passed, including a tiny exact eight-endpoint/five-edge
source slice, nine reversal/wrong-endpoint cases, exact mapping-scope negatives,
unknown-vocabulary fallback and registry-transition refusal tests. The exact
pre-change `semantic_registry_transition` lane passed against
`08050783bd1fffb434e1d91927e64c728808861d`, validating current and baseline
registry/schema snapshots without normalizing the corpus. A separate raw
carrier scan found zero unmapped node kinds or edge predicates after this
amendment; it does not count synthesized topology edges or replace a full graph
validation, and unrelated reified Claim predicate gaps remain outside scope.
The existing whole-projection test
now requires the three mappings while retaining its source-body/status checks;
this bounded implementation does not rerun the entire normalized corpus.
Local tests and transition checks do not prove historical meaning, CI, merge,
deployment or source admission. The integration master owns independent
meaning/test review, full-corpus validation scheduling, affected derived
consumer refresh and any separately authorized landing.
