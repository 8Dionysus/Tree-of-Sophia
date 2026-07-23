# Translation Source Review v2 Materialization

Status: blind source-visible review interface materialized and mechanically
verified; no human review, German transcription, translation, or source
acceptance completed

Materialization date: 2026-07-23

## Question

Can the rejected v1 automatic selector be replaced with a review surface that
lets a real human determine layout, cross-page boundaries, and diplomatic
German directly from the source scan without seeing the v1 candidate or the
recognized Russian translation?

The answer is **yes for interface preparation only**. The packet closes the
mechanical and blindness prerequisites for human source review. It does not
answer any of the 30 source questions and cannot authorize a translation lane.

## Frozen input boundary

The Tree-owned `translation-source-review-plan.v2.json` was frozen before any
v2 candidate text. Its SHA-256 is
`ba04adba9787c91ffca017128db0250f5565663084ba7f60a609e227b88eaf25`.
It declares 30 new review-unit identities and 89 unique
previous/current/next-page renders. Every unit supersedes one v1 proposal
without reusing its candidate text.

The materializer verified all 30 declared EPUB member digests, but did not
extract or emit automatic German from them. The source-visible review pages
come from the exact 529-page Naumann 1893 image-container PDF with SHA-256
`61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf`.
The recognized Antonovsky witness remained sealed and its content was neither
consulted nor emitted.

## Private packet closure

The host-owned private packet is:

```text
/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/
  shared-inputs/tos-translation-foundation-v1/
    zarathustra-translation-source-review-v2-20260723/
```

| Evidence | Value |
| --- | --- |
| packet status | `awaiting-real-human-source-review` |
| packet files / bytes | 94 / 45,034,156 |
| review units | 30 |
| rendered pages | 89 unique 180 DPI RGB PNGs |
| page-set SHA-256 | `b4b0faa670693c4db07a1f6eed3bde3d9191c0cc24168139f39cdf64144ea038` |
| manifest SHA-256 | `8db6f88131355e858b861696899ae5edc37b238ec31493d898c559153010850c` |
| receipt SHA-256 | `8177a52bcde3ff6d46f57f991a355f76fa222baa3e1404e4eeabb4e5c234feba` |
| pass 1 workbook SHA-256 | `115194a16fee0bcd730b6785d775cd523b0a8513da81e100082a883518d7b458` |
| pass 2 workbook SHA-256 | `ef9094daaf18ce7ce1a9b8b9433790100cc02623fee869b9328f7e615ee1947f` |
| blank review JSONL SHA-256 | `cebb91dde2ae3c796d173734ea7d5bc095332bba15cacc72193700755a6b582a` |

Pass 1 exposes layout classification, prose-boundary selection, uncertain
glyphs, and diplomatic transcription. Pass 2 independently checks
transcription, punctuation, historical spelling, lineation, page furniture,
and both boundaries. Browser download produces a local draft only; it does not
submit a review or change acceptance state.

## Resource evidence

The actual host write ran through `abyss-machine resource launch` as a
`medium/indexing` job with `force=false`. Admission projected 8,574.284 MiB of
available memory after active reservations and sampled 77 C package
temperature. The first owner estimate was 96 MiB because rendering is strictly
sequential. The observed systemd peak was 142.961 MiB with zero swap and a
40.706-second service runtime. The runtime demand profile learned a
178.701-MiB future estimate from that observation; 96 MiB must not be reused as
the next-run estimate.

## Translation and semantic gates frozen after materialization

`translation-laboratory-plan.v1.json` now freezes the complete 17-stage route
from exact source anchor through preserved rejected alternatives. Its SHA-256
is `9a260fb4a21c44a67b56d2887ebba4d01d82c49c5d3008b39ed44d4779145ee4`.
The plan records the observed source state as 0 of 30 human-accepted units and
keeps human-only, AI-only, AI-alternatives, and AI+human lanes blocked. It also
records the resident-first model order, conditional downloads, CTranslate2 as
an unproved specialized runtime route, the external-source etymology law, all
19 translation evaluation axes, and a separate non-philological personal
read-aloud layer.

