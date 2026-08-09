# ToS Script Topology

Scripts in Tree of Sophia are command-plane organs for source-first
philosophical growth. They build generated companions, validate route
boundaries, run release lanes, and provide small deterministic helper contracts.
They do not create philosophical authority, runtime policy, proof verdicts, or
graph service truth.

Machine-readable script coverage lives in
[`script_inventory.json`](script_inventory.json). It includes every tracked
non-pyc file under `*/scripts/*`, including local script route cards and
skill-local helper scripts.

## Command Authority

Blocking command sequences live in
[`validation_lanes.json`](validation_lanes.json). The script inventory is
descriptive and testable: it proves each active script surface has an owner
route, source truth, read/write boundary, lane posture, CI inclusion, and test
target.

Inventories describe script surfaces. They do not store release command order
and do not promote advisory helpers into hard gates.

## Inventory Fields

Each entry records:

- `path`
- `family`
- `organ_lane`
- `owner_surface`
- `source_truth`
- `reads`
- `writes`
- `side_effects`
- `validation_lane`
- `ci_inclusion`
- `test_target`
- `disposition`

## Script Families

| Family | Owns | Boundary |
| --- | --- | --- |
| `script_route_card` | Local route guidance for `scripts/`. | Covered by route-card validation and script topology; not a command sequence. |
| `projection_builder` | Generated/read-model writes from source surfaces. | May write tracked generated companions; must not define source meaning. |
| `projection_helper` | Shared builder library code. | Library only; command posture comes from callers. |
| `projection_validator` | Generated/read-model parity checks. | Compares projections against source and builder expectations; does not own source meaning. |
| `source_validator` | Source-home, domain, route-card, intake, canon, or mechanics checks. | Validates source-owned boundaries without becoming doctrine. |
| `compatibility_builder` | Public compatibility mirrors and public-safe examples. | Writes mirrors only from canonical source routes. |
| `compatibility_helper` | Shared compatibility mirror code. | Library only. |
| `lane_loader` | Validation lane manifest loading and checking. | Loads command authority; does not own lane meaning by itself. |
| `release_entrypoint` | Release lane execution. | Runs command sequences from the lane manifest. |
| `mechanics_local_runner` | Discovery of mechanic package-local and part-local tests, builders, and validators. | Runs only source-discovered mechanics homes; does not own mechanic meaning. |
| `skill_local_contract_tool` | Deterministic helper contracts shipped with local agent skills. | Advisory/local-only; not ToS release authority, runtime policy, or hidden hard gates. |

## Root Scripts

Root `scripts/*.py` currently own repo-wide builders, validators, release
execution, lane loading, mechanics-local discovery, and shared helpers. Root scripts
may be mechanics-owned by `owner_surface`, but a root location does not make the
script repository-wide truth.

The source-witness pair is deliberately split: `build_source_witness_catalog.py`
projects tracked corpus identity records into a navigational catalog, while
`validate_source_witness_foundation.py` checks schemas, reference closure,
companions, catalog parity, and any locally present payload bytes. Neither tool
can certify bibliographic truth, OCR quality, rights clearance, translation,
semantics, or human acceptance.

Explicit-local structure builders remain separate from that release-safe
validator. The Mysl transfer-target builder reads one exact ignored PDF only
when given a payload root, preserves independently resetting *Genealogie* and
*Antichrist* number series, and writes text-free proposed anchors and target-only
crosswalks. Release validation checks their tracked schemas, digests, frozen
transfer-frame closure, and zero-authority gates without requiring the book.

The German transfer-source builder follows the same split while preserving a
more demanding witness boundary: *Genealogie* uses one exact local PDF, whereas
*Antichrist* binds a Commons DjVu address witness to a separate Internet
Archive DjVuXML navigation Item through a bounded two-page relation. Its
tracked outputs contain 140 proposed source addresses and no prose. A second,
payload-free builder intersects only `series:unit` keys and composes twenty
possible German structural routes for all twelve frozen candidate pages. The
release validator proves inventory, rights, anchor, digest, provenance, label,
and route closure; it explicitly cannot prove accepted text, exact passage
ends, source-target passage alignment, translation, eligibility, gold,
semantics, canon, or human judgment.

