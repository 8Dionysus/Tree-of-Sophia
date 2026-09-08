# Scoped translatability: source growth and common-reader review

Date: 2026-09-08. This completes an executable translatability value slice,
not Foundation L03, a full lexical profile, calibrated linguistic assessment,
UI acceptance, research admission, CI, merge or deployment.

## Contract and operation

Entity registry 26 and relation registry 25 declare `lexical_translatability`
with concrete Lexeme, LexicalSense and native-bound Occurrence domains, and
one Claim-scoped literal range. The
[contract](../contracts/source-lexical-translatability-claim.schema.json) and
[owner explanation](../doctrine/semantic-interchange/README.md#scoped-lexical-translatability)
separate adequacy for a task, transfer of specified aspects, language-marked
candidate wording and the presence/outcome of a reported search. Thirteen
scalar properties use the existing semantic catalog and lens filters.

Neither no matching search result nor an inadequate rendering asserts
universal untranslatability. A report needs its sought criterion and coverage;
partial renderings can coexist with a search for a fuller one. Partial transfer
may be adequate for a limited task. The enclosing Claim's polarity, uncertainty,
review and assessment status remain independent. The schema refuses absolute
`untranslatable`, a target subject ID instead of the typed value, missing
scope, unqualified candidate strings and incompatible domains or layers.

No new production Python dispatch, universal translation ontology, tool call,
identity merge or per-kind UI screen was added. The v3 exact-value grant still
owns creation and correction; source copy, retained history, conflict handling
and current-scope replay retain their existing contracts. The complete Claim
is mandatory context for its statement form. Its value is independently
focusable through that Claim, not an independently minted lexical identity.

Synthetic tests cover all axes, explicit unknowns, independent wording
languages/scripts, equal values in separate Claims, negative/unknown polarity,
zero confidence, unknown extensions and no guessed references or chronology.
They exercise named filters, inspection, reverse focus, command creation,
correction, exact predecessor bytes and source-copy context. They do not prove
any real language judgment.

## Real source and exact growth

The [Goethe source reading](2026-09-08-goethe-translatability-source-reading.md)
records authorship, source scope, the protected text-observation digest and
its limits. The retained observation is a web reader's returned text, not an
original HTTP-byte capture. An independent helper assignment inspected the
mapping but read the same article; this is not independent evidence or a
competence receipt. Its date-scope wording suggestion was applied before any
source creation binding; this packet makes no publication-date assertion.

- [Leitkultur Lexeme](../source-witnesses/lexical-descriptions/sid-2e581c085b6c4d6ea882268c1321a27b/lexeme.json):
  `tos.lexeme.sid-2e581c085b6c4d6ea882268c1321a27b`.
- [Scoped Claim](../source-witnesses/relations/goethe-leitkultur-translatability/source-claims.jsonl):
  `tos.claim.sid-959199367b07454c8479a5a625ab8261`.

The Claim reports the article's limited adequacy judgment about two English
renderings in its described German political context. It remains
`scholarly_report`, `inferred`, `unreviewed` and public-metadata-only;
`search_report=null`. Neither target Sense identities nor historical tokens,
universal impossibility or a performed search are invented. Russian descriptions
and source-copy statements are explicitly original research wording, not
certified translations of the article. Lexeme label and candidate expressions
retain German and English declarations respectively.

Three existing source-owner commands created the subject, Claim and its
statement form, yielding three forms total, in 17.567316 seconds. A fresh
process replayed all three with their original receipts in 2.309051 seconds.
Exact source records and the protected reader capture were unchanged. Public
source packages retain their technical serialization provenance; upstream
reading, model reasoning and content quality are not proved by that receipt.

## Actual local reader

The probe used the normal `ToSAccessCore.discover(root)`, current corpus index
and source-backed bibliographic graph, not a separate demonstration store.
An isolated full-detail depth-0 lens selected a ready Russian hover or statement
packet at the exact subject identity. The entire mandatory context matched;
the packets remained non-standalone and granted no admission.

```json
{
  "schema_version": "tos_lens_spec_v1",
  "lens_id": "goethe-exact-reading",
  "seed": {"focus_node_id": "tos.claim.sid-959199367b07454c8479a5a625ab8261"},
  "traversal": {"depth": 0},
  "detail": "full",
  "language": "ru"
}
```

The same core filtered the real value by rendering judgment, aspect transfer
and target language. The no-result-search filter did not match this source's
absent search report. Focusing the value reached its Claim and lexical subject;
three cursor pages reached those exact objects without repeating primary
carriers. The value's own packet retained the enclosing Claim context.

Observed snapshot:
`60791dc4ea5d05ff32a0566e5d43fe381dffe8ed6b3f36df3e0f7bcc52aae06d`.
Later documentation-index regeneration may change the snapshot without
changing these historical measurements.

| Operation in one fresh local process | Observed result |
| --- | --- |
| Cold shared graph | 28.725710 s |
| Exact metadata / Claim full-detail lens | 0.351803 s / 0.351276 s |
| Value-to-Claim-to-subject focus | 0.280610 s |
| First / repeated lexical search | 4.875485 s / 0.032742 s |
| Depth-3 exploration, 2 primary nodes / 3 relations per page | 3 pages, 5 unique primary carriers |
| Work per exploration page | 6, 8, 6 units |
| Peak RSS | 2,171,544 KiB |

These are bounded local observations, not p95, accepted resource budgets,
scaled-data performance, persistent runtime health or UI smoothness.
Continuation may repeat context endpoints, never primary discoveries.

## Validation and remaining review

- New command slice: one focused test passed, 10.865 seconds. Its first two
  runs exposed only incorrect fixture assumptions (missing required wording
  language declarations; nested packet assumed where materializations are
  already direct packets). Production code was not changed to satisfy them.
- Full source Claim command module: 28 tests passed, 288.675 seconds.
- New source-profile/graph slice: one focused test passed, 1.264 seconds.
- The first full graph run found 19 stale-generated-snapshot failures/errors
  among 83 tests after the source registry change, all from exact rebuild
  mismatch. After rebuilding and validating the generated graph, the full
  rerun passed all 83 tests in 262.437 seconds. No expectation was weakened.
- Source catalog, source-foundation validator, bibliographic graph and corpus
  index builders/validators passed. Optional absent payloads were not supplied
  or reclassified as available. The full source-foundation suite ran 98 tests
  in 110.387 seconds: 97 passed, one optional private-payload availability
  check skipped. Source-home validation passed. Documentation/corpus-index
  tests passed: 44 tests in 35.075 seconds. Generated corpus/documentation
  currentness and all 56 nested agent-card checks also passed.

Focused reproduction:

```bash
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_claim_commands.py
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python -m unittest discover -s tests -p test_source_witness_foundation.py
python scripts/build_source_witness_catalog.py --check
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
python scripts/build_tos_corpus_index.py --check
python scripts/validate_tos_corpus_index.py
```

Review checklist: yes for source return, authored/derived distinction,
Claim/value/subject separation, retained lineage, scoped language/context,
plurality, no source instructions as authority and ToS/AoA boundaries.
Publication, rights clearance, canon and calibrated competence are not inferred.
The public derivative omits protected capture contents, configuration paths,
credentials and operator evidence; direct private-path checks found no matches.

The separate UI owner has accepted the full-packet consumption contract and
is implementing existing card/reader/tooltip seams in its own worktree.
No UI file, camera, gesture, scene composition or active UI process changed in
this source slice. Actual UI interaction and D1 parity are unverified here.
Competent source-visible assessment of this report and broader L03 completion
remain with the lexical and assessment owners; ready forms are not acceptance.
