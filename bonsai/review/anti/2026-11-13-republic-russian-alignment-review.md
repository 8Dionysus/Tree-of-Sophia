# Plato, Republic: Russian card alignment and translation findings

Reviewed against Antiquity alignment at main `9abd3d46da1c75d54d835036bf256dcb432c368b`, Greek original `retained-republic-grc2-5f9f8ad62cdb` revision `5046b810cb3fa6ab9163967602811611ce1436031551d8a8bac06b0b1ee772e4`, and Russian witness `retained-republic-ru-karpov1863-wikisource` revision `ff832e136e90132c6496203d54325ccf72bfe743b35d3f9c3c6e7bde58c02025`. Offsets below count Unicode code points in the selected Greek units. The Russian descriptions report the pairs currently offered by the stored alignment.

| Greek unit, reference and range | Greek subject | Current Russian subject |
| --- | --- | --- |
| `tos.text-unit.sid-30964fd34db4458c914156b4c619690a`, IV.434, 1853–1933 | Transfer what was observed in the city to the individual | The preceding account of constructing the city |
| `tos.text-unit.sid-3dfba184a7244e53b1e30e372445e2f3`, VII.516, 335–390 | Habituation is needed to see the things above | The preceding answer that this would not happen immediately |

The current receipt reports alignment by sentences of the whole work. The original's 278 units carry references such as `Книга 4 · 434`, while this Russian witness's 278 units carry page numbers such as `434`. Their numbering is not recognized as equal by the existing counterpart selection. A trial matching the final number produced a subject-matching pair for IV.434, but also made I.331, 1936–2037 unrelated and truncated the Russian at IV.443 and VII.514. An additional trial offering sentence continuations across unit boundaries introduced further mismatches. Neither experiment is a safe replacement. Matching references alone does not prove corresponding sentences; a future repair must check the affected ranges and preserve exact source offsets.

A distinct translation issue occurs at `tos.text-unit.sid-69836c088fe34bf2a6dfe33029fdc437`, VI.509, 1537–1787. The selected Greek has `ἄνισα`, unequal sections; Karpov's Russian witness says equal sections. The printed [Karpov scan, page 342](https://plato.today/TEXTS/PLATO/Karpov/0306.pdf) was visually checked and likewise reads «равныя части». This is not established as a transcription or alignment error, and the historical witness must not be silently rewritten. A separately sourced and rights-reviewed alternative or correction would need its own attribution and review. This comparison does not decide the wider textual history.

The two alignment errors and the translation discrepancy above were directly compared with the Greek. An inventory captured 92 unique single-unit citations with their current Russian and English pairs; this is not an assessment of every pair, a complete review of the translation, or a change to the public alignment. No witness or card text was altered in this review.

## Scoped repair of VII.516

The subsequently added `localReferencePairs` scope binds the exact original
and Russian revisions above and names the complete 516 unit in each. The
Greek reference says `Книга 7 · 516` and the Russian one `516`; the unit
identities, not the number alone, are retained in the scope and receipt. The
normal sentence aligner is rerun on the two complete units. For the cited
Greek range 335–390 it selects the Russian sentence beginning «Понадобилась
бы, думаю, привычка», instead of the preceding response. The Russian sentence
continues with the next Greek sentence about seeing shadows and reflections;
that additional context is retained because it is part of the same sentence
in the retained Russian witness. The automatic group also includes the preceding Greek
answer, so it is not a claim of exact sentence-to-sentence boundaries for the
entire 516 unit. The cited thought was compared directly with Greek and the
retained Russian witness. No original or translation text is rewritten, and
the original remains available through the card's switch. Other Republic
citations, including the distinct IV.434 issue and the VI.509 witness
discrepancy, are not resolved by this bounded change.
