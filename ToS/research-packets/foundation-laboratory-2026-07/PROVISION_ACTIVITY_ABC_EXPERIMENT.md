# Provision-Activity A/B/C Experiment

Status: bounded model-run representation experiment with direct source-return
inspection; not human bibliographic review, claim acceptance, graph truth, or
permission to bulk-populate the corpus

Experiment date: 2026-08-01

## Question

Can Tree of Sophia answer Edition-level place, publisher, and statement-date
questions without turning an imprint into an untyped fact, confusing a
publisher with a printer or successor, or losing the exact evidence and claim
that produced the answer?

The frozen cases are:

1. the 1889 Leipzig C. G. Naumann *Götzen-Dämmerung* Edition;
2. the 1908 Leipzig Insel *Ecce Homo* Edition;
3. negative controls for Druckerei C. G. Naumann, the modern Berlin Insel
   successor, invalid Gregorian dates, publisher/printer role collapse, and a
   title-page statement date relabeled as public release.

No source payload was copied or published. The experiment reads tracked
public-safe metadata, source-owned claims, identity records, and the generated
catalog/graph only. All new records remain model-made and unreviewed.

## Compared representations

| Variant | Representation | What was inspected |
| --- | --- | --- |
| A | existing Edition labels and notes | direct record and metadata search for `Leipzig`, publisher strings, and years |
| B | isolated flat candidate fields | an untracked fixture shaped as `place`, `publisher`, and `date` literals over the same two cases |
| C | Edition-owned reified `provision_activity` claim | exact source claim row, Place and Organization identities, generated catalog row, graph trace, normalized-ref query, and negative query |

Variant B was kept outside the repository source tree. It exists only as the
rejected comparison shape; no flat field became authority.

## Manual inspection result

### Raw evidence and claim return

The model directly inspected both metadata snapshots and both source claim
rows, then compared them with the catalog rows and graph query returns.

- *Götzen-Dämmerung* preserves the manifestation transcription `Leipzig,
  Verlag von C. G. Naumann, 1889.` and marks `1889` as a transcribed
  `statement_date`. It explicitly does not turn that year into the separately
  reported printing, receipt, or public-sale chronology.
- *Ecce Homo* preserves the DNB report `Leipzig : Insel-Verl., [1908]` and
  marks `1908` as a catalog-supplied `statement_date`. It does not manufacture
  an exact day, printing date, or public-release date.
- Both claims resolve to `tos.place.leipzig` while resolving to two different
  historical Organization records.
- Every provision edge in the generated graph starts at its reified claim
  node. No direct Edition-to-Place or Edition-to-Organization truth edge was
  observed.

This inspection is AI evidence in the solo+AI workflow. It is not recorded as
human bibliographic acceptance.

### Representation comparison

| Requirement | A: labels/notes | B: flat fields | C: provision claim |
| --- | --- | --- | --- |
| return exact statement and evidence | partial and search-dependent | absent unless extra ad hoc fields are added | exact claim file, line, canonical digest, evidence, and provenance |
| distinguish transcription from authority report | not structurally guaranteed | no | yes |
| share Leipzig without merging activities | string coincidence only | literal equality only | one provisional Place identity through two separate claims |
| distinguish the historical publishers | readable strings only | two strings, no governed identity | two distinct provisional Organization identities |
| distinguish publisher from printer | prose-dependent | no typed role | schema-bound role plus rejected printer shortcut |
| distinguish statement year from public release | prose-dependent | flat `date` is ambiguous | typed `statement_date` plus explicit warning |
| reject modern successor as the 1908 publisher | no exact identity query | no succession posture | exact normalized query returns `no_match` |
| revise without overwriting the Edition | no claim history | field replacement | claim identity, provenance event, version, and review posture |
| graph without asserting a direct fact edge | no governed projection | encourages direct edges | all edges remain claim-originating |

