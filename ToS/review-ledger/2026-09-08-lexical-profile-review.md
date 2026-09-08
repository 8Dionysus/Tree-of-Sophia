# Lexeme, written form and lexical sense: bounded review, 2026-09-08

Status: implemented source/profile and local-reader slice of Foundation L02.
Not complete language/text coverage, exact Occurrence creation, calibrated
linguistic assessment, research admission, UI acceptance or deployment.

## Owner changes and real source

The [lexical contract](../contracts/lexical-description-record.schema.json),
[Corpus Foundation](../doctrine/CORPUS_FOUNDATION.md) and
[registry explanation](../doctrine/semantic-interchange/README.md#lexemes-written-forms-and-contextual-senses)
own the distinctions. Entity registry 23 adds declared source profiles for
Lexeme, LexicalForm and LexicalSense through the existing semantic metadata
reader. Required accounts, scope and continuity criteria are substantive
source fields, not source text, exact addresses or a dictionary verdict.
LexicalSense retains its existing type identity and `tos.sense.*` namespace.

LexicalForm fixes the supplied written representation, language, script,
representation kind, notation scope and Unicode posture. `record.revise`
can correct its account but cannot rewrite `form_identity`. This object is
neither a `tos.form.*` human display packet nor a legacy computed string-group
key. Matching strings do not merge referents. Unknown semantic content and
extensions remain available without being interpreted.

The [source reading](2026-09-08-jgb-lexical-source-reading.md) identifies the
retained eKGWB capture, its exact fixity and the complete inspected JGB 19
section. This slice did not change or publish those source bytes. Provider
rights and transport limits remain explicit. Five provisional subjects were
created using ordinary source-owner commands, with 15 source-copy forms:

| Subject | Kind | Boundary |
| --- | --- | --- |
| `tos.language.german` | Language | language, not the Russian description or Latin script |
| `tos.lexeme.german-wille` | Lexeme | proposed lexical grouping, not one spelling or token |
| `tos.lexical-form.german-wille` | LexicalForm | supplied `Wille` representation, not an exact occurrence |
| `tos.lexical-form.german-willen` | LexicalForm | supplied `Willen` representation, without token-case attribution |
| `tos.sense.willing-jgb19` | LexicalSense | bounded reading of willing in JGB 19, not the philosophical conception |

[Four Claims](../source-witnesses/relations/lexical/source-claims.jsonl) use
relation registry 22's `lexeme_in_language`, `lexical_form_of` and
`lexical_sense_of`. All carry separate statement language/script, relation
basis, attestation scope and source-reading evidence. They are expressly
`linguistic_analysis`, `unreviewed`, with no admission. This is the agent's
qualified analysis, not a dictionary's report. Concrete domain/range and
evidence requirements apply; opposite polarities and competing assignments
may coexist. No transitivity or one-form/one-sense cardinality is imposed.
Four separate Claim statement forms preserve each complete qualified Claim.

Exact creation retries for all five subjects and the Claim package returned
their original receipts under the final identity guard. Their source records,
versions and original requests were not rewritten. Later form/record
correction uses the same existing separately delegated transaction and retained
history, not a profile-specific writer. No real lexical record correction or
linguistic admission was needed or claimed in this slice.

## Native identity boundary

Independent helper review found that a standalone record could reuse an ID
already held by a native semantic annotation, including a nonpublic one.
The correction reserves the understood native v2 entity namespaces without
projecting restricted text. The guard is in shared profile validation, so
protected form, revision and assessment readers cannot bypass it by parsing
the metadata themselves. Current identity is also checked on historical retry.

Creation binds the complete native metadata inventory. Existing-record
commands consult it only for native v2 namespaces: occurrence, lexeme, sense,
sign and concept. Unrelated Document/Letter reads preserve their no-corpus-
neighbor-discovery boundary. Native membership and bytes are rechecked through
the protected file reader. An opaque inventory fingerprint enters prepared
creation/revision, form configuration and assessment snapshots; native paths
and raw digests do not enter public profile inputs or exposed source contracts.
The local inventory refuses unsupported schemas, more than 1024 packets,
more than 1 MiB per packet or more than 8 MiB in total. This bounded scan is
not an indexed corpus-wide identity service or a claim of finished scaling.

Historical request/receipt bytes remain historical evidence. They are not
rewritten or compared to today's dependency fingerprint on exact retry.
Current owner scope and the current identity guard still apply. A future
native namespace expansion requires an explicit adapter transition.

The helper's final read-only review found no remaining actionable issue in
these boundaries and independently passed the late-ID and no-corpus-crawl
tests (2 tests, 6.288 seconds). It did not establish historical or linguistic
truth, agent competence, full-suite success or public availability.

## Validation and real reader

Root automated results:

- Source commands: 39 tests, 345.700 seconds; this includes creation/correction
  for all three lexical profiles, immutable form identity, retained unknown
  fields, exact replay, scope, concurrency and process-loss checks.
- Source revisions: 25 tests, 38.040 seconds after correcting existing
  owner-path error precedence; the wrong profile is rejected as a permission
  error before its path adapter is selected. The focused check also passed.
- Assessment: 62 tests, 11.628 seconds, including actual owner-snapshot
  invalidation on native packet arrival without exposing that packet's path.
- Human forms: 22 tests, 0.235 seconds.
- Source bibliographic graph: 81 tests, 269.503 seconds after rebuilding
  source catalog and projections. Synthetic competing lexical readings,
  exact fields, both carriers, semantic property filters and reverse focus
  pass without treating artificial fixtures as linguistic evidence.
- Script topology: 10 tests, 1.925 seconds. Source-foundation, bibliographic
  graph and source-home validators passed; available source bytes were
  fixity-checked. No validator grants semantic or rights acceptance.

The final real reader probe used snapshot
`8928c3078764fdd59c0803f306bd9a3d038a474157f2377ab0e6405b180f9774`.
Both source-claims and source-navigation carriers retain each complete source
record with one subject identity, Russian/English names and exact qualified
Russian hover context. Form identity and semantic content stay in mandatory
context; semantic property IDs select the expected referents. Four complete
unreviewed Claims expose exact Russian statements and their full context.

| Local operation | First call | Repeated call |
| --- | ---: | ---: |
| Graph load, cold process | 21.832 s | not measured here |
| Catalog after graph load | 6.358 s | <0.001 s |
| Search for Wille, lexeme kind | 4.155 s | 0.155 s |
| Bounded shared-identity inspection | 1.209 s | <0.001 s |

Eight bidirectional depth-2 focus calls took 0.260–1.460 seconds and returned
5–11 source carriers with 4–13 relations; each center had one shared scene
vertex. Peak process RSS was 2,172,588 KiB with the graph and caches retained.
These are observations, not accepted latency/memory budgets. Catalog was not
measured in its own cold process. Later note/index generation changes the
snapshot; it does not change this recorded measurement retrospectively.

Reproduce narrow automated checks from the repository root:

```bash
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_revisions.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_knowledge_assessment.py -v
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py -v
python scripts/build_source_witness_catalog.py --check
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
python scripts/build_tos_corpus_index.py --check
python scripts/validate_tos_corpus_index.py
```

Read-only consumption uses `ToSAccessCore.discover(root)`,
`knowledge_search('Wille', kind_ids=['lexeme'], limit=10)`,
`knowledge_node('tos.lexeme.german-wille', relation_limit=10)` and
`knowledge_focus('tos.sense.willing-jgb19', depth=2)`. Reverse focus from the
lexeme exposes the same proposed sense/form/language Claims, not inferred
dictionary facts. The API needs no lexical-profile-specific screen.

## Checklist and continuation

Review checklist: yes for source return, source/derived separation, lexical
and philosophical layer distinction, scope, identity and language continuity,
plurality, unknown fields, exact history and ToS/AoA boundaries. No rights,
publication, canon, competence or admission authority was acquired by these
validators or by the new profiles. Counterpart, compost and golden-kernel
promotion checks are not applicable to this slice.

Next source owner: native TextLayer/TextUnit/Anchor and exact Occurrence
creation. A written form or paragraph locator is not an anchored occurrence.
The existing private-source posture must remain intact; a new exact unit does
not reclassify source rights. L02, L03, calibrated assessment, complete human
forms, indexed processing and growing-data budgets remain open. Worker/D1,
real UI interaction/smoothness, CI, merge and deployment were not verified by
this slice. Reader rollback must retain the new sources and their history.
