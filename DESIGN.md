# ToS Design

Tree of Sophia is a source-first philosophical tree. It organizes witnesses,
observations, interpretations and reviewed knowledge so that every derived
view can return to the material and decisions that support it. This structure
serves the [Charter's research purpose](CHARTER.md#purpose): tracing, comparing
and measuring thought while keeping each representation open to examination.

A tree records descent, inheritance and branch context. A graph exposes
relations across those branches. Both are useful; authored sources and review
records govern the graph's meaning.

## Shape

| Surface | Owns |
| --- | --- |
| Root documents | purpose, architecture, boundaries, direction and public entry |
| [ToS/](ToS/README.md) | the authored philosophical source home |
| [Doctrine](ToS/doctrine/KNOWLEDGE_MODEL.md) | knowledge model, node law, naming and interpretation discipline |
| [Source witnesses](ToS/source-witnesses/README.md) | identity, exact material, provenance, rights and source-near layers |
| [Philosophy](ToS/philosophy/README.md) | eras, traditions, works, concepts and transmission branches |
| [Zarathustra](ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md) | the first deeply worked source-to-review kernel |
| [Candidate intake](ToS/candidate-intake/) | provisional observations and proposals |
| [Review ledger](ToS/review-ledger/) | dated judgments, grounds, disagreement and scope |
| [Canon](ToS/canon/) | reviewed authored nodes and relations |
| [Mechanics](mechanics/README.md) | repeatable operations around those sources |
| [Access](access/README.md) | native reading, CLI/HTTP/MCP and browser delivery |
| [Derived exports](ToS/derived-exports/README.md) | reproducible graphs, indexes and bounded public handoffs |
| [Statistics](stats/README.md) | local measurements over declared populations and evidence |

The [source-home manifest](ToS/source_home.manifest.json) records branch
membership. Concrete record shapes live in [contracts](ToS/contracts/);
implementation lives with its native crate or adapter.

## Coupled Growth

The world-philosophy foundation supplies breadth: source identity, language,
time, place, transmission and stable addresses. Its people, works and contexts
support both mathematical exploration and deeper philological work in the
[roadmap's sequence](ROADMAP.md#current-tos-direction). The Zarathustra kernel supplies
a dense worked route through observation, interpretation, review, rejection,
ambiguity and promotion.

Transfer the source-to-review method to other branches while allowing their
own vocabulary and ontology. A useful gold packet exposes the grounds for
accepted, rejected and unresolved readings, together with version lineage.
The [golden-kernel contract](ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md) owns that
packet's requirements; branch research and review records retain worked cases.

## Corpus Evidence Spine

```text
work -> expression -> edition -> item -> immutable file
  -> passage or region -> observation -> versioned claim
    -> scoped assessment -> admission, canon or deferral -> derived view
```

Physical artifacts and scholarly composites have parallel identity routes;
they are not forced into the bibliographic ladder. Aggregate volumes can group
Works through Collections. A subset or one-member grouping does not create a
new Work without bibliographic evidence.

Stable IDs preserve the referent and its history while paths and labels may
improve. An exact source anchor identifies the immutable file, representation
and selector separately. Byte, Unicode code-point and browser UTF-16 positions
retain their declared units.

Extraction, OCR, correction, normalization, segmentation, alignment and semantic
annotation are separate versioned layers. Each transformation records its exact
inputs, outputs, method and provenance. Competing segmentations and readings
remain independently addressable. A derived normalized text never silently
replaces its diplomatic predecessor.

Claims retain maker, method, time, evidence, uncertainty, alternatives and
review posture. Responsibility, provision activities, time facets, catalogue
reports and identity reconciliation are claim-owned; shared labels do not
establish equivalence. A provider observation records what that provider
reported at that time.

[Corpus Foundation](ToS/doctrine/CORPUS_FOUNDATION.md) owns these distinctions.
[Knowledge Assessment](ToS/doctrine/KNOWLEDGE_ASSESSMENT.md) owns competent,
source-visible human or authorized-agent assessment. Rights, personal consent,
publication and canon retain their respective owner decisions. Historical
review formats keep their original meaning; new assessment routes use explicit
adapters.

### Source-witness topology

The [source-witness guide](ToS/source-witnesses/README.md) owns physical routing
for Works, Expressions, Editions, Items, Collections, artifacts, scholarly
composites, Agents, Places and Organizations. Their stable IDs survive explicit
path migrations.

Authored metadata, fixity, provenance, rights and forensic evidence remain in
Git or an explicitly selected immutable corpus revision. Bulk source retirement
requires exact membership, historical locators and verified restore. Generated
catalogs are replaceable data artifacts. Payloads, processing caches and large
outputs stay outside the source checkout in explicitly selected storage.

[Source-owner operations](mechanics/growth-cycle/parts/branch-growth-cycle/docs/SOURCE_OWNER_OPERATIONS.md)
own mutation procedures. [Release guidance](docs/RELEASING.md) separates
software delivery, corpus admission and data publication.

### Replaceable projections

Lexical indexes, embeddings, RDF, property graphs, KAG exports and visualization
databases read stronger source records. Every projection preserves the relevant
source references and assessment states, and can be rebuilt from declared
versioned inputs.

The bibliographic graph reads source-witness claims. The philosophy graph reads
branch structure and review lenses. They have different source populations;
neither becomes the sole copy of an assertion or review decision. Exact
current query behavior belongs to the [access contracts](access/contracts/README.md).

The maintained implementation is a [Rust workspace](rust/README.md) with shared
native/WASM rules and explicit adapters. PostgreSQL holds transactional source
updates and their durable queue; independent projections serve reading and
search under [TOS-D-0071](docs/decisions/TOS-D-0071-transactional-source-and-independent-projections.md).
ToS builds, operates and exports independently of ecosystem checkouts and
services under [TOS-D-0072](docs/decisions/TOS-D-0072-repository-independent-execution-and-exports.md).
Consumers own their integrations. Useful libraries, databases, processing tools
and hosting remain explicit dependencies of the operations that use them.

## Movement

1. Identify the source, work, concept, relation or branch pressure.
2. Follow the nearest source owner and its contract.
3. Preserve evidence, uncertainty and competing readings through review.
4. Promote only within the scope of the actual admission or canon decision.
5. Rebuild the affected public or derived companions from those sources.

[Boundary law](BOUNDARIES.md) assigns authority. [The roadmap](ROADMAP.md) names
future pressure. [Agent-surface design](DESIGN.AGENTS.md) explains contributor
routing. These documents point to the owners of detailed contracts and cases
rather than maintaining parallel inventories.