The transfer target-passage builder is the next deliberately local-only layer.
It keeps the twenty frozen pages and thirty-five conservative routes unchanged,
then slices each expected target numbered unit from its label to the next
same-series label in the exact Mysl PDF bbox layer. Thirty-five private mode-0600
files stay ignored; tracked geometry, counts, digests, anchors, and provenance
retain thirty-two real page intersections and three nonintersection negatives.
Its release validator proves that closure without reading private text. Layer
exactness is not diplomatic or accepted Russian, source-passage evidence,
alignment, target gold, eligibility, semantics, canon, or human judgment.

The companion transfer source-passage builder returns to the exact German
layers but does not pretend that every structural route has a recoverable text
boundary. It materializes twenty-eight private mode-0600 candidates only where
both numbered labels resolve inside one named ABBYY, DjVuXML, or PDF-bbox
layer; seven routes remain explicit unresolved-boundary records. Tracked data
contains geometry, digests, witness relations, counts, anchors, and no source
strings. The validator separately preserves the Antichrist Commons-address to
Internet-Archive-navigation two-page relation as navigation only, with no
textual-identity claim. Neither builder nor validator accepts German, aligns a
source passage to Russian, opens eligibility or gold, schedules human work, or
creates semantic, publication, or canon authority.

The catalog/validator pair also owns exact mechanical closure for the three
authored identity-ladder claim files under `ToS/source-witnesses/relations/`.
It must reconcile Work→Expression, Expression→Edition, and Edition→Item claim
IDs against their record fields and item manifests, and must reject any attempt
to treat bibliographic embodiment as textual equivalence.
It also closes each currently declared Work chronology reference against the
dedicated first-publication object contract and digest-bound batch event. This
checks ordering mechanics only; it cannot certify a universal Work date,
historical completeness, or accepted chronology.

Catalog v3 additionally projects the bounded Place and Organization families
used by Edition-owned `provision_activity` claims. The validator keeps the
literal statement, activity kind, role-specific participants, temporal facet,
normalized identity, Edition reference, evidence, and provenance in exact
closure. This mechanics cannot promote an authority match, equate a publisher
with a printer or successor, or turn a statement date into public release.

The source-witness bibliographic graph pair is a downstream generated route,
not a second source validator. Its builder and validator read the public-safe
catalog plus exact claim, evidence, anchor, and provenance records, keep every
assertion reified as a claim node, and reject any edge that loses source return.
They do not emit direct subject-object truth edges or own graph runtime
behavior.

Normalized provision participants are emitted only as claim-originating
`has_normalized_place` or `has_normalized_agent` edges. Querying one of those
identities remains source-return navigation through the reified claim, not an
Edition-to-identity fact assertion.

The companion bibliographic graph query script is a deterministic, read-only
stdout reader. It verifies a complete source-backed rebuild before applying
explicit AND selectors and returns the exact source claim plus complete trace
bundle. It rejects selector-free requests and silent truncation; it does not
write query artifacts or create review, relation, runtime, or service
authority.

The current mechanics-local homes are:

- `mechanics/agon/parts/threshold-registry/`, where the part owns its builder,
  validator, generated registry, and test;
- `mechanics/boundary-bridge/parts/derived-kag-seam/`, where the part owns the
  bounded KAG export generator and validator while generated payloads stay in
  `ToS/derived-exports/`;
- `mechanics/boundary-bridge/parts/public-mirror-sync/`, where the part owns
  public mirror sync scripts while mirror payloads stay in
  `ToS/public-compatibility/`;
- `mechanics/relation-weaving/parts/graph-promotion/`, where the part owns the
  relation-pack promotion validator while canonical relation payloads stay in
  `ToS/canon/relations/`;
- `mechanics/experience/tests/`, where the Experience package owns boundary
  contract tests that span several Experience parts.
- `mechanics/questbook/scripts/` and `mechanics/questbook/tests/`, where the
  Questbook package owns obligation and dispatch compatibility validation while
  root `QUESTBOOK.md` and `quests/` remain public source records.

`scripts/run_mechanics_local_tests.py` discovers these local homes and runs the
related checks.

## Skill Helper Scripts

`.agents/skills/*/scripts/*.py` helpers are deterministic contract tools for
local skill material. They can model dry-run, readiness, and risk contracts, but
they do not become ToS runtime policy enforcement or release blockers unless a
future owner decision explicitly promotes one concrete check.

## Promotion Rule

A script may move from advisory/local-only into a blocking lane only when a
current owner surface and decision record prove that Tree of Sophia owns the
checked behavior. Until then, runtime policy, MCP service authority, graph UI
caches, eval verdicts, federation, and skill execution remain route-only or
sibling-owned.
