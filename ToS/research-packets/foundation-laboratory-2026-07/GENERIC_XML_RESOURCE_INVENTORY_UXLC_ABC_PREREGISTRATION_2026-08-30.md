# Generic XML resource inventory and UXLC projection A/B/C preregistration

Date frozen: 2026-08-30  
State: immutable before candidate output  
Actor: `model:codex`  
Human participant: none  
Runtime/model/LLM: none  
Source payload posture: operator-local, Git-ignored, mode `0600`  
Public contract authority: none

## 1. Purpose

The exact UXLC Proverbs 23:1–3 response is locally possessed and fixity-
verified, but the current public source-resource inventory has no honest
generic XML profile. The canonical builder routes `text/xml` into the TEI
adapter and fails with `InventoryBuildError: TEI payload has no text element`.

This experiment compares three owner shapes:

- A — one opaque XML document resource;
- B — generic namespace-aware element topology bound to one exact File;
- C — UXLC book/chapter/verse/word resources used as the primary inventory.

It also tests the expected layered architecture: B as owner plus C as a
derived provider projection. The laboratory must determine returnability,
genericity, correction behavior, privacy, security, cost and manual burden
before any public-contract change or Item admission.

## 2. Frozen research inputs

| Input | Role |
|---|---|
| `GENERIC_XML_RESOURCE_IDENTITY_AND_PROVIDER_PROJECTION_RESEARCH_2026-08-30.md` | ordered normative → established → fresh research and candidate consequence |
| `A04_UXLC_PROVERBS_23_1_3_EXACT_RESPONSE_AND_GENERIC_XML_NO_FIT_2026-08-30.md` | exact witness, dynamic envelope, rights split and current no-fit |
| UXLC exact local payload SHA-256 `98fdc24c…303c` | selected real source input; never copied into tracked lab output |
| `source-resource-inventory.schema.json` SHA-256 `40f44a88…ca5f5` | current public owner control |
| `build_source_resource_inventories.py` SHA-256 `94fa0595…a7cb6` | current canonical builder and fail-closed route |
| `resource-inventory.no-fit.v1.json` SHA-256 `34cb8216…b759` | pre-lab negative result |
| `rights.json` SHA-256 `7c34ba6b…bb65` | mixed-layer redistribution boundary |

The preregistration itself will be hashed into `freeze-receipt.json` before
any candidate output is generated.

## 3. Input/output isolation

The tracked laboratory directory will contain:

1. `input-manifest.json` with only public-synthetic fixtures, exact source refs,
   fixity, counts and candidate-independent operations;
2. `sealed-evaluation-manifest.json` with expected gates and forbidden
   implications;
3. builder and consumer programs;
4. public-synthetic candidate outputs;
5. source-visible text-free receipts and aggregate metrics.

The exact UXLC payload and every source-derived candidate owner/projection are
written only beneath a Git-ignored local laboratory path in
`/srv/AbyssOS/Tree-of-Sophia/ToS`. Tracked receipts may include file hashes,
resource counts, topology digests, pass/fail booleans and timings, but not
source strings, attribute values or complete source-derived paths.

Candidate builders may read `input-manifest.json`, synthetic fixtures and the
declared local source path. They may not read the sealed evaluation manifest.
The evaluator may read the sealed manifest only after candidate outputs exist.

## 4. Parser contract

Every candidate that parses XML must use the same frozen posture:

- strict XML parsing; no recovery;
- `load_dtd = false`;
- `dtd_validation = false`;
- `resolve_entities = false`;
- `no_network = true`;
- `huge_tree = false`;
- no XInclude processing;
- reject any document containing a DOCTYPE declaration before inventory
  creation;
- UTF-8 output with deterministic JSON key and array ordering.

The actual Python, lxml and libxml2 versions are measured at run time and
recorded. A candidate fails if it relies on an undeclared default or silently
recovers malformed XML.

## 5. Candidate A — opaque document owner

### Hypothesis

One resource represents the entire XML file with exact SHA-256, byte size,
media type and an `xml_document` role.

### Expected strengths

- smallest output;
- complete byte-fixity binding;
- no provider vocabulary;
- low rights leakage and low construction cost.

### Registered failures

A fails owner adequacy if it cannot:

- return a later verse or word assertion to one exact source element;
- distinguish provider header and content topology;
- expose sibling order required for exact source return;
- support a provider projection without reparsing the hidden source.

A may still remain a valid file-capture view. Losing owner status does not
invalidate its fixity behavior.

## 6. Candidate B — generic expanded-name element owner

### Hypothesis

One inventory binds to one exact File and enumerates every element node in
preorder. Each resource contains only:

- deterministic file-local ID;
- nullable namespace URI and local name;
- preorder position;
- depth;
- parent resource ID or null at root;
- one-based element-child position;
- one-based same-expanded-name sibling position;
- structured root-to-element path of expanded names and same-name sibling
  positions;
- element-child count, attribute count and sorted attribute expanded names.

