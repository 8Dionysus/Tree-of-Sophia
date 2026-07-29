# Discovery Run Receipts

Schema-valid query/result records live here. The July 2026 foundation research
is not backfilled because its earliest exact query order was not captured under
the new protocol; that gap is stated in `../DISCOVERY_PROTOCOL.md`.

`ekgwb-za-i-vorrede-1.2026-07-28.v1.json` is the first protocol-native run. It
preserves a stable GND work record, repeated direct Nietzsche Source failures,
an operator hostname error, and the bounded noisy general-web result order.
The run is intentionally `incomplete`: it selected no critical-edition
witness, downloaded no source payload, inferred no rights, and opened no
source-review or translation lane.

`antonovsky-cultural-revolution-2007-volume-4.2026-07-28.v1.json` reconciles
the supplied Antonovsky edition against the originating Russian State Library
record. The selected record supports edition identity, responsibility,
pagination, RSL record number, and ISBN. It does not identify the exact local
PDF, reconstruct its acquisition history, or establish redistribution rights.

`mysl-1996-volume-2.2026-07-28.v1.json` reconciles the supplied *Mysl* volume
against the Vernadsky National Library catalog and the CiNii Books union
catalog. The selected records support volume and set identity, ISBN, and NCID.
The catalog extent of 830 pages and the local PDF container count of 831 are
both retained; no exact digital-item equivalence or rights conclusion is
inferred.

`nietzsche-mysl-volume-2-member-works.2026-07-28.v1.json` resolves the six
previously unrepresented whole-work identities in that volume through current
DNB/GND authorities. It preserves two section-level `Jenseits von Gut und
Böse` records and the unrelated `Venedig` result as explicit rejections rather
than silently merging them with whole works.

`antonovsky-1913-wikimedia-open-witness.2026-07-28.v1.json` separates the
modern local Antonovsky editions from a source-visible 1913 witness. It selects
the exact 402-page Wikimedia Commons scan, whose description declares the
digital object public domain, and acquires a checksum-matched local research
copy. It defers the linked CC BY-SA Wikisource transcription because its
proofreading posture is unresolved. Open rights evidence therefore opens a
candidate route without being confused with textual quality or gold.

`dta-zarathustra-part-1-open-structured-witness.2026-07-28.v1.json` resolves
the 1883 Schmeitzner first edition of part 1 through DNB, the direct Deutsches
Textarchiv object, and TextGrid's university-repository API. It selects and
acquires the exact DTA DTABf/TEI P5 object while retaining five ordered
TextGrid alternates as cross-checks. The run preserves DTA's external
native-speaker-checking statement, the current open-license evidence, the
stale conflicting Dublin Core field, and the separate facsimile-rights layer.
None of those facts makes the text ToS-accepted or critical.

`dta-zarathustra-parts-2-4-open-structured-witnesses.2026-07-28.v1.json`
continues the same ordered route across parts 2 and 3 and the 1891 first public
edition of part 4. It selects the three direct DTA objects, preserves the
unchanged five-result TextGrid order and exact response digest, and acquires
three checksum-matched local TEI items. The run keeps their distinct years,
publishers, holding copies, source scopes, and rights records rather than
collapsing them into one supposed “original text.”
