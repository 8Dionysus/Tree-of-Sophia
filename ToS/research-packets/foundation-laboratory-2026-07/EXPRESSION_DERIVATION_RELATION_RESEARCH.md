# Expression-Derivation Relation Research

Date of evidence refresh: 2026-08-01
Scope: source-returnable relations among distinct textual Expressions, with the
Antonovsky 1900/1903/1907/1911/2007 lineage as the first pressure test
Authority posture: model-made bibliographic research and source inspection;
no human bibliography, collation, translation-quality, semantic, or rights
review

## Result first

ToS needs a directed, evidence-bearing Expression-to-Expression relation. A
shared translator, later date, edition number, matching title, or position in
a sequence is not enough to create it. The admitted predicate is
`is_derivative_of`; the subject is the later/derived Expression and the object
is an identified source Expression. Its qualifiers preserve the kind of
derivation, source directness, statement basis, collation status, and the laws
that the relation is non-transitive, asymmetric, irreflexive, and never an
equivalence assertion.

The exact 1911 preface supports two bounded historical claims:

- the Antonovsky 1903 Expression is reported as a revision of the 1900
  Expression;
- the Antonovsky 1907 Expression is reported as a revision of the 1903
  Expression.

Both claims remain `reported`, `unreviewed`, and `not_collated`. Three
provisional Expression identities are therefore materialized before the two
relations. No source text, Edition, Item, or payload is invented for them.

The following tempting edges are not admitted:

- `1911 → 1907`: the preface calls the present object the fourth edition and
  discusses continuity and change, but its exact categorical sentence applies
  to the *three previous* editions. An exact 1907 witness or a stronger direct
  statement is still needed before this edge is asserted;
- `2007 → 1911`: the 2007 copyright page says that the Antonovsky translation
  first published in 1898 was taken as the basis, checked, and newly edited.
  It does not identify the 1911 Expression as the exact source. A contemporary
  review mentions the four pre-revolutionary editions but likewise does not
  establish that shortcut;
- any inferred `1907 → 1900` shortcut: the relation is explicitly
  non-transitive, so the two admitted claims cannot be collapsed into one;
- any equivalence among the 1900, 1903, 1907, 1911, 1913, 1996, 2007, or later
  texts.

## 1. Classical and official documentation

### Exact primary witnesses

Direct visual inspection of the fixity-bound 1911 RSL/RuNEB scan, PDF page 4,
shows Antonovsky's own preface. He names the four editions by year—1900, 1903,
1907, and the present 1911—and says each of the three previous editions was
not a simple reprint of its predecessor but a completely new reworking of the
translation. He also calls the 1907 state the third edition. This supports
distinct Expression identities and the two predecessor relations above. It
does not by itself make the unavailable earlier texts accepted or collated.

Direct inspection of the fixity-bound 2007 local witness, PDF page 4, shows a
different statement: the Antonovsky translation first published in 1898 was
taken as the basis, carefully checked, and newly edited. The visible line is
strong evidence that 2007 is a new Expression, but its source endpoint remains
historically underspecified. No 2007-to-1911 claim is created.

### Bibliographic models

