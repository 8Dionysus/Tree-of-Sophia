# Zarathustra concept workbench v1

This route compiles the complete four-part German/Russian technical corpus into
one reusable candidate workbench and prepares English translation candidates
on demand. A request supplies mutable labels, lexical
seeds, explicitly weaker semantic-neighbor seeds, distinctions, negative
controls, and an allowed relation vocabulary. The builder returns a complete
request-local dossier without accepting a concept.

The workbench keeps five contexts separate:

1. witness/alignment owns exact text, anchors, order, shapes, and gaps;
2. lexical/morphology owns observed forms and reversible family proposals;
3. speaker/context owns discourse-role candidates and bounded source return;
4. the request owns its search scope and negative controls;
5. the candidate graph is a derived navigation surface.

The three languages do not have equal authority. German is the source witness;
Antonovsky's Russian is a historical translation witness and a useful
contrast; English is generated per selected German occurrence as an
unreviewed request-local candidate. There is intentionally no pretranslated
English witness hiding behind the workbench.

Every selected German occurrence now receives one text-free tracked English
task. Its mode-`0600` companion contains the exact barrier-bounded German
context and any aligned Russian comparison context. The eventual English
candidate must provide four simultaneous views: literal gloss, contextual
translation, semantic alternatives, and an explicit untranslatability note.

The required analysis order is morphology -> syntax -> historical sense ->
sourced etymology -> recurrence inside *Zarathustra* -> Russian witness
comparison -> English rendering. An etymological claim without an external
citation is schema-invalid; model memory is not evidence, existing English
translations remain sealed until the candidate is frozen, and etymology may
suggest a translation consequence but never dictate Nietzsche's meaning.

Exact DE/RU text, readable all-form indexes, token occurrences, and context
windows remain in ignored mode-`0600` artifacts. Tracked outputs use opaque
identities, source addresses, text digests, counts, statuses, and typed
candidate relations.

`speaker-state-candidates.v1.jsonl` covers every witness-local prose paragraph
and every verse line. German and Russian states are proposed independently;
paragraph alignment only compares them later. The state machine records
explicit speech cues where available and otherwise keeps
`zarathustra_or_external_narrator` unresolved. A green build therefore proves
coverage, not correct literary attribution.

The graph uses a concept hub. Occurrences are not joined into a complete
pairwise clique. Passage-to-passage edges are limited to source order and
same-reading recurrence; stronger relations such as opposition, qualification,
or metaphor require a later source-visible semantic pass.

## Multilingual concept search back to German

The query adapter resolves a Russian, German, or English discovery label to a
stable **navigation route**, then follows the current request-local concept
candidate and its typed realization relations back to exact German witness
occurrences. The route ID is stable across request versions because it is
bound to the opaque request identity key; it is explicitly not an accepted
`concept_id`. Mutable labels therefore aid discovery without defining semantic
identity.

For example, the Russian genitive form `судьбы` is recognized as a reversible
morphology candidate for the discovery label `судьба` and returns the German
source cards in work order:

```bash
python scripts/query_zarathustra_concept_workbench_v1.py \
  --query 'судьбы' --language ru --limit 10
```

Each card contains the exact German surface and context, part, reading and
text-unit anchors, speaker status, aligned Antonovsky context where available,
the candidate realization relation, and the prepared source-first English
task. Exact source text is emitted only at local runtime from the ignored
mode-`0600` index and is not written to tracked search output.

The default result set contains only the direct German lexical/morphological
core. Semantic-neighbor expressions remain visibly separate and require an
explicit expansion:

```bash
python scripts/query_zarathustra_concept_workbench_v1.py \
  --query 'судьба' --language ru --include-semantic-neighbors --limit 10
```

Searching a semantic-neighbor alias also resolves only with `ambiguous`
status. Negative controls such as lowercase German `los` do not resolve to the
concept route. Russian remains a historical translation comparator and
English a generated on-demand candidate; neither becomes German source
authority through search.

## On-demand word analysis

One search result can now be compiled into a source-bound task for an agent:

```bash
python scripts/prepare_zarathustra_word_analysis_v1.py \
  --query 'судьбы' --language ru --rank 1
```

The task contains the exact German form and barrier-bounded context, stable
occurrence and anchor references, the aligned Antonovsky comparison, speaker
status, source and context digests, and a fixed analysis order: morphology,
syntax, historical sense, sourced etymology, contextual semantics, recurrence,
Russian-witness comparison, and English rendering. The lemma remains a
candidate. Etymology requires a point citation, may inform a translation
choice, and cannot determine contextual meaning or authorial intent.

An agent can return an `english-translation-candidate.v1.schema.json` object
and check both its schema and its binding to the prepared occurrence:

```bash
python scripts/prepare_zarathustra_word_analysis_v1.py \
  --query 'судьбы' --language ru --rank 1 \
  --validate-candidate /absolute/path/to/candidate.json
```

The compiler writes nothing. Exact text remains local and excluded from the
public standalone bundle; a valid candidate is still AI-generated,
unreviewed, non-semantic, non-graph, and non-canonical.

The first request, `fate` v2, is a proof that low-frequency inflections and verse
are no longer lost: morphology expansion runs over every observed form,
including frequency-one historical Russian forms and German compounds.
Lowercase `los` is retained as a hard negative, while `Verhängniss` and other
neighboring expressions remain a separate semantic-candidate tier and never
increase the direct-mention count.

The German lexical source item also contains 1,796 tokens from the appended
`Dionysos-Dithyramben`. They remain visible in the source-item census but are
excluded from the Zarathustra work scope. In particular, three apparent extra
`Schicksal` occurrences belong to that separate work; the German direct core
inside Zarathustra is therefore 26 occurrences, not 29.

Build and verify:

```bash
python scripts/build_zarathustra_concept_workbench_v1.py --build --issue-identities
python scripts/build_zarathustra_concept_workbench_v1.py --check
python -m unittest tests.test_zarathustra_concept_workbench_v1
```

Run another schema-valid request without changing the builder:

```bash
python scripts/build_zarathustra_concept_workbench_v1.py --preview \
  --request /absolute/path/to/concept-request.v2.json
python scripts/build_zarathustra_concept_workbench_v1.py --build --issue-identities \
  --request /absolute/path/to/concept-request.v2.json
python scripts/build_zarathustra_concept_workbench_v1.py --check \
  --request /absolute/path/to/concept-request.v2.json
```

Non-default requests receive their own
`outputs/<request-key>-v<version>-<full-opaque-identity>/` route and private
analysis database. Request identity is versioned independently of its mutable
label; supersession is explicit. Request-local form, occurrence, relation, and
English-task IDs include that opaque identity/version binding, so two concept requests
cannot silently share local objects.

The builder validates both request and relation outputs. Four structural edge
types keep every occurrence connected to both its form and the concept hub;
the request's optional allowlist controls translation, speaker, reprise, and
sequence edges. Graph nodes are projected only when referenced by an emitted
edge, so a narrow request cannot create dangling or isolated nodes.

If every declared seed is absent, the request remains a valid negative result:
the graph contains only its concept candidate and sets `empty_result: true`.
The no-isolated-node invariant applies to evidence-bearing form, occurrence,
and speaker nodes; absence is recorded rather than padded with invented edges.
