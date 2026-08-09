# Forensic Intake — Internet Archive Google/Stanford DjVuXML

Status: AI-assisted exact-file and structural inspection; no human textual or
rights review

Inspected: 2026-08-08

Payload visibility: local only

## Exact object

The Item owns the current Internet Archive derivative
`nietzscheswerke00nietgoog_djvu.xml`: 8,882,082 bytes, provider SHA-1
`4f4efaafa1a5f849aa97a9282805dd335098ed93`, and local SHA-256
`2307ace28af92da2b0128a5ef750e995d83a5655359e0debd3adb0cf1044b8c7`.
The size and SHA-1 match the fresh official metadata response captured on
2026-08-08. The local payload is held only in the canonical gitignored source
tree with mode `0600`.

## Container and page structure

Python's standard XML parser accepts the file as well-formed DjVuXML with 525
`OBJECT` pages. The resource inventory records page order, geometry, counts,
and one-way content fingerprints but emits no OCR strings. This is two pages
more than the separately fixed Commons DjVu's 523 scan pages.

The two files remain distinct Items. A stable two-page offset for the bounded
*Der Antichrist* numbered sequence is established only by the separately
tracked structural-map event, which combines ordered OCR number-label
candidates with source-visible Commons pages. This report does not project that
local relation to the rest of either container.

## Text and rights boundary

The file is provider-generated OCR plus coordinates and layout. Its text is not
accepted German, its coordinates are not an independent textual witness, and
its current Internet Archive descriptive title is source-visibly contaminated.
The historical Nietzsche text has a positive public-domain route in the
reviewed DE and US scopes, while the OCR coordinate arrangement, derivative
package, database/contract posture, and archive additions remain unresolved.
The exact file therefore remains local-research-only and is not authorized for
future-site upload or redistribution.

No source text, accepted German, translation relation, semantic unit, graph
claim, eligible transfer unit, target gold, or canon authority is created by
this intake.