No text, tail, attribute value, comment, processing instruction or DTD content
is serialized. No per-element content/label hash is emitted in tracked output.

### Expected strengths

- source-format neutral;
- prefix-independent;
- exact return inside the bound immutable File;
- document order and duplicate-sibling disambiguation;
- adequate substrate for provider adapters without importing their semantics;
- deterministic independent rebuild.

### Registered failures

B fails if:

- prefix spelling changes identity while expanded names remain equal;
- no-namespace and TEI-namespace elements collapse;
- two same-name siblings receive the same path;
- interleaved sibling names lose all-child order;
- a path resolves to zero or multiple elements in the bound source;
- source text or attribute values enter tracked output;
- per-element hashes make short source strings cheaply recoverable;
- B claims stability across different File digests;
- a DTD/entity input produces an inventory;
- provider passage coordinates cannot be projected without reading builder
  internals.

## 7. Candidate C — UXLC-specific primary owner

### Hypothesis

The primary inventory contains only UXLC-selected book, chapter, verse and
word resources with provider coordinates and exact source element refs.

### Expected strengths

- smallest domain-useful return surface;
- direct three-verse/twenty-three-word query;
- low downstream provider-query cost.

### Registered failures

C fails as generic owner if:

- it cannot represent an unrelated well-formed XML vocabulary;
- UXLC names or numbering enter the generic profile contract;
- it requires provider semantics to prove XML element identity;
- it reports provider word positions as OSHB IDs, BHSA slots or accepted ToS
  word units;
- a provider correction requires rewriting generic source identity;
- it cannot cite a source-generic resource without reparsing the source.

C may pass as a derived adapter over B even while failing primary-owner fit.

## 8. Public-synthetic positive controls

All fixture strings are invented for this laboratory and contain no UXLC or
Nietzsche source text.

| ID | Fixture pressure | Required observation |
|---|---|---|
| P1 | no-namespace root and nested elements | namespace URI is null; paths resolve exactly |
| P2 | two prefixes bound to the same namespace | expanded-name identity ignores prefix spelling |
| P3 | default namespace plus unqualified attribute | element namespace retained; unqualified attribute does not inherit it |
| P4 | three same-name siblings | one-based same-name ordinals 1/2/3 are unique |
| P5 | interleaved `item`, `note`, `item` children | all-child order 1/2/3 and same-name item order 1/2 both survive |
| P6 | empty element versus explicit start/end pair | exact file digest may differ while element topology compares equal |
| P7 | reordered attribute serialization | exact bytes may differ; attribute count/name set topology remains equal |
| P8 | comment and processing-instruction variation | exact bytes change; element-only scope is explicit |
| P9 | nested mixed namespaces | every path resolves by expanded name, not prefix |
| P10 | true TEI namespace fixture | B sees generic elements; format routing may separately prefer TEI adapter |
| P11 | unqualified `teiHeader` lookalike | never classified as TEI |
| P12 | dynamic envelope value with stable content subtree | exact File changes; topology remains equal; no silent exclusion |
| P13 | sibling reorder | document order/child positions and affected paths change |
| P14 | text-only mutation | exact File changes; topology equality does not imply content equality |
| P15 | attribute-value-only mutation | exact File changes; no attribute value leaks into owner |
| P16 | identical repeated input | byte-identical deterministic output in separate processes |

## 9. Security and false-equivalence negatives

| ID | Input or claim | Required failure |
|---|---|---|
| N1 | external entity and local-file URI | no inventory; no external read |
| N2 | internal entity/DOCTYPE | no inventory even when expansion seems harmless |
| N3 | Billion Laughs-style entity chain | no inventory before expansion |
| N4 | malformed XML with recoverable suffix | no inventory; recovery forbidden |
| N5 | namespace prefix used as identity | evaluator rejects |
| N6 | no namespace treated as TEI namespace | evaluator rejects |
| N7 | whole-file digest called stable passage ID | evaluator rejects |
| N8 | C14N equality called Work/passage equivalence | evaluator rejects |
| N9 | topology equality called content equality | evaluator rejects |
| N10 | source-local ordinal called intrinsic word ID | evaluator rejects |
| N11 | provider word called accepted ToS word | evaluator rejects |
| N12 | provider projection used without B source ref | evaluator rejects |
| N13 | tracked owner contains any synthetic-forbidden source token | evaluator rejects |
| N14 | tracked source receipt contains text/attribute values | evaluator rejects |
| N15 | public contract or canonical builder changed during lab | evaluator rejects |
| N16 | any Item, anchor, text layer, semantics or graph state admitted | evaluator rejects |

## 10. Exact-source controls

### S1 — exact selected response

The source-visible run must observe, without reproducing source strings:

- File SHA-256 `98fdc24c…303c` and 8,227 bytes;
- 133 elements and 27 attributes;
- root local name `Tanach`, null namespace;
- no TEI namespace and no TEI `text` element;
- three provider verse elements;
- 23 provider word elements in 10 / 7 / 6 grouping.

