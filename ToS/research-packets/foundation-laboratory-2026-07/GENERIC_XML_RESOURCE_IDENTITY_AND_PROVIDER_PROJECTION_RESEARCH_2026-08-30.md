# Generic XML resource identity and provider projection research

Date: 2026-08-30  
Research question: how can Tree of Sophia return from a source-neutral XML
resource to exact source structure without calling arbitrary XML TEI, exposing
source text, or turning one provider's vocabulary into the generic owner?  
Triggering witness: exact local UXLC 2.5 build 27.6 response for Proverbs
23:1–3  
Public contract authority: none

## Outcome

The source-visible prior is now narrower than the initial no-fit record:

```text
exact immutable XML file
  → generic element topology bound to that file
    → optional provider-specific derived projection
      → later source anchors and content layers
```

The generic owner should enumerate element resources by expanded name,
document order, parent, depth and a fully structured one-based path. It should
not interpret those elements as a book, chapter, verse, word, TEI division or
semantic unit. It should not publish per-word or per-element content digests by
default. The exact file digest owns byte identity; private source-visible
comparison may use declared canonicalization, but short-element digests can be
dictionary-recoverable and therefore are not automatically text-free.

UXLC book/chapter/verse/word coordinates remain useful. They belong in a
separate derived adapter that cites generic element resources and declares its
provider vocabulary, version and zero acceptance authority. This is not a
compromise between two competing owners: it is an owner/projection boundary.

## 1. Classical and normative foundation

### 1.1 XML and the information set

XML 1.0 Fifth Edition defines well-formed document syntax but does not assign
domain meaning to arbitrary element names. Namespaces in XML defines an
expanded name as namespace name plus local name. Prefix spelling is therefore
not identity: two prefixes can denote the same namespace, while an unqualified
`teiHeader` has no TEI namespace merely because its local name resembles a TEI
element.

XPath 3.1 compares expanded QNames by namespace URI and local name while
ignoring prefix spelling. XPath 1.0 also establishes document order and the
tree of element nodes. These are the minimum stable mechanics required for a
generic structural owner.

XPointer was designed to address subresources inside XML and explicitly warns
that shorthand fragment return depends on available ID typing. `xml:id` can
supply intrinsic identifiers when the source actually uses it. The UXLC
response does not supply an intrinsic ID for each verse or word, so ToS must
not invent source IDs and report them as intrinsic.

Normative sources:

- XML 1.0 Fifth Edition: <https://www.w3.org/TR/xml/>
- Namespaces in XML 1.0 Third Edition:
  <https://www.w3.org/TR/xml-names/>
- XPath 3.1: <https://www.w3.org/TR/xpath-3/>
- XPointer Framework: <https://www.w3.org/TR/xptr-framework/>
- xml:id 1.0: <https://www.w3.org/TR/xml-id/>

### 1.2 Canonicalization is comparison, not passage identity

Canonical XML 1.1 produces a physical representation for comparing XML
information content under its declared rules. The Recommendation explicitly
states that application-specific equivalence is outside its general account:
documents may differ canonically while remaining equivalent for an
application.

That boundary matters here. Two UXLC responses have different exact bytes
because the provider envelope contains a retrieval-time value, while their
selected content subtree matched under one declared C14N 1.0 observation.
Neither digest becomes a permanent Proverbs passage identity:

- whole-file digest identifies one exact capture;
- canonical subtree digest supports one declared comparison;
- provider coordinates identify a provider-defined location;
- Work/Expression/Edition identity remains a separate bibliographic layer.

Canonical XML 1.1: <https://www.w3.org/TR/xml-c14n11/>

### 1.3 Current ToS contract boundary

`source-resource-inventory.schema.json` owns mechanical, source-text-free
resource enumeration. It currently has profiles for PDF, DjVu, EPUB, TEI,
provider OCR XML, scan data, image archives and JSONL. Its generic XML media
route is incorrectly exhausted by the TEI adapter, which correctly fails on
the UXLC response because there is no TEI `text` element.

The failure must remain evidence. A generic XML profile cannot be introduced
by weakening TEI detection or accepting any element named `teiHeader`.

## 2. Established scholarly and engineering work

### 2.1 Canonical citation is not serialization position

Blackwell and Smith's CITE/CTS work separates citation, text content,
alignment and acts of analysis. Their ordered hierarchy of citation objects
supports machine-actionable passage return without treating XML serialization
as the scholarly citation system. That distinction supports a two-layer ToS
design:

- generic XML paths return to one exact source file;
- provider or scholarly citation projects over those source resources.

Sources:

