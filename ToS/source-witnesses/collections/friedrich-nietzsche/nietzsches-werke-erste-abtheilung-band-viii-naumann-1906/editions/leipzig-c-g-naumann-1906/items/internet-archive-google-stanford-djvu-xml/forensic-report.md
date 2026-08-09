# Forensic Intake — Internet Archive Google/Stanford navigation derivatives

Status: AI-assisted exact-file and structural inspection; no human textual or
rights review

Inspected: 2026-08-08

Payload visibility: local only

## Exact objects

The Item owns the current Internet Archive derivative
`nietzscheswerke00nietgoog_djvu.xml`: 8,882,082 bytes, provider SHA-1
`4f4efaafa1a5f849aa97a9282805dd335098ed93`, and local SHA-256
`2307ace28af92da2b0128a5ef750e995d83a5655359e0debd3adb0cf1044b8c7`.
The size and SHA-1 match the fresh official metadata response captured on
2026-08-08. The local payload is held only in the canonical gitignored source
tree with mode `0600`.

The same Item now also owns the exact official
`nietzscheswerke00nietgoog_jp2.zip` page-image package (79,087,792 bytes,
525 zero-based contiguous JP2 members, provider SHA-1
`118ae03f81c0e82138a56db5eef78a0aecf8813d`, local SHA-256
`fa52999956bb9190e54ef2d52ed03dfbbfd4f1e91c0034319c341d48702d19a9`)
and `nietzscheswerke00nietgoog_scandata.xml` (157,669 bytes, 525 leaves,
provider SHA-1 `432ce3a49315b8d6502ba7d6f6504b9ce6837e7a`, local SHA-256
`5b2c0fe0ec55f1d330a17c066adbc56f2e0bc5b5cd4eb4761fe844601e73a8d5`).
Both match the fresh official file record and remain mode `0600` in the
canonical gitignored source tree.

## Container and page structure

Python's standard XML parser accepts the file as well-formed DjVuXML with 525
`OBJECT` pages. The resource inventory records page order, geometry, counts,
and one-way content fingerprints but emits no OCR strings. This is two pages
more than the separately fixed Commons DjVu's 523 scan pages.

The Internet Archive derivative family and Commons scan remain distinct Items.
A stable two-page offset for the bounded
*Der Antichrist* numbered sequence is established only by the separately
tracked structural-map event, which combines ordered OCR number-label
candidates with source-visible Commons pages. This report does not project that
local relation to the rest of either container.

Within the Internet Archive Item, the scandata leaf sequence, JP2 member
sequence, and DjVuXML `OBJECT` sequence each contain 525 entries. The bounded
relation is zero-based leaf/member `n` to one-based navigation page `n + 1`.
This relation supports page-image return inside that Item only; it does not
collapse the separate Commons address Item into it.

## Bounded source-visible marker return

Exact JP2 members were visually inspected only for the OCR gaps that block the
frozen transfer frame. The printed markers `8.`, `9.`, and `44.` are visible on
navigation pages 240, 241, and 290 (JP2 leaves 239, 240, and 289). Their
threshold-bounded ink regions in the original 4034 x 5834 rasters are
respectively `(1839,2011)-(1895,2068)`, `(1979,2253)-(2040,2315)`, and
`(1834,3399)-(1931,3451)` in top-left pixel coordinates.

The marker tokens are absent from the provider OCR, but the first following
DjVuXML lines remain present at page/order coordinates 240/8, 241/10, and
290/20. These facts admit only a composite model-visible-marker to exact
automatic-line boundary candidate. They do not repair the OCR, transcribe the
image, accept German, or establish textual identity between the Internet
Archive and Commons Items. No human repeat was performed.

## Text and rights boundary

The files contain provider-generated page images, OCR, scandata, coordinates,
and layout. Their text is not accepted German, their coordinates are not an independent textual witness, and
its current Internet Archive descriptive title is source-visibly contaminated.
The historical Nietzsche text has a positive public-domain route in the
reviewed DE and US scopes, while the OCR coordinate arrangement, derivative
package, database/contract posture, and archive additions remain unresolved.
The exact files therefore remain local-research-only and are not authorized for
future-site upload or redistribution.

No source text, accepted German, translation relation, semantic unit, graph
claim, eligible transfer unit, target gold, or canon authority is created by
this intake.
