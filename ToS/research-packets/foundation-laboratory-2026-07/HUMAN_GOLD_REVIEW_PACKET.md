# Fifteen-Page Human Gold Review Packet

Status: blind source-visible interface materialized and mechanically verified;
one rare calibration observation preserved without completion attestation;
no double check, OCR quality result, gold acceptance, or remaining human debt

Materialization date: 2026-07-23

Successor note, 2026-07-26: this document records the frozen v1 interface and
its original two-reviewer posture. New assurance claims are governed
additively by `ToS/contracts/manual-gold-assurance.schema.json`,
`gold-assurance.v2.json`, and `HUMAN_ASSURANCE_RESEARCH.md`. The current
solo+AI route uses criteria review by default; a delayed, pass-1-hidden
same-human recheck is intra-annotator stability only, and German textual
review remains language-competence-blocked. Packet cardinality is not
scheduled human work.

Closure update, 2026-07-28: the mutable Workbench session preserved
`tos-sample-antonovsky-p011` as one rare independent calibration observation
and closed with `attestation_status: not-collected`,
`promotion_authorized: false`, and `human_debt_units: 0`. The other fourteen
units are explicitly unscheduled. The source autosave remained byte-identical
at SHA-256
`6ba7619bf4591353fb8d5afea75a92d0e2ddd705cbb5adb37885c883cc4a1635`;
the private closure and receipt digests are respectively
`e0ff950043eb823b7894ef3e4ee958f5292b111dc7d9f7d408dc9a3620b848da`
and
`bc655fe8129df8b30bdeb304a2eec0de02723eaf4a8e984ecf5ebac486f0f355`.

## Question

Can the fifteen output-blind OCR/structure gold candidates frozen before A/B/C
execution be presented to real reviewers with exact source identity, adjacent
page context, two independent passes, and no contestant or recognized-
translation contamination?

The answer is **yes for interface readiness only**. The packet makes the next
human action concrete and reproducible. It does not perform that action and
does not change `gold-status.json`: all fifteen units remain candidates.

## Frozen input closure

The packet reads the tracked `gold-status.json`, `ocr-visual-samples.json`, and
the private 36-page frozen render manifest. It resolves five candidates from
each source:

- Antonovsky 2007 Russian PDF;
- *Mysl* 1996 Russian collected-works PDF;
- Naumann 1893 German parent scan.

Each review unit exposes the previous, current, and next page from the same
digest-verified PDF. Adjacent candidate triplets share pages, so the exact
fifteen-unit closure contains 41 unique 300 DPI RGB PNGs. Every current page
must remain byte-identical to the corresponding frozen OCR contestant input.
The fifteen center pages total 7,549,837 bytes; all fifteen hashes matched.

## Private packet closure

The host-owned packet is:

```text
/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/
  shared-inputs/tos-human-gold-foundation-v1/
    zarathustra-human-gold-review-v1-20260723/
```

| Evidence | Value |
| --- | --- |
| packet status | `awaiting-real-human-double-check` |
| packet files / bytes | 46 / 19,272,534 |
| review units | 15: 5 per source |
| rendered pages | 41 unique 300 DPI RGB PNGs |
| frozen center pages | 15; 0 hash mismatches |
| frozen OCR render-set SHA-256 | `031c036b72904006d1cd1c885b843833d8dcc20b7aa25cbfb316aa49bcdbaef4` |
| review page-set SHA-256 | `0f873aeb24c564a247f9fc33a9234751cbe8bf7adf3b338814492f9517f053af` |
| manifest SHA-256 | `a40f3d244fce19782c0c983634e37f252c36fdd6973def4ac01a3a633820c23d` |
| materialization receipt SHA-256 | `10d5421110d242c48abd8d2f6618a15177e99b0520a3383a43fe520d5940eeee` |
| pass 1 workbook SHA-256 | `8c1debadfe243097eff09ccb324a0d63f09015d9eb59ae99d231d68433fc98c8` |
| pass 2 workbook SHA-256 | `4e1815e425e02b2e05f3aa99dca0b0c4f4e8da87057435086803487a3a166ca4` |
| blank review JSONL SHA-256 | `7ceeaf6ddb7d9443ef5ffba7db1848e2e606a0e562d763a7b578b3b99c508d07` |