- Blackwell and Smith, “Modeling Citable Textual Analyses for the Homer
  Multitext” (2016):
  <https://datascience.codata.org/articles/dsj-2016-017>
- Canonical Text Services URN specification:
  <https://cite-architecture.github.io/ctsurn_spec/>

The CITE model is evidence for separation, not a requirement to mint CTS URNs
for UXLC or Tree of Sophia. ToS has not established a canonical citation
registry for this witness.

### 2.2 Stand-off structure and graph projection

Established digital-edition work shows why source encoding, stand-off
annotation and graph projection should remain distinguishable. A text may be
encoded in XML, analyzed through separate aligned objects and projected into
RDF without making the graph or analysis the source owner.

Relevant sources:

- “Graph Data-Models and Semantic Web Technologies in Scholarly Digital
  Editing” (2021/2023 publication context):
  <https://cris.unibo.it/bitstream/11585/835330/3/Tomasi_Graph%20Data-Models_compressed%20%281%29.pdf>
- “Semantic precision: crafting RDF-based digital editions for unveiling the
  layers of historical correspondence” (2024):
  <https://academic.oup.com/dsh/article/39/3/813/7689293>
- “Archiving a TEI Project FAIRly” (2022):
  <https://doi.org/10.4000/jtei.4324>

The lesson used here is bounded: derived semantics and graph adjacency should
retain return paths and provenance. These sources do not validate UXLC verse
boundaries, Hebrew words or ToS graph claims.

## 3. Fresh 2025–2026 relevance check

Freshness is used to detect changed constraints, not to displace stable XML
standards merely because they are old.

### 3.1 TEI P5 4.12.0

The current TEI P5 release is 4.12.0, generated 2026-07-28. TEI elements are
defined in the TEI namespace. The exact UXLC response has root `Tanach`, no TEI
namespace and no TEI `text` element. The latest TEI release therefore
strengthens the no-fit; it does not provide a compatibility excuse.

- current release record:
  <https://www.tei-c.org/release/doc/tei-p5-doc/en/html/TitlePageVerso.html>
- current guidelines: <https://www.tei-c.org/release/doc/tei-p5-doc/en/html/>

### 3.2 Current digital critical-edition practice

Lazzerini and Di Franco (published 2026-05-21) compare inline parallel
segmentation with double-end-point attachment and use stand-off linkage to
represent witness families, corrective hands and hierarchies. This reinforces
the need to preserve source topology while placing editorial structure in an
explicitly derived layer.

- “Creating a digital critical edition of a classical text with XML/TEI”
  (2026): <https://umanisticadigitale.unibo.it/article/view/23486>

The article is about TEI critical editions, not generic XML. Its applicability
is architectural: provider/editing semantics should not be smuggled into a
source-neutral resource owner.

### 3.3 Current AI-assisted metadata work

The 2025 TEI-to-DCAT study treats LLM assistance as support for metadata
mapping and enrichment while keeping identifiers, standards conformance,
rights and provenance explicit. It does not justify accepting model-generated
structure as source truth.

- “Automatic Metadata Extraction Leveraging Large Language Models in Digital
  Humanities” (published 2025-12-18):
  <https://doi.org/10.3390/electronics14244962>

For this laboratory no LLM is used. Exact parser output and direct source
inspection are sufficient and cheaper.

### 3.4 Current parser and security posture

Current lxml documentation notes that lxml 6.0 made HTTP, FTP and zlib support
optional because automatic remote access and decompression create security
risks. Its parser exposes `no_network`, `resolve_entities`, `load_dtd`,
`recover` and `huge_tree`; permissive settings can change both safety and the
resulting information set. OWASP's current XML guidance recommends rejecting
DOCTYPE/external entities and preventing external retrieval.

- lxml parsing documentation: <https://lxml.de/parsing.html>
- OWASP XML External Entity Prevention:
  <https://cheatsheetseries.owasp.org/cheatsheets/XML_External_Entity_Prevention_Cheat_Sheet.html>
- Python XML vulnerability documentation:
  <https://docs.python.org/3/library/xml.html#xml-vulnerabilities>

The experiment must therefore record and enforce:

- XML mode, not recovery mode;
- no DTD loading or validation;
- no entity resolution;
- no network access;
- no XInclude;
- no `huge_tree` bypass;
- an explicit failure for any DOCTYPE declaration.

## 4. Source-visible UXLC pressure

The exact selected response has:

- 8,227 bytes and SHA-256 `98fdc24c…303c`;
- root `Tanach`, no namespace;
- 133 element nodes, 27 attributes, no comments and no processing
  instructions;
- one provider header and one content subtree;
- one selected book, one chapter, three ordered verse elements and 23 ordered
  word elements distributed 10 / 7 / 6;
