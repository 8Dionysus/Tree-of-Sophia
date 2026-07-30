# eKGWB Rights and Transport Refresh

Status: current model-authored research assessment; no source admission, legal
clearance, publication decision, or human task

Research snapshot: 2026-07-30

Machine-readable assessment:
`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/rights.ekgwb.za-i-vorrede-1.v1.json`

## Question

May Tree of Sophia use the exact eKGWB locator `Za-I-Vorrede-1` for private
local indexing, comparison, and analysis, and what remains blocked for
sharing, source admission, or a future public server?

This refresh corrects one over-broad earlier formulation. A no-derivatives
license is not the same thing as a ban on every private transformation. The
current CC BY-NC-ND 4.0 legal code permits non-commercial production and
reproduction of adapted material while prohibiting its sharing. That
distinction does not authenticate the currently reachable HTTP response,
admit it as source truth, establish German competence, or authorize
publication.

## I. Official documentation and law

The current [eKGWB documentation](https://doc.nietzschesource.org/en/ekgwb)
describes a citable digital reference edition with stable siglum-based URLs.
The current [Nietzsche Source rights
page](https://doc.nietzschesource.org/en/rights) declares CC BY-NC-ND 4.0 for
the site while also asserting that the Colli-Montinari scientific text is in
the public domain. These statements concern potentially different layers:
the underlying Nietzsche text, the Colli-Montinari scientific edition,
editorial additions, the digital edition, and the website presentation must
not be collapsed into one rights object.

German [UrhG §70](https://www.gesetze-im-internet.de/urhg/__70.html) gives a
25-year term to qualifying scientific editions of public-domain works. This
supports the relevance of publication dates to the public-domain claim, but
it is not a project-specific legal ruling and does not resolve every digital
or editorial layer.

The controlling current license text is the [CC BY-NC-ND 4.0 legal
code](https://creativecommons.org/licenses/by-nc-nd/4.0/legalcode.en).
Section 2(a)(1) distinguishes two permissions:

- licensed material may be reproduced and shared for non-commercial purposes;
- adapted material may be produced and reproduced, but not shared, for
  non-commercial purposes.

The [Creative Commons
FAQ](https://creativecommons.org/faq/#can-i-combine-material-under-different-creative-commons-licenses-in-my-work)
likewise distinguishes private adaptations under 4.0 NoDerivatives licenses
from distribution of adaptations. The earlier ToS statement that the
no-derivatives boundary itself forbids a local transformable research corpus
was therefore too broad.

## II. Established scholarly record

Paolo D'Iorio's 2010 description of eKGWB,
[The Digital Critical Edition of the Works and Letters of
Nietzsche](https://www.uma.es/nietzsche-seden/obra/eKGWB.pdf), establishes the
edition's scholarly design, stable addressing, and then-current CC BY-NC-SA
3.0 posture. It is historical license evidence, not authority for the
current license.

D'Iorio's 2024 account,
[Nietzsche Source: a Scholarly Web
Resource](https://doi.org/10.1590/2316-82422024v4503pd), describes eKGWB as a
freely usable research and teaching resource with exact citation duties,
reports the present BY-NC-ND posture, and repeats the public-domain claim for
the Colli-Montinari text. This is strong scholarly context, but the live
official license and the legal code remain the operative rights evidence.

The current [ITEM project page](https://www.item.ens.fr/ekgwb/) independently
identifies eKGWB as the Colli-Montinari digital reference edition, describes
its scale and stable URLs, and supplies an institutional route back to the
Nietzsche team.

## III. Fresh access and transport check

On 2026-07-30, fresh bounded host checks found:

- `doc.nietzschesource.org` and `www.nietzschesource.org` both resolved to
  IPv4 `134.158.33.187`;
- HTTPS requests to the documentation pages, public site, exact stable
  locator, and exact static-include endpoint failed before an HTTP response
  because port 443 could not be reached from this host;
- the public site root and exact static-include endpoint remained reachable
  over HTTP with status 200;
- the documentation host returned HTTP 503;
- the ITEM institutional page returned HTTPS 200;
- no authenticated exact `Za-I-Vorrede-1` source route was obtained.

The previously captured HTTP response remains repeatable transport evidence,
not authenticated source admission. Repetition and byte identity do not
replace server authentication.

## Decision

| Action | Current posture | Reason |
| --- | --- | --- |
| Preserve public metadata, locator, citations, and rights evidence | allowed | no source payload is published |
| Keep the already captured exact response in ignored local custody | allowed for bounded research | operator policy is local-only and the license supports non-commercial private use |
| Produce private local indexes, embeddings, normalization, comparison, or analysis | allowed as a research route, not admitted truth | CC BY-NC-ND 4.0 permits private non-commercial adaptations; provenance and source-return remain mandatory |
| Treat the HTTP response as an authenticated critical source | blocked | transport authenticity is unresolved |
| Accept German readings, grammar, semantics, or translation quality | blocked | no German-competent human authority has reviewed them |
| Share or publish an adapted eKGWB corpus, index containing recoverable adapted text, or derivative text | blocked | NoDerivatives prohibits sharing adapted material |
| Commercial use | blocked without separate permission | NonCommercial condition |
| Publish even an unmodified source payload on the future ToS site | blocked by current operator policy and review law | only separately reviewed, explicitly authorized materials may be published |

## Access-request effect

The existing request remains unsent. Its useful purpose is narrower now:

1. obtain an authenticated or institutionally supplied copy of the exact
   passage;
2. clarify any intended server-side, quotation, or sharing scope beyond
   private local research;
3. preserve a direct institutional answer if the live license or project
   topology changes.

Permission is not required merely to create a private non-commercial local
adaptation under CC BY-NC-ND 4.0. Sending the request remains a human external
action and has not been scheduled.

## Authority boundary

This is a model-authored research and routing assessment, not legal advice or
human rights clearance. It changes neither the historical critical-witness
packet nor its digest-bound downstream snapshots. It creates no source
content, accepted German unit, translation draft, sign, semantic claim,
publication payload, or human debt.
