# ToS Design

## Role

This document names the system form of `Tree-of-Sophia`.

ToS is a source-first philosophical tree whose branches can later be read as
graphs while preserving their roots.

## Thesis

ToS grows by turning source-witnessed philosophical material into reviewable
nodes, relations, lineages, contexts, branches, and public routes.

A graph can show adjacency. A tree shows descent, pressure, branch,
inheritance, and return. ToS needs both, but graph views remain projections of
source-rooted growth.

## Shape

| Layer | Role |
| --- | --- |
| root | repository authority, public orientation, route law, current direction |
| `ToS/` | source home for the philosophical tree |
| doctrine | node law, naming discipline, relation contracts, calibration |
| philosophy | branch topology for traditions, eras, works, figures, concepts, transmissions |
| Zarathustra | first golden growth kernel and pressure-tested entry branch |
| canon | reviewed authored nodes, relations, and registries |
| review ledger | accountable inspection before stronger movement |
| mechanics | repeatable organs of operation |
| `stats/` | owner-local measurements over declared ToS populations and evidence refs |
| generated/exported companions | compact readers, public seams, graph/KAG handoffs, downstream views |

The layers can touch. They should not collapse.

## Coupled Growth

ToS advances through a broad foundation and a dense first cycle.

The world-philosophy corpus soil orders source identity, witness and edition,
language, time, place, tradition, transmission, and stable text addresses. It
does not pretend that bibliographic order is already semantic understanding.

The Zarathustra golden kernel makes one full path unusually legible: source
witness -> address -> observation -> semantic proposal -> relation and context
-> human review -> canon -> derived graph or retrieval view.

The kernel teaches agents how to move through that path, including how to
reject, defer, and preserve ambiguity. It does not teach them to impose
Nietzsche's ontology on the rest of the tree.

### Engineering ownership

| Stage | Owning surface | Required invariant |
| --- | --- | --- |
| corpus soil | `ToS/philosophy/` with `ToS/source-witnesses/` | ordered source identity and evidence status remain distinct from semantic claims |
| golden source | `ToS/source-witnesses/` | exact witness, provenance, language role, and stable address remain inspectable |
| observation and proposal | `ToS/candidate-intake/` under doctrine and contract law | source-near observation remains distinct from interpretive proposal |
| review | `ToS/review-ledger/` through review mechanics | acceptance, rejection, ambiguity, counter-reading, and reviewer rationale remain visible |
| authored promotion | `ToS/canon/` | humans own the stronger judgment and every object returns to source |
| derived learning surfaces | `ToS/derived-exports/`, bounded local evals, and stronger downstream owners | graph, retrieval, training, and evaluation views remain projections rather than ToS authority |

No new gold-packet schema should be treated as settled until one real bounded
packet exercises this entire route and exposes its missing distinctions.

## Corpus Evidence Spine

The corpus foundation has a different stability law from semantic canon.

```text
work -> expression -> edition -> item -> immutable file
  -> passage/region anchor
    -> observation
      -> versioned claim
        -> human review event
          -> derived index or graph
```

The acquired bytes, digest, identity record, and review history form a durable
spine. Claims about authorship, edition, structure, lemma, etymology,
translation, sense, concept, and relation remain evidence-bearing and
revisable. Stable identity protects their lineage; it does not make their
content infallible.

Time follows the same law. A Work has no single self-evident year: composition,
manuscript fixation, printing, title-page dating, private circulation, public
release, reception, and later preservation are separate claim facets. The
current first-publication profiles exercise one bounded ordering facet while
retaining intervals, stage boundaries, availability, precision, provenance,
and explicit uncertainty.

### Source-witness topology

`ToS/source-witnesses/` combines a tracked catalog with a speaking physical
tree:

```text
catalog/
places/<place>/
organizations/<historical-organization>/
works/<responsibility>/<work>/expressions/<language-and-responsibility>/
  editions/<edition>/items/<item>/
collections/<responsibility>/<collection>/
chronology/<responsibility>/first-publication/
provision-activities/
```

Collections prevent multi-work volumes from being duplicated under every
contained work. Stable ToS IDs, not paths, own identity, so the tree can be
improved through explicit migrations.

Only the immutable item bytes inside `payload/` are gitignored. The item
manifest, digest, provenance, rights, forensic report, and catalog entry stay
tracked. Working OCR, model caches, benchmarks, and large derivatives route to
the `abyss-stack` laboratory or host-managed storage. Reviewed source-near
text may return to the witness tree when rights permit.

See `ToS/doctrine/CORPUS_FOUNDATION.md` for identity, anchor, sign, claim,
translation, rights, and projection law, and
`ToS/source-witnesses/README.md` for physical routing.

### Replaceable projections

Lexical indexes, embeddings, vector stores, RDF, property graphs, KAG exports,
and visualization databases are readers over stronger tracked records. They
must be reproducible from versioned inputs and may not become the sole copy of
an assertion or review decision.