- mixed-layer rights: copying permission for the Hebrew text, but no
  redistribution grant for the complete XML envelope.

No source text is reproduced here. These counts are factual forensic metadata.

The source creates five distinct identity questions:

| Question | Honest owner |
|---|---|
| Are these the same bytes? | exact File SHA-256 |
| Which element in this exact file? | generic expanded-name structural path |
| Is this element a UXLC verse/word? | versioned UXLC derived adapter |
| Does it represent Proverbs 23:1–3? | provider selector plus later reviewed passage binding |
| Is its content accepted/correct/semantically aligned? | later source-text, linguistic, translation and review layers |

Collapsing any two of these questions would make downstream graph edges look
more stable than their evidence.

## 5. Candidate consequences

### A — opaque document resource

Candidate A is a valid capture owner but an insufficient resource inventory.
It can establish exact bytes, media type and one document resource. It cannot
return a later verse/word assertion to the exact element that triggered it.

### B — generic element topology owner

Candidate B should bind one inventory to one immutable File and enumerate
element nodes only. Each resource needs:

- deterministic file-local resource ID;
- expanded name: nullable namespace URI plus local name;
- one-based preorder/document-order position;
- depth and parent resource ID;
- one-based position among all element children;
- one-based position among siblings with the same expanded name;
- a structured root-to-element path repeating expanded name and same-name
  sibling position at each step.
- sorted attribute expanded names and attribute count, never attribute values.

The path is an exact locator only under the bound file digest. It is not a
cross-version identity. Element IDs must not be reported as intrinsic source
IDs.

The first profile should deliberately exclude text nodes, comments,
processing instructions, namespaces as separate nodes, DTD declarations and
attribute values. Attribute names are structural observations and remain
source-local in this laboratory; their future public visibility still requires
the contract and rights decision that this lab does not make. Exact file
fixity detects every byte change. Future profiles may add other node kinds only
after a new use case and rights review.

### C — UXLC semantic adapter

Candidate C should derive provider resources from B:

- selected book;
- chapter;
- three verses;
- 23 word elements.

Each record must cite the B element resource and declare the UXLC vocabulary
and build. It may carry provider coordinate values required for return, but no
Hebrew content. It must not claim OSHB word IDs, BHSA slots, morphological
truth, accepted passage segmentation or a generic XML role.

As a replacement generic owner, C fails. As a derived adapter over B, C is the
expected useful architecture.

## 6. Content-fingerprint decision

The existing inventory contract permits one-way content fingerprints. That is
not sufficient reason to emit them for generic XML:

- a digest of a short word or label can be recovered from a dictionary or a
  bounded candidate corpus;
- canonicalization choices affect the hashed representation;
- hash equality proves only equality under the declared procedure;
- source rights may differ between content and markup;
- public `source_text_included: false` would be misleading if small protected
  strings were cheaply recoverable.

Therefore the registered B candidate has no public per-element content or
label fingerprint. Private comparison may calculate coarse or per-element
digests, but tracked evidence records only aggregate equality/change facts and
non-content digests unless publication is separately cleared.

## 7. Expected correction law

For two captures with unchanged topology and changed provider envelope value:

1. exact File digest and File identity change;
2. B resource IDs may repeat as file-local labels, but their enclosing File
   binding changes, so no global identity equality is implied;
3. B topology can compare equal as a derived fact;
4. private content comparison changes only at the changed node and its
   ancestors;
5. C provider coordinates can compare equal if the same provider passage is
   returned;
6. no source text, translation, semantic claim or accepted passage identity is
   inferred.

For sibling reorder:

1. exact File digest changes;
2. document order and child positions change;
3. structured paths change where same-name ordinal or ancestor position is
   affected;
4. no normalizer may silently preserve the old return path.

## 8. Decision and stop line

Proceed to a source-visible A/B/C laboratory with B as the generic-owner prior
and C only as a derived projection prior. Do not alter the public schema or
canonical builder until:

- frozen synthetic and exact-source controls execute;
- DTD/entity inputs fail closed;
- two independent processes rebuild identical owner bytes;
- a separate consumer returns every selected UXLC element from B without
  reading the builder's internal state;
- correction replay distinguishes exact file change, topology equality and
  provider-coordinate equality;
- direct manual source/output inspection confirms that tracked outputs contain
  no Hebrew or other source strings;
- rights and authority boundaries remain explicit.

This research establishes a candidate architecture only. It establishes no
new public contract, admitted UXLC Item, accepted Hebrew, intrinsic word ID,
translation, Amenemope relation, semantic annotation, graph edge, canon state
or publication permission.