Both local HTML workbooks expose page triplets and export drafts only. Pass 1
records diplomatic transcription, layout, reading order, legibility,
uncertain glyphs, source ambiguity, and human minutes. Pass 2 requires a
different reviewer, keeps pass 1 hidden until both drafts are frozen, repeats
the transcription independently, and checks punctuation, case, hyphenation,
page boundaries, lineation, reading order, page furniture, and unresolved
glyphs.

No OCR output, embedded/reference OCR, model answer, or recognized translation
is present in either workbook or in the blank JSONL.

## Working human session

A separate mutable copy was prepared without editing the immutable packet:

```text
/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/
  human-review/zarathustra-15-page-gold-pass-1-20260723/
```

It contains `START_HERE.md`, `review-session.json`, an exact blank working
JSONL, and a mechanical-readiness receipt. The working JSONL still has:

- pass 1 completed: 0 / 15;
- pass 2 completed: 0 / 15;
- adjudicated: 0 / 15;
- accepted human gold: 0 / 15.

The readiness-receipt SHA-256 is
`dd7106561a9115c60679457d9a2fc2e6ae3e0cb2a47f17262aa35d1e3dd5e42f`.
This receipt records interface state, not a human act.

## Independent checks beyond a green materializer

The following observations were made separately:

1. the verifier re-read Tree-owned inputs and reported exactly 15 blind
   candidates and 41 context pages;
2. all 15 center-page digests, byte counts, and dimensions matched the frozen
   OCR inputs;
3. both workbooks contained exactly 15 units in the same order as the blank
   JSONL, referenced 41 existing page assets, and had no missing assets;
4. the working copy was byte-identical to the immutable blank template and
   contained zero human identities, attestations, transcriptions,
   adjudications, or accepted gold states;
5. Antonovsky page 11, *Mysl* page 4, and Naumann parent-scan page 44 were
   opened visually. They exposed respectively clean Russian body text,
   bibliographic front matter, and a noisy bordered German scan with a
   digitization watermark;
6. the gate exited `2` / `blocked` with the exact reasons
   `real_human_pass_1_incomplete:15`,
   `independent_real_human_pass_2_incomplete:15`, and
   `human_gold_acceptance_incomplete:15`;
7. host storage and resource validators passed 19 / 19 and 16 / 16 checks.

The stack verifier was strengthened after this inspection. It reconstructs
the deterministic blind workbooks and exact blank records, requires the exact
source-file and 41-page sets, closes every current page over the frozen render
identity, and rejects swapped source-page references. An `accept-with-limits`
or revised second pass cannot become accepted gold without an error-ledger
reference. These contracts are committed in `abyss-stack` as `8bc3758`.

## Resource evidence

The actual render ran through `abyss-machine resource launch` as a bounded
`medium/generic` service with `force=false`. It completed successfully in
27.982 seconds, used 27.553 CPU seconds, peaked at 307.594 MiB, and used no
swap. Owner admission reported no blocked or denied reasons. The thermal owner
retained its real law: 100–105 °C is monitored active range, above 105 °C is
watch, 106 °C is hot, and 109 °C is critical.

## Authority and next move

Mechanical closure can prove source identity, ordering, blindness, file
fixity, and declared review shape. It cannot prove that a reviewer is human or
that a transcription is correct.

The legacy full-packet exact-reference experiment remains available only if a
future research question explicitly justifies its cost. Its next admissible
evidence would then be:

1. a real human completes all fifteen pass-1 units against the visible pages;
2. a different real human independently completes pass 2 without viewing pass
   1 during review;
3. discrepancies are adjudicated, limitations/revisions enter the manual error
   ledger, and each accepted record receives an explicit final diplomatic
   transcription;
4. only then may OCR CER/WER, structure quality, reading order, and human
   correction time be manually measured and compared.

That optional route is not an operator backlog. Current solo+AI human work is
opened only by a new source or method calibration, serious candidate
disagreement, unresolved or semantic risk, a translation question within
declared competence, or periodic drift control. UI regression uses synthetic
fixtures or one separately declared control episode. Exact-reference metrics
that still require fifteen adjudicated units remain unmeasured rather than
being converted into fourteen repetitive tasks.