The first corpus-wide bibliographic reader lives under
`ToS/derived-exports/graph/`. It keeps every claim as a node between its
subject and object, preserves literal objects as literals, and carries exact
routes to evidence, maker, provenance time and method, review posture, source
line, and claim digest on every edge. It is deliberately separate from the
atlas/view graph projection: one reads the source-witness claim spine, while
the other reads the philosophy branch and its review lenses. Neither is a
second owner.

The source spine now materializes the full declared identity ladder through
three claim families: Work `has_expression` Expression, Expression
`embodied_by` Edition, and Edition `exemplified_by` Item. Owner records keep
the speaking structural links and cite the exact outgoing claim IDs; validation
requires exact two-way closure against the claim packets and item manifests.
This deliberate overlap is traceability, not competing truth. In particular,
an embodiment link is bibliographic topology and cannot establish textual or
critical-edition equivalence.

The bounded Nietzsche corpus additionally gives every current Work exactly one
Work-owned `authored_by` packet to the Friedrich Nietzsche Agent. That
corpus-wide closure is documentary and provenance-bound, not a convention
derived from paths, and it does not select an author-final text or flatten
editorial, paratext, translation, or transmission responsibility.

Responsibility evidence may resolve through an Expression-local source anchor,
not only a collection boundary map. The 1913 Antonovsky case binds a
`translated_by` claim to one proposed whole-page title-page anchor and exact
file digest while keeping the Agent identity provisional. A page credit is
bibliographic evidence; it is not accepted text, translation quality,
editorial equivalence across reprints, or a biography of the translator.

Edition-level provision is likewise claim-owned rather than flattened into
publisher/place/date fields. A bounded provision claim groups the exact
transcribed or reported statement with typed activity, role-specific Place and
Organization references, a declared temporal facet, evidence, provenance, and
review posture. A normalized participant is only a claim-originating graph
route: it cannot bypass the claim, replace the literal statement, or equate a
publisher with a printer, successor, or public-release event.

The first *Zarathustra* provision extension is deliberately Edition-exact. The
1883 DTA authority statement belongs only to the separately modeled first
part; the same labels on parts II and III are not evidence for copied claims.
The historical Schmeitzner Corporate Body is modeled as an Organization and
kept distinct from the affiliated Person, later firms, and rights holders.
This is an identity-and-provenance boundary, not accepted bibliography.

The repository-local query route verifies the entire tracked graph against a
fresh source-backed rebuild before returning any selected claim. It exposes
exact AND selectors and returns the source JSONL object plus its complete graph
trace as deterministic stdout JSON. It refuses unbounded implicit dumps and
silent truncation, persists nothing, and does not turn a reader into runtime,
review, or graph authority.

This boundary lets ToS compare graph and retrieval technologies without
changing what owns philosophical meaning.

## Movement

Healthy ToS movement:

1. source, work, concept, relation, or branch pressure appears;
2. the nearest owner surface names the material;
3. witness, review, and structure give it a place;
4. doctrine and mechanics make repeatable movement explicit where repetition is
   real;
5. canon or public/export surfaces appear only after the authority route is
   inspectable.

The result may become graph-readable, KAG-readable, or runtime-visible. It does
not stop being source-owned.

## Naming

Names should carry topology. A folder or file name should tell the next agent
whether it is entering a source witness, branch, doctrine card, canon node,
review note, mechanic part, generated companion, or export seam.

Working labels from tools, sessions, or UI surfaces do not become repository
topology unless they name a real owner boundary.

## Authority

ToS-authored source surfaces own ToS meaning. Generated files, compact indexes,
public examples, validators, owner-local statistics, runtime readers, KAG
exports, and graph views are companions. They help humans and machines navigate
the tree; they do not author the tree. `stats/` owns local questions and
evidence handoffs; `aoa-stats` owns their shared measurement grammar.

When another AoA organ owns the stronger truth, ToS routes to it.

## Principles

- grow from source toward structure;
- prefer branch topology over flat buckets;
- make uncertainty visible without turning it into a warehouse;
- keep mechanics as organs of movement;
- let public and generated seams return to authored authority;
- treat Zarathustra as the first golden growth kernel, not a generic sample or
  universal ontology;
- keep stable evidence distinct from versioned semantic interpretation;
- name small routes as if the future tree will grow through them.

## Failure Signals

- root docs become inventories;
- `ToS/` becomes a shelf for mechanics, tests, or generic process notes;
- generated/exported views start acting like canon;
- branch names come from old session labels rather than owner truth;
- validation pretends to be philosophical authority;
- graph readiness weakens source traceability.
- a filename, model output, annotation-service database, or graph record starts
  acting as the only identity or claim authority;
- source bytes are committed accidentally or a gitignored payload loses its
  tracked fixity, rights, and provenance companions.

## Relationship to AoA

Agents of Abyss is the philosophical-engineering center. ToS is the
meaning-bearing philosophical tree the system ultimately serves.

AoA may provide law, agents, proof, memory, stack, runtime, and downstream
machinery. ToS receives that support without surrendering source authority.
