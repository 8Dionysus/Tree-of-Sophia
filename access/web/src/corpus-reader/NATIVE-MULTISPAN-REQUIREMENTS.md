# Native multispan whole-book requirements

This is a requirements handoff for the future native whole-book owner. It does
not bind a backend, grant source access, or change the legacy corpus reader.
The frontend adapter may map the owner operation named `manifest` to its
existing `document` seam, but it must preserve the owner packet shape and
status.

The source authority is
[`source-text-unit-packet-v1`](../../../../ToS/contracts/source-text-unit-packet-v1.schema.json).
Exact source selectors follow
[`source-anchor-v2`](../../../../ToS/contracts/source-anchor-v2.schema.json).
Translation mappings follow
[`translation-alignment-packet-v1`](../../../../ToS/contracts/translation-alignment-packet-v1.schema.json).
Those contracts remain stronger than this access projection.

## Owner operations

The owner supplies exactly four read operations. Each response is bounded,
revision-pinned and independently validates its identity before the browser
uses it.

| Operation | Required responsibility |
| --- | --- |
| `catalog` | Page works and available representations. Preserve stable work, expression, edition, item, file and text-layer identifiers, version availability, visibility, rights notice and optional `total`. |
| `manifest` | Return the selected work's versions and source packet inventory. Carry packet, segmentation, unit and text-layer versions, supersession references, unit-count knowledge, ordered child references and rights state without requiring text payloads. |
| `window` | Return one bounded ordered page of source units. Keep packet and segmentation identity with every unit, preserve ordered child references and every source span or declared gap span, and return opaque previous/next cursors plus an optional total. |
| `search` | Search only the explicitly requested version or an explicitly advertised corpus scope. Return exact unit and span addresses with the same packet, segmentation, unit and text-layer versions; a result excerpt is never a replacement for the source span. |

The owner must treat an omitted or `null` `total`/`unitCount` as unknown. Zero
means that the owner has asserted an empty result or empty source. The reader
may continue an empty intermediate page when it carries an opaque
`nextCursor`, one bounded request at a time. It must never infer a total from a
page, prefetch all cursors, or turn a metadata page into text.

## Packet and unit closure

Every source-bearing returned unit needs an identity closure equivalent to:

```json
{
  "packet": {"id": "...", "version": 1, "sha256": "..."},
  "segmentation": {"id": "...", "version": 1},
  "summary": {
    "unit_id": "...",
    "unit_version": 1,
    "layer_id": "...",
    "layer_version": 1,
    "segmentation_id": "...",
    "segmentation_version": 1
  },
  "ordered_child_unit_refs": ["..."],
  "spans": [
    {
      "anchor_ref": "...",
      "selector": {"type": "text_position", "start": 0, "end": 1,
        "position_unit": "unicode_code_point", "interval": "half_open"},
      "exact_sha256": "...",
      "text": "..."
    }
  ]
}
```

The concrete native return may add owner fields, but it must retain the
versioned packet, segmentation, unit and layer identities and their digests.
For packet units, `ordered_anchor_refs` and
`ordered_child_unit_refs` are source-owned order, not a browser-generated
sort. A unit can be discontinuous. Gaps, omitted source ranges and source
boundaries therefore travel as explicit ordered gap spans or gap anchors with
their selector, role, digest and reason. The owner must not silently join
separate spans with whitespace, punctuation or a synthetic separator.

The browser renders each returned span in order and keeps its address. It does
not synthesize `unit.text` by concatenating spans, does not make a gap look like
source wording, and does not use child order as an inferred textual offset.
Selection across two spans remains a cross-span action requiring an explicit
owner rule; it is not a new continuous source anchor.

Unit and segmentation revisions follow the packet's supersession rules. A
version 2 packet, segmentation or unit must identify the exact predecessor it
supersedes. The owner must reject mixed revisions, duplicate ordered refs,
wrong layer digests and spans whose code-point interval or exact digest does
not match the returned bytes. Cursor continuation is tied to the same source,
packet, segmentation and representation revision.

## Rights and availability

`catalog`, `manifest`, `window` and `search` all carry the current visibility
and rights result for the requested representation. Metadata-only,
restricted, expired, missing and unavailable states remain inspectable in
catalog and manifest. A text operation returns a typed unavailable result when
rights or delivery do not permit wording; it must not return a guessed source,
an older version, or a knowledge-catalog record in its place.

For conditional local reading, the owner returns its complete required notice
set and machine-checkable expiry and fixity information. License and attribution
notices are retained when that source requires them; the browser does not
invent notices or infer a license from access alone. Expiry is enforced on every read and continuation, not
only when the manifest was first opened. Public metadata and local text access
remain separate visibility states, and the access layer does not grant current
use or publication authority.

## Alignment

Parallel panes may show two independently supplied versions. They acquire an
alignment control only when the owner returns a matching, source-linked
`translation-alignment-packet-v1` mapping. The owner checks both sides' work,
expression, edition, item, file, text-layer and segmentation identities; every
mapping keeps ordered source and target anchor refs, correspondence shape,
alignment/claim versions, status, evidence and review state. The owner also
checks each referenced anchor selector and digest against the returned packet
before advertising the mapping.

An unreviewed, stale, disputed, omitted or identity-mismatched mapping is
visible as unavailable or unresolved. The reader does not align by language,
ordinal, similar text, paragraph count, scrolling position or title. Absence
of an alignment packet leaves the two panes independent.

## Owner dependencies and acceptance

The future owner must provide:

1. the packet and source-layer records validated against the source-text-unit
   schema, including rights and visibility;
2. a bounded native adapter for the four operations, with opaque cursor,
   unknown-total and unavailable-state tests;
3. exact span and gap closure tests covering discontinuity, ordered child refs,
   packet/segmentation/unit supersession and code-point/digest mismatch;
4. an alignment validator bound to the translation-alignment packet, with
   positive, stale, unresolved, reordered and rights-restricted cases; and
5. a transport receipt that identifies the owner, current source revision,
   response byte budget and authority boundary.

This handoff is complete when those owner tests and receipts exist. A green
frontend validator or a synthetic fixture cannot accept native source meaning,
rights, alignment or canon.
