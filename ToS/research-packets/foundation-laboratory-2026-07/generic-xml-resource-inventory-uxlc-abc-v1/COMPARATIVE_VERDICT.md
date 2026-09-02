# Generic XML resource inventory and UXLC projection comparative verdict

Date: 2026-08-30  
Result: B generic owner preferred; C retained only over B  
Hard gates: 25/25 pass after manual source/output review  
Comparison result SHA-256: `0fe557af…2708`
Public contract changed by this laboratory: no

## Decision

The laboratory selects this layered source-return architecture:

```text
A exact capture view

B exact-file generic XML element owner
  └── C UXLC provider-coordinate projection
```

- A remains useful as the smallest exact capture view, but it cannot own
  element return.
- B owns generic XML element topology inside one immutable File. Its identity
  is prefix-independent and file-bound; it does not claim cross-file element
  identity.
- C is rejected as a generic owner because it recognizes UXLC vocabulary and
  cannot represent arbitrary XML.
- C is retained as a derived adapter because all provider resources cite B and
  return to exact source elements.

No per-element content or label fingerprints are selected. Exact File fixity
detects byte change; private canonicalization may compare two captures, but a
short-element digest would not reliably remain source-text-free.

## Evidence sequence

### 1. Normative and classical

XML 1.0, XML Namespaces, XPath, XPointer, xml:id and Canonical XML establish
expanded-name identity, document order, conditional intrinsic IDs, fragment
addressing and bounded canonical comparison. None assigns TEI or provider
semantics to arbitrary local names.

### 2. Established scholarly work

CITE/CTS and digital scholarly-edition work support separating source return,
canonical citation, stand-off analysis and graph projection. The laboratory
uses that separation without claiming CTS conformance or an accepted
scholarly citation hierarchy for UXLC.

### 3. Fresh current state

TEI P5 4.12.0 still requires TEI namespace identity. Current lxml 6.1.1 /
libxml2 2.12.10 behavior and current XML security guidance require an explicit
no-DTD/no-entity/no-network posture. 2025–2026 digital-edition work continues
to support stand-off editorial structure rather than collapsing provider
markup and interpretation.

The full evidence ladder is recorded in
`GENERIC_XML_RESOURCE_IDENTITY_AND_PROVIDER_PROJECTION_RESEARCH_2026-08-30.md`.

## Executed controls

The revised complete run executed 133 fresh processes:

- 116 successful candidate builds;
- 16 DTD/entity/malformed builds failed closed;
- one C-on-generic-XML build failed source-shape recognition;
- zero unexpected exits;
- zero negative outputs.

Every successful A/B/C/B+C output was independently rebuilt byte-identically
in a second process. A third, separately implemented consumer imported no
builder code and returned:

- 133/133 B elements for the selected source;
- 133/133 B elements for the same-selector replay;
- 28/28 C records through exact provider paths for both sources;
- 28/28 B+C records through existing B refs for both sources;
- zero path or metadata mismatch.

## Real-source result

The selected and replay files are both 8,227-byte, 133-element,
27-attribute, non-namespaced XML with three provider verses and 23 provider
word elements in 10 / 7 / 6 grouping. They have different exact digests:

- selected: `98fdc24c…303c`;
- replay: `d970996c…a30e`.

Their B ordered topology is identical (`c689dcf2…fb1c`), and their C projection
shape is identical. Direct private C14N comparison locates four changed
ancestor/envelope subtrees while the provider content subtree remains equal.
This proves the correction law expected by the preregistration:

```text
exact capture change
  ≠ topology change
  ≠ provider projection change
  ≠ accepted passage or Work equivalence
```

## Privacy and rights result

The source files and every source-derived candidate output remain mode `0600`
and Git-ignored. Source-value controls include element text, tails, attributes,
comments, and processing instructions from every exact source. After
final-result materialization, the evaluator enumerates all 128 Git-tracked
laboratory files, applies substring matching to generated JSON/receipt data,
and scans them against 75 private source-value controls; it found zero
source-value matches. The fixture privacy scan found zero Hebrew code points.

The private C and B+C outputs carry the already declared provider identity and
provider coordinates; they carry no Hebrew. The rights question for a future
tracked full structural inventory remains a separate promotion decision. This
laboratory does not convert the provider's permission for biblical Hebrew text
into permission to redistribute the complete XML response.

## Preserved negative evidence

Two non-source failures remain first-class:

1. the original runner mishandled GNU `time` failure output and the B+C builder
   used Python proxy `id()` instead of node-aware lxml identity;
2. the first evaluator result passed 24/25 but exposed an arithmetic error in
   the sealed PC1 count: seven resources had been preregistered as six.

Neither failure was erased or relabeled as a candidate result. The runner,
builder, consumer and evaluator revisions were separately frozen; candidate
outputs were not changed for the sealed-arithmetic correction.

## Quality, cost and speed

| Shape | Wall range | Peak RSS range | Real selected output |
|---|---:|---:|---:|
| A | 0.05–0.07 s | 25,396–25,700 KiB | 1,210 bytes |
| B | 0.05–0.07 s | 25,408–26,244 KiB | 190,972 bytes |
| C | 0.06–0.07 s | 25,448–25,744 KiB | 37,670 bytes |
| B+C | 0.06–0.07 s | 25,472–26,500 KiB | 246,154 bytes |

Direct monetary cost is zero; electricity and fully loaded cost were not
measured. No human task or language review was opened. Manual burden consisted
of two exact-source reopens, seven explicit XPath returns, representative
owner/projection inspection, 128 tracked-laboratory-file leak checks and one
correction comparison.

## Promotion posture

This result is sufficient evidence to consider an additive
`generic_xml_elements_v1` public contract profile. It is not itself that
promotion. Before changing `ToS/contracts/` and the canonical builder, a
separate promotion decision must close:

1. the public/private visibility of full source-derived element paths and
   attribute names under mixed-layer rights;
2. the exact schema migration and compatibility surface;
3. the distinction between generic B inventory and optional provider C
   projection;
4. the Item-manifest and source-anchor return route;
5. rollback behavior if another generic XML witness falsifies B.

## Stop line

Established:

- generic exact-file XML element-owner mechanics;
- strict parser/security posture;
- prefix-independent expanded-name paths;
- duplicate and interleaved sibling return;
- deterministic independent rebuild;
- real UXLC source return;
- dynamic-envelope correction separation;
- C-over-B provider navigation;
- text-free tracked receipts and measured local cost.

Not established:

- public schema/profile promotion;
- admitted UXLC Item, anchor or source-text layer;
- accepted Hebrew, verse/word segmentation or intrinsic word ID;
- morphology, lemma, transliteration, etymology or translation;
- alignment with OSHB, MACULA, BHSA, SDBH or another edition;
- Amenemope/Proverbs relation;
- philosophical sign, semantic claim, graph edge or canon state;
- redistribution of the exact XML response or future-site publication.