Variant A remains useful for human navigation. Variant B is cheaper but loses
the distinctions that motivated the experiment. Variant C is retained as the
smallest source-returnable foundation shape; this does not authorize a wider
population run.

## Negative controls

A direct one-off inspection, separate from the repository validators, produced
the following outcomes:

| Mutation or shortcut | Result |
| --- | --- |
| add flat top-level `publisher` | rejected as an additional property |
| use `printer` inside a publication activity | rejected by activity/role coherence |
| relabel `statement_date` as `public_release_date` | rejected; that role is not in the contract |
| use day precision with `1889-02-31` | rejected by Gregorian format checking |
| use an interval whose start follows its end | rejected by source-foundation temporal closure |
| resolve publisher to Druckerei C. G. Naumann | rejected identity was not materialized |
| resolve the 1908 publisher to modern Insel Verlag Berlin | rejected identity was not materialized; normalized query returned `no_match` |
| query normalized Leipzig | exactly two provision claims returned |
| inspect provision graph edges | both claims retained; every edge originated at its claim node |

The repository regression tests separately exercise stale generated output,
missing or wrong-type normalized identities, orphaned Edition references, and
digest/provenance drift. Passing those tests does not replace the raw
inspection above.

## Cost and speed snapshot

Five warm local runs were measured on `abyss-machine` with Python wall-clock
time. These numbers describe this exact checkout and are not universal
performance claims.

| Operation | Median | Observed range |
| --- | ---: | ---: |
| rebuild object/claim catalog | 99.02 ms | 98.54-105.14 ms |
| rebuild bibliographic graph | 266.48 ms | 262.97-271.18 ms |
| query both provision claims | 438.80 ms | 404.24-456.43 ms |
| query normalized Leipzig | 421.98 ms | 414.43-478.57 ms |
| negative modern-successor query | 449.62 ms | 414.26-460.24 ms |

The two claims, one Place, and two Organizations changed the generated
surfaces as follows:

| Surface | Before | After | Delta |
| --- | ---: | ---: | ---: |
| catalog objects | 65 | 68 | +3 |
| catalog claims | 99 | 101 | +2 |
| graph nodes | 301 | 312 | +11 |
| graph edges | 627 | 645 | +18 |
| claim catalog bytes | 131,905 | 135,767 | +3,862 |
| graph bytes | 1,185,777 | 1,224,589 | +38,812 |

Human inspection time was not measured because no human review episode was
opened. Inventing a number from model activity would violate the experiment's
authority boundary.

## Decision

Retain C as a bounded foundation mechanic and preserve A as a navigation
fallback. Reject B as the source authority shape. Keep all two provision
claims and all three normalized identities `unreviewed`/provisional. At this
experiment's closure, do not add the optional Chemnitz/Schmeitzner case or
bulk-populate other Editions until a real next question demonstrates that the
two-case contract is insufficient or a bibliographic review explicitly
promotes these records.

## Post-experiment extension — 2026-08-01

The frozen measurements and two-case decision above were not rewritten. The
next concrete question was whether an exact structured authority statement for
the already modeled first part of *Also sprach Zarathustra* could exercise the
retained C mechanics while resisting two label-driven errors: substituting the
Person Ernst Schmeitzner for the historical publishing Organization, and
copying the statement to parts II and III.

Ordered research and a separate discovery receipt are recorded in
[`ZARATHUSTRA_PART1_PROVISION_IDENTITY_RESEARCH.md`](ZARATHUSTRA_PART1_PROVISION_IDENTITY_RESEARCH.md).
The bounded extension admits one unreviewed first-part claim, one provisional
Chemnitz Place, and one provisional historical Schmeitzner Organization. Direct
queries return exactly that first-part claim for either normalized identity;
the part-II, part-III, and Person-GND controls return no match. This is a
post-experiment contract extension, not a sixth timing run, a third member of
the original sample, human review, accepted bibliography, or permission for
bulk population.