Expected candidate counts:

- A: one document resource;
- B: 133 element resources;
- C: 28 provider resources if book + chapter + 3 verses + 23 words are
  selected; header elements remain outside C but inside B.

### S2 — source return

A separate consumer receives only B owner bytes plus the bound local source.
For every B resource it must resolve exactly one element and reproduce the
registered expanded name, depth, parent and order facts. It must not import or
call the candidate builder.

For C, the consumer must resolve every projected provider record through its B
resource ref. A C record with no B ref or a ref to the wrong element fails.

### S3 — same-selector correction replay

A second actual `Server.xml?Prov23:1-3` capture may be acquired to a separate
local-only File after the preregistration is frozen. The replay must record:

- exact File equality/difference;
- B topology equality/difference;
- which generic structural nodes changed only in private content comparison;
- C coordinate equality/difference;
- absence of any inferred textual or semantic acceptance.

If the live response is byte-identical, use the already observed prior-capture
digest plus public-synthetic P12 for dynamic-envelope logic and report the live
replay as inconclusive rather than fabricating a correction.

## 11. Independent-process and correction gates

Each candidate is built twice in separate fresh processes from the same frozen
input. A third process performs source return. A fourth process evaluates the
sealed gates. All environment-sensitive fields belong in separate run
receipts, never in deterministic owner bytes.

Required gates:

- G1: preregistration and manifests predate candidate outputs;
- G2: frozen input digests match;
- G3: public synthetic fixtures contain no source text;
- G4: exact local source digest/mode/Git-ignore posture match;
- G5: parser posture is explicit and fail-closed;
- G6: N1–N4 yield no owner output;
- G7: A/B/C outputs rebuild byte-identically across processes;
- G8: B paths resolve exactly for P1–P16 and S1;
- G9: B expanded names are prefix-independent;
- G10: duplicate and interleaved sibling controls preserve both ordinals;
- G11: exact File change never silently normalizes away;
- G12: topology equality never implies content or passage equality;
- G13: C cites B for every provider resource;
- G14: C passes derived-adapter queries but fails generic-owner generality;
- G15: A passes capture fixity but fails element return;
- G16: tracked outputs contain no source strings or attribute values;
- G17: no per-element source content fingerprints are tracked;
- G18: no TEI false classification;
- G19: no intrinsic/accepted word-ID claim;
- G20: no public contract, Item, anchor, text, semantics or graph mutation;
- G21: source-return consumer is independent of builder implementation;
- G22: correction replay distinguishes exact file, topology and provider
  projection;
- G23: manual direct source/output review precedes final verdict;
- G24: quality, wall time, peak RSS, output bytes and estimated human burden
  are reported separately;
- G25: machine-green results remain explicitly non-authoritative.

No candidate can win with an unexercised G6, G8, G16, G20, G21 or G23.

## 12. Manual review protocol

This is a solo+AI project. No Human Gold task is created. The Master performs
one source-visible review at the material decision boundary:

1. verify the exact local file before reading candidate output;
2. inspect root namespace, total elements/attributes and provider counts
   independently of the builder;
3. open representative B paths: root, header lookalike, content subtree,
   first/last verse and first/last word;
4. resolve those paths manually with an independent query;
5. inspect tracked outputs for source strings and attribute values;
6. inspect C as a projection and confirm every record returns through B;
7. compare candidate mechanics with the preregistered stop lines;
8. write a verdict before running the final evaluator.

The review does not assess Hebrew orthography, grammar, morphology or
translation. Those require a separate competent-language workflow.

## 13. Metrics

For A, B, C and B+C record separately:

- resource count;
- deterministic owner bytes;
- build wall time;
- independent return wall time;
- observed peak RSS;
- exact-source path-return success;
- source-derived tracked bytes;
- direct monetary cost;
- estimated human inspection operations;
- generic owner fit;
- provider query fit;
- rights/privacy risk.

No composite score is preregistered. Quality gates dominate speed and size.
Among equally passing shapes, smaller deterministic owner bytes and lower
manual burden may decide.

## 14. Expected decision and stop line

The registered prior is:

- reject A as the only resource owner but retain it as a capture view;
- prefer B as generic exact-file element owner if all hard gates pass;
- reject C as generic owner;
- retain C as a derived UXLC adapter over B if all source refs and authority
  boundaries pass.

The experiment may falsify that prior. It cannot authorize changes outside its
scope.

Even a full pass establishes only resource-owner mechanics for generic XML and
one provider projection. It does not establish:

- an admitted UXLC Item or public inventory;
- accepted source text, verse or word segmentation;
- intrinsic cross-corpus IDs;
- morphology, lemma, transliteration, etymology or translation;
- equivalence with OSHB, MACULA, BHSA, SDBH or another edition;
- an Amenemope/Proverbs relation;
- a philosophical sign, concept, semantic edge or graph truth;
- canon state, redistribution permission, site transfer or publication.