The companion `semantic-ladder-packet.schema.json` fixes the order exact form
-> concordance -> context -> morphology -> lemma -> section/work/author
recurrence -> translation correspondences -> sign candidate -> real-human
decision -> relations -> concepts -> competing readings -> graph. A packet
cannot exist without accepted source-review references. If the manual sign
stage is not `human-accepted`, every relation/concept/counterreading/graph
stage must remain `blocked` or `not-started`.

The stack-owned read-only `gate-translation-lab` was then run twice against
the real private packet:

| Input human evidence | Records found | Pass 1 | Pass 2 | Accepted German | Exit / decision |
| --- | ---: | ---: | ---: | ---: | --- |
| no review output | 0 | 0 | 0 | 0 | `2` / `blocked` |
| immutable blank template | 30 | 0 | 0 | 0 | `2` / `blocked` |

Both runs independently re-verified all interface fixity and comparator
sealing. The second proves that 30 schema-valid rows do not become human
evidence merely by existing. Only an explicit final `accept` counts toward the
30-unit source gate; `accept-with-limits`, uncertainty, rejection, or deferral
requires resolution or a superseding unit.

After the translation-reference research closed, the gate was strengthened
and run again against the immutable blank template. It independently validated
the 14-entry, nine-category reference register at SHA-256
`6e58c070f6c0b251c1607ca24be5482d5ee5634749c7e6bbda5f683764bd3758`:
zero admitted content, zero human bibliographic reviews, zero human rights
reviews, and zero permission requests. Reference research and interface
fixity passed; both human passes, all 30 source decisions, and pre-draft
analysis remained blocked; the command again exited `2`. Even a future 30/30
source acceptance now opens only source-grounded morphology, syntax,
historical sense, sourced etymology, recurrence, allusion, and literal
interlinear work. Human-only, AI-only, alternatives, AI+human, and comparator
lanes stay closed until that evidence is frozen.

## Independent manual checks

The following checks were performed separately from the green materializer
exit:

1. the packet verifier re-read all fixity and reported 30 blind units and 89
   source-page renders;
2. a physical file count found exactly 89 page PNGs and 94 packet files;
3. a set diff between every page named by the frozen triplets and every
   rendered-page manifest entry was empty;
4. the JSONL contained 30 unique units, zero human-pass attestations, zero
   source decisions, zero transcriptions, and no visible candidate or
   comparator state;
5. all 30 complete v1 candidate strings were searched as multiline fixed
   strings across both workbooks and the JSONL: zero matches;
6. both recognized-comparator identifiers were searched across the review
   surfaces: zero matches;
7. pages 56, 203, and 519 were opened visually and showed readable, correctly
   routed German scan pages from the beginning, middle, and end of the work;
8. the manifest's review-plan digest matched a fresh digest of the tracked
   plan.

An initial hand-written set-diff query failed because it assumed field names
that the real plan and manifest do not use. The failure was kept visible; the
objects were inspected and the corrected query produced the empty diff above.
This is another reason not to treat a validator or a shell exit as the observed
reality.

## Authority and next move

The packet is an interface, not review evidence. Its blank JSONL must be copied
to a new output before a real human records identity, time, source-visible
decisions, uncertainty, and both independent passes. No generated process may
set either human attestation or simulate the human-only lane.

Until those 30 source units are accepted by real human review:

- AI-only translation remains blocked;
- AI+human translation remains blocked;
- the recognized translation remains sealed;
- morphology, etymology, semantic signs, and transfer A/B/C may be prepared as
  contracts, but may not consume these units as accepted German.

The next admissible evidence is a real pass-1 source review followed by an
independent pass-2 verification. The user's personal read-aloud layer remains
separate and begins only after philological source acceptance.
