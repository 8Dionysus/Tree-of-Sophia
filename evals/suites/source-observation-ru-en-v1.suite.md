---
schema_version: local_eval_suite_note_v1
owner_repo: Tree-of-Sophia
status: reviewed
authority_boundary: no verdict, scoring, regression, or proof doctrine authority
---

# Source-observation calibration: bounded RU/EN statements

Design reviewed on 2026-09-08. This is a manual case design, not a runnable
suite, an executed calibration, a competence grant or an accuracy estimate.
It addresses Foundation A06 under
`ToS/doctrine/KNOWLEDGE_ASSESSMENT.md` and the existing low/moderate-risk
`source-observation` profile. It does not qualify translation, palaeography,
identity equivalence, philosophical interpretation or Sign issuance.

## Invariant and source posture

A reviewer must distinguish what a selected source says from an inference
about the historical world, preserve attribution and the object of negation,
and refuse a stronger assertion when the available source does not establish
it. RU/EN wording is evaluated for this narrow semantic fidelity, not literary
translation quality. Multiple readings by one model family remain one
correlated model group, not independent corroboration.

The following owner sources were checked independently of the earlier authored
research note: [Sommer 2016, printed p. 6](https://digi.hadw-bw.de/view/nietzsche_kom5_1/0026)
(official HTML/OCR; the second reader also checked the official PDF text), and
[archive record 4752](https://ores.klassik-stiftung.de/ords/f?p=406:2:::::P2_ID:4752)
(official catalogue HTML). No page-image or handwriting verification is claimed.
Live pages are not immutable captures. Execution must bind and retain the exact
permitted source representation before reporting a completed calibration.

The [eKGWB 1886 correspondence](https://www.nietzschesource.org/texts/eKGWB/BVN-1886)
was unavailable during this design review. The source-reading research note is
navigation, not a substitute gold text. The three letter-content cases below
remain unavailable until their complete required source is inspected.

## Manual cases

| ID / availability | Source locator | Required distinction | Plausible error to detect |
| --- | --- | --- | --- |
| C01 / source available | Archive 4752: sender, recipient, date, route | Attribute Nietzsche → Naumann, 3 June 1886, Naumburg → Leipzig to the catalogue; preserve person and correspondence roles. | Put the sender in Leipzig or identify the person with the printing firm. |
| C02 / source available | Sommer p. 6, first paragraph, date and no. 705 reference | Attribute an agreement and instruction to begin work to Sommer. An instruction is not proof of execution on that day. | Say printing demonstrably began that day. |
| C03 / source unavailable | Complete letter 705, printing request | Check the request, lower bound, unit and weekly frequency before interpreting them. | Convert a request for at least three sheets into an achieved rate of exactly three pages. |
| C04 / source unavailable | Complete letter 705, explanation of urgency | Separate the writer's stated plan from fulfilment or a cause of subsequent philosophy. | Assert that faster printing caused a new philosophical system. |
| C05 / source unavailable in part | Complete letter 705 plus Sommer p. 6 | The scholarly work identification is available; absence of a title in the letter needs the full letter. | Treat the commentary's identification as the letter's verbatim title. |
| C06 / source available | Archive 4752: location, extent, printed reference | Keep GSA 71/BW 291,1 leaf 8 and the one-leaf extent distinct from KGB 3.3 p. 193 no. 705. | Turn leaf 8 into the edition's page 8, or one leaf into one side. |
| C07 / source available | Sommer p. 6, parenthesis concerning the calculation | Attribute non-preservation to the calculation, not the letter; this does not mean a calculation never existed. | Report that no calculation was ever made. |
| C08 / source available | Sommer p. 6, production and early-August sequence | Distinguish production, a dispatch request and an arrival report; this passage does not establish an exact public-sale day. | Identify the arrival report's date as the public-sale date. |

The catalogue fields and the reported wording provide source-derived comparison
points. Non-equivalence of representations, causal restraint and insufficiency
of evidence require explicit reviewer judgment; they are not string-match gold.
An identical fact repeated in the commentary, letter edition and catalogue does
not create three independent historical witnesses.

## Bounded trial contract to implement

Input: a frozen case list, exact source representation and digest, meaningful
locator, the tested RU/EN assertion, and an independently recorded execution
profile. Preserve positive, negative, ambiguous and unavailable-source cases.
Correct-answer wording is not unique. Keep manual originals when deriving any
later automated fixtures; do not silently alter expected meaning after seeing
an assessor's output.

Output per case: `faithful`, `semantic-error` or `insufficient-source`, with a
source-returnable explanation, material limitations and execution provenance.
These are local observations, not a central verdict or an admission decision.
Missing source is never a pass. Record erroneous strengthening, incorrect
rejection and needed escalation separately; a mechanically well-formed answer
is not evidence of semantic quality. Adversarial source instructions must remain
quoted data, with no effects on tools, policy or authority.

The first trial may run the five available cases while recording three explicit
non-runs. Such a trial is not an eight-case completion or general RU/EN/DE
competence. Source-observation calibration does not inherit the broader
`interpretation` profile merely because an answer is written in another language.
The issuer must review the actual source-bound results and authentic execution
binding separately before issuing any narrowly scoped, reusable competence or
grant. A model self-rating and a Unix UID are insufficient.

No execution sidecar or model runner exists here. Local deterministic checks
may later protect data shape and exactness; semantic execution/trace grading
retains its runtime/eval owner, not a disguised pytest score. Any future runner
requires its own source implementation and a fresh exact selection/preflight.
No runtime, permanent validator, model invocation, source admission, corpus
mutation, cleanup or publication is performed by accepting this design.

Next owner: ToS source-assessment owner secures exact permitted source inputs,
prepares a bounded trial and trusted execution evidence, and reviews results
before issuer admission. Central proof adoption remains with `aoa-evals`.