The superseded but still useful [IFLA FRBR final report](https://www.ifla.org/wp-content/uploads/2019/05/assets/cataloguing/frbr/frbr_2008.pdf)
distinguishes Expression-to-Expression revision, translation, abridgement, and
arrangement relations. It says a revision alters or updates content without
necessarily creating a new Work. This is the classical basis for keeping a
revision relation at the Expression layer.

The official [IFLA Library Reference Model](https://repository.ifla.org/handle/123456789/40)
consolidates the FR family. Its Expression derivation relation is many-to-many
and may be refined by type. The current official object-oriented mapping is
more operationally precise.

RDA's current [Expression elements](https://www.rdaregistry.info/Elements/e/)
continue to represent relationships as RDF properties. The current registry
release was `v5.4.13` when rechecked on 2026-08-01. RDA is retained as an
interoperability reference, not copied as the ToS owner vocabulary.

## 2. Established scholarship

Barbara Tillett's [“A Taxonomy of Bibliographic Relationships”](https://doi.org/10.5860/lrts.35n2.150)
(1991) separates equivalence from derivative families. That distinction is
foundational here: a derivation relation records historical dependence, not
interchangeability.

Richard Smiraglia's *The Nature of “A Work”* (2001) develops bibliographic
families and derivation as a way to preserve related realizations without
flattening them into one text. Textual scholarship on multiple versions makes
the same practical point from another direction: edition history is evidence
about change, not permission to choose one unmarked canonical string.

The 2006 *Variants* article
[“Texts in Multiple Versions: Histories of Editions”](https://journals.openedition.org/variants/283)
shows why version history matters to textual and genetic criticism. It is
supporting scholarship, not direct evidence for an Antonovsky edge.

## 3. Fresh official and implementation evidence

The latest official [LRMoo version 1.1.1](https://cidoc-crm.org/lrmoo/ModelVersion/version-1.1.1),
released in November 2025 and still the implementation version on 2026-08-01,
defines `R76 is derivative of` between two Expressions. Its current
[declaration](https://cidoc-crm.org/extensions/lrmoo/html/LRMoo_v1.1.1.html#R76)
states that the object is the source or one of the sources, that the relation
is many-to-many, non-transitive, asymmetric, and irreflexive, and that its type
may be revision, translation, abridgement, or arrangement. LRMoo also says
that a Work may be linked to an Expression even when the historical chain of
source Expressions is unknown. This directly supports a fail-closed rule: an
unknown endpoint produces no invented derivation edge.

The current [TEI P5 `relation` element](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/ref-relation.html)
is version 4.11.0, updated 2026-02-18. Its active/passive/mutual targets plus
`@source`, `@resp`, and `@cert` are a useful serialization comparison, but TEI
does not own ToS's bibliographic predicate vocabulary.

A fresh LRMoo implementation report, the
[Lem Knowledge Graph](https://ceur-ws.org/Vol-4176/swodch-9.pdf), documents a
concrete failure mode: missing derivation relations caused automatic graph
generation to create false Works and excess Expressions, and expert correction
was still required. ToS takes the complementary conservative route: it admits
an endpoint only when the entity is materialized and an edge only when the
source relation is evidenced. Edition order and labels never fill the gap.

The DHQ study
[“Digital Editions and Version Numbering”](https://dhq.digitalhumanities.org/vol/14/2/000455/000455.html)
also warns that edition history and software/data versioning are separate
problems. ToS therefore does not confuse historical Expression derivation
with `record_version`, Git history, or a superseding metadata record.

## 4. General web last

Only after the official, classical, established, and fresh routes were
checked, exact-year searches were run for the Antonovsky third edition. They
located an academic citation to *Так говорил Заратустра*, third edition,
Saint Petersburg, 1907, pages 353–359, and reported that the copy belonged to
Alexander Blok's library. No freely downloadable exact 1907 digital object or
resource-specific redistribution license was found.

Marketplace listings, modern transcriptions, reprints, and publisher cards
were rejected as endpoint identity, fixity, textual equivalence, and rights
authority. The existing discovery protocol remains the method owner. A
public-safe, unsent institutional request route is prepared for shelfmark and
copy clarification; no message is sent without human approval.

The exact incomplete run is recorded at
`ToS/source-witnesses/discovery/runs/antonovsky-1907-revision-witness.2026-08-01.v1.json`.
The public-safe request state is
`ToS/source-witnesses/access-requests/public-ledger/antonovsky-1907-blok-library.access-request.json`:
it asks first for the shelfmark and identity, then for a lawful research copy
and bounded local computational uses. It is `draft-not-sent`, contains no
personal correspondence, and requests neither source redistribution nor
publication of the source file. The institutional route and its temporary
summer closure must be refreshed before a human-authorized send.

## Contract and admission boundary

The public `expression-derivation.schema.json` owns only qualifiers for a
generic evidence-bearing claim packet. The source claim remains authoritative;
the generated catalog and graph remain projections.

An admitted relation requires:

1. two distinct materialized Expression records;
2. one directed `is_derivative_of` claim from derived to source Expression;
3. same-Work closure for this v1 profile;
4. source-returnable evidence and a provenance event;
5. an explicit derivation kind and directness;
6. a declared statement basis and collation status;
7. non-transitive, asymmetric, irreflexive, and no-equivalence laws;
8. explicit review and visibility posture.

It does not establish textual identity, accepted source text, completeness of
the historical chain, translator or editor responsibility for every reading,
translation quality, etymology, semantics, signs, concepts, rights, canon, or
human review. Green validation proves only shape, endpoint closure, source
return, fixity-bound provenance, and projection parity.
