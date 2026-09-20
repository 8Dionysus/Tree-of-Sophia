# Zarathustra reading workbench v1

A whole-work, source-returning **candidate** reading layer. German remains the
source authority; Antonovsky 1911 is a historical comparator. English is an
on-demand source-bound analysis task, not an additional hidden witness.

The layer connects existing multilingual concept retrieval to current
sentence/clause anchors, quotation scopes, contextual voices and repeating
lexical sequences. It preserves the previous linguistic-analysis spine rather
than rewriting its identities or claiming its heuristic morphology is repaired.

## Read and run

The source-bound chapter policy is a separately retained candidate data input
covering 81 chapter records. Historical agent review and independent challenger
receipts remain with the old revision; they do not attest this recovery build.
The generated manifest, coverage receipt, reading census, quote ledger and gap
ledger belong to the explicitly selected private dataset, not this source tree.

```bash
python scripts/build_zarathustra_reading_workbench_v1.py --build --source-root /path/to/private-source-root --output-root /path/to/new-reading-data
python scripts/build_zarathustra_reading_workbench_v1.py --check --source-root /path/to/private-source-root --output-root /path/to/reading-data
python scripts/build_zarathustra_reading_workbench_v1.py --validate-tracked --source-root /path/to/private-source-root --output-root /path/to/reading-data
python scripts/query_zarathustra_reading_workbench_v1.py --query судьбы --language ru --limit 100 --group-by speaker,formula --source-root /path/to/private-source-root --analysis-root /path/to/reading-data
```

The source and analysis roots are explicit data selections. The builder requires a
separate output root outside both the software checkout and the input dataset; it
never overwrites an existing reading output. The query requires both roots and
never selects executable code from either one. The database is mode 0600. No
production/public fallback reads another checkout. See [query contract](QUERY_CONTRACT.md) for the
shared local CLI, HTTP and native MCP operation and availability envelope.
Browser/WebMCP and public Worker integration are outside this recovery port.

## Method developed against actual reading failures

1. Preserve exact contexts, sentences, surfaces and original IDs. All new
   offsets are half-open **context-local Unicode code points**, never relabelled
   XML/text-node offsets or browser UTF-16 indices.
2. Inventory every chapter in both witnesses, then inspect opening/closing and
   reporting transitions. Bind chapter/range/span policies to exact source
   hashes. A policy identifies a contextual candidate, not accepted authorship.
3. Scan quotations per reading. Expected closers win over opener glyphs;
   continuation signs do not increase depth. Source-visible OCR/glyph anomalies
   get explicit candidate events; no correction changes the source bytes.
4. Keep reporting clauses, outer utterer, attributed quoted voice, performed
   persona and hypothetical modality distinct. Missing antecedents and quote
   scopes remain ambiguous. A chapter baseline is not a universal coreference
   solution; explicit cues and source-bound overrides refine it.
5. Detect repeated lexical sequences of 4–32 surface tokens, preserving all
   source spans. Suppress redundant nested sequences only when their complete
   occurrence sets agree. Link consecutive returns, not every pair of passages.
   Whitespace-separated letters in the Russian OCR are quality-deferred;
   sentence-crossing sequences are lexical repetitions, not syntactic phrases.
6. Return concept hits through an independently checked occurrence crosswalk.
   Enrich with containing sentence, clause, speech candidates and overlap-based
   formula membership; neighboring formulas are not falsely labelled word
   memberships. Counts distinguish the full match set from the returned page.
   Explicit German printed hyphenation (`¬` plus newline) can yield additional
   normalization candidates against already selected forms. These keep exact
   source spans and deletion operations; they have no invented legacy occurrence
   ID and are counted separately from the predecessor results and their groups.
7. Recheck source conservation, actual alignment anchors, negative examples and
   ordinary query/access paths. An independent Codex challenger checks different
   properties and source cases; agreement is review evidence, not semantic canon.

An exact algorithmic result is exact only for its declared operation. The
copied sentence/clause alignments retain their original candidate status and
verse exclusions. Historical morphology, first-verb dependency heuristics,
general pronoun coreference, synonym discovery and semantic near-variants are
**not** certified by this release. The Russian legacy occurrence-to-page/block
bridge is deferred; the complete Russian source-surface layer is still present.

Etymology requires external cited evidence. Contextual translation and wordplay
belong to an occurrence; a dictionary history does not establish Nietzsche's
intended meaning. The existing word-analysis task continues to enforce this
boundary. Preparing a task is not the execution of an English translation.

## Transfer to the rest of ToS

The reusable components are the immutable source-span contract, separately
versioned speaker policies, occurrence crosswalk, sparse recurrence families,
source-bound query enrichment and explicit coverage/gap accounting. German
quotation conventions and Antonovsky OCR policies stay witness-local; they are
not universal defaults for another work. Changes to evidence or a method require
regeneration and a new checked manifest, without silently accepting the output.

Focused tests live in `tests/test_zarathustra_discourse.py`,
`tests/test_zarathustra_recurring_formulas.py`,
`tests/test_zarathustra_reading_challenger.py`,
`tests/test_zarathustra_reading_workbench_v1.py`,
`tests/test_zarathustra_reading_query_v1.py` and `access/tests/test_reading_access.py`.
Full source checks run only where the private material is installed; public CI
checks the software and synthetic contracts without installing private data.
