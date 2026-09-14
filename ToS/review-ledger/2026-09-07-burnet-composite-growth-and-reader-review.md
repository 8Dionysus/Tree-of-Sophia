# Scholarly composite growth and shared-reader review

Date: 2026-09-07. Integration reviewer: `agent:codex-tos-foundation`.
This is implementation and source-boundary review, not independent content
assessment, source-text admission, publication or Foundation v1 completion.

## Owner change and real source path

The existing composite type now declares an explicit compatible descriptive
profile alongside the retained native witness adapter. See
[TOS-D-0054](../../docs/decisions/TOS-D-0054-compatible-scholarly-composite-record-shapes.md),
the [source reading](2026-09-07-burnet-editorial-composition-source-reading.md)
and the [ordinary profile contract](../doctrine/semantic-interchange/README.md#descriptive-composite-growth-alongside-native-witnesses).

The source command created one modern editorial composition,
`tos.composite.burnet.parmenides-arrangement-1908`, under the existing
scholarly-composites owner. It retains a composition account, method, coverage,
referent criterion and Russian/English names. Its three source-copy forms
are ready without granting admission. The separately delegated Claim command
created four qualified statements with source-bound statement forms:

- reconstructed target: the existing Parmenides poem;
- intellectual container: the existing Burnet book;
- included passage: the existing English presentation of (8);
- contextual compiler: the existing John Burnet Agent.

The original ten `composite-witness.json` records are byte-unchanged. No native
record was upgraded, duplicated, recast as a physical artifact or supplied with
invented common metadata. Native form/assessment support remains an explicit
separate route. The new composition is not the ancient original or the book.

## Observed execution and verification

Ordinary preparation, source creation and exact retry took 6.797 s for the
subject; Claim creation and retry took 6.555 s. The retained per-package
request, environment, provenance and receipt bind their exact created bytes.
The capture records serialization, not upstream reading or model execution.
All four subsequent Claim statement forms were ready and unadmitted.

The real shared-reader probe preserved the complete new record in both
source-navigation and source-claims carriers and collapsed them to one scene
vertex. All four qualified Claim bodies and their mandatory reading context
were retained. Russian hover and primary names, English alternate name and
source scope/content were checked; no date was inferred from the label.
Each discovered `tos.property.composite-*` content property selected exactly
the new record by its source value. The explicit assessment input adapter
resolved nine exact records, including all endpoints, without invoking an
assessment. Burnet is one origin, not four independent corroborations.

Measured local snapshot before this closeout note:
`338283603e999da1c12ed95f6a0af1b70169c8fde22533585852200c3a38b4d8`.
Cold graph construction: 26.328 s; peak RSS: 1,284,960 KiB. Depth 2, limits
80 nodes / 150 relations:

| Focus | Nodes / relations | Seconds |
| --- | --- | --- |
| editorial composition | 11 / 13 | 0.304 |
| Parmenides poem | 13 / 13 | 0.308 |
| Burnet book | 10 / 10 | 0.310 |
| quoting passage (8) | 10 / 10 | 1.336 |
| John Burnet | 8 / 7 | 0.305 |

These are observations of this local reader, not latency guarantees, cold-cost
acceptance, UI smoothness measurements or Cloudflare/D1 evidence.

Checks completed:

```bash
PYTHONPATH=tests python -m unittest test_source_witness_bibliographic_graph
PYTHONPATH=tests:mechanics/growth-cycle/tests python -m unittest test_source_commands test_knowledge_assessment
PYTHONPATH=access/src python -m unittest discover -s access/tests -p test_knowledge_contract.py
python scripts/validate_source_witness_foundation.py
python scripts/validate_tos_source_home.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```

Results: 76 graph tests (241.452 s), 86 command/assessment tests (226.362 s),
59 access tests (35.680 s). The extended native-old-registry and dual-format
checks additionally passed together (2 tests, 4.140 s). The full descriptive
creation/correction matrix also passed independently (339.828 s), including
source-version inspection, unknown fields, property queries and current
permission checks on retry. Source and decision validators passed.

Red checks first exposed missing profile support, misplaced command/assessment
inputs, a forged legacy schema and a forged native schema ref under the old
registry. Each was rejected after the corresponding guard change. An earlier
full graph run saw generated-parity failures while companions were stale;
the final run followed the owner rebuild and passed. It is not counted as
semantic failure or suppressed by weakening the parity checks.

## Manual boundary review and remaining work

Yes: source return, authored/derived distinction, persistent identity,
editorial mediation, uncertainty and language provenance remain explicit.
Reconstruction, inclusion, quoting passage and compiler are separately typed
Claims with concrete domains/ranges, inverse readings and required evidence.
Neither competing compilers nor competing reconstructions are excluded by
an artificial one-answer cardinality. Source/schema changes cannot silently
replace the retained adapter; source correction cannot change referent scope.

No canon, rights, personal consent, external publication, deployment or new
runtime authority was granted. No foreign work was changed. Counterpart,
lived-witness, compost and gold-admission routes are not applicable here.
The public source reading remains provisional: this review does not endorse
Burnet's textual choices or make the Russian/English descriptions assessed.

Next content work remains source-visible assessment and exact ancient quotation
and edition comparison. Native witness forms/assessment and whole-corpus
migration remain separate incomplete work. UI consumption and whole-foundation
performance/CI/landing are not established by this slice. Reader rollback
preserves both source formats, the new records and their history.
