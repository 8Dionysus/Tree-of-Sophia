# Native TextUnit return and assessment: bounded review, 2026-09-08

Status: implemented native return and local assessment adapter within
Foundation L02. Not a finished language/text profile, an Occurrence writer,
real linguistic admission, public graph/UI integration or deployment.

## Owner boundaries

[Corpus Foundation](../doctrine/CORPUS_FOUNDATION.md) owns native textual
identity/layer law; [Knowledge Assessment](../doctrine/KNOWLEDGE_ASSESSMENT.md)
owns competence-scoped agent or human judgment. The
[binding schema](../contracts/native-text-unit-binding.schema.json) pins one
existing packet, selected segmentation/unit and ordered anchors to their frozen
text layer and explicit bibliographic scope. The
[assessment-view schema](../contracts/native-text-unit-assessment-subject.schema.json)
and [growth-cycle contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#native-textunit-return-and-assessment)
define the executable adapter, without replacing native source records.

The resolver checks metadata by default; exact UTF-8 reading is a separate
explicit choice. It checks source-scope/Item/File/anchor/layer/rights closure,
fixed layer predecessors and policy/configuration dependencies, exact span
hashes and declared coverage without changing newlines, Unicode or coordinates.
It does not read the original Item payload, rerun extraction, execute source
instructions or evaluate linguistic quality. Metadata-only success cannot
claim byte verification. Private access requires the owner-selected scope;
public availability requires the existing recorded exact gates, not a newly
inferred permission.

V3 source-owner configuration binds selected native units to the existing
journal. Its subject ID/version remain native; the canonical assessment digest
describes the versioned adapter view rather than raw packet bytes. All packet
fields survive. Separate layer evidence shares the same origin and cannot
become circular or independent corroboration. Metadata-only inputs cannot
append through the v3 adapter, including as another target's evidence. Native
historical human-review fields and original proposed/unreviewed statuses remain
unchanged; qualified new agent events use the separate existing journal.

V3 rechecks protected configuration bytes and selected dependencies after
acquiring the subject lock and immediately before publishing a new head. A
late refusal may retain an orphan blob but not a visible partial history.
The issuer must still keep source inputs stable; this is not a filesystem
transaction or isolation from hostile same-UID code. V1/v2 semantics remain.

## Real-source return

The retained DTA German first-paragraph machine layer and its existing
first-sentence proposal were resolved, without editing either:

- Packet `source-text-unit.za-i-vorrede-1-p1.dta-sentence-proposal.v1.json` in
  the Zarathustra `gold-sets/foundation-pilot-v1` source home.
- Unit `tos.text-unit.sid-291ea362adcc409b9d981b76bc608b6d`, version 1.
- Segmentation `tos.text-segmentation.sid-851802e194c243c9a2320f5d305b4ea2`,
  version 1; the selected unit is a sentence proposal, not a lexical token.

The existing exact text representation was verified in owner-local mode;
metadata-only mode did not claim that verification. Native statuses remained
`method_proposed`, `proposed`, `unreviewed`, with `local_only` effective
visibility and no original-payload or public-content verification claim.
No source text, selectors or short-span hashes are reproduced here.

A separate real `describe` then `inspect` pass used v3 protected local
configuration with no grant, competence or execution-profile assertion.
Both modes returned empty history, no current admission and the unchanged
native statuses. Metadata mode discovered only describe/inspect; exact mode
also discovered append grammar without granting authority to use it. Both
returned eight public contract refs without private packet/layer paths or
source text. Measured local preparation plus both commands: 0.721496 seconds
metadata-only, 0.735106 seconds exact owner-local. These are single bounded
canaries, not latency budgets, UI timings or corpus scaling acceptance.

Local reproduction helpers are retained with the task's private source-owner
configuration in `.git/tos-foundation-native-unit-20260908/verify_existing.py`
and `verify_assessment.py`. They require the unchanged retained DTA layer;
they do not fetch missing private bytes or append a review. Portable API and
CLI preparation are documented in the linked mechanic contract.

## Automated verification and review

- Resolver: 27 tests, 10.473 seconds. Synthetic CRLF/NFD and nonzero-coordinate
  fixtures preserve byte identity; private access, false public flags, unknown
  contracts, invalid source/anchor identity, symlinks/escapes, changed inputs,
  budgets and fabricated native human review are negative controls.
- Native assessment integration: 18 tests, 26.614 seconds. Positive synthetic
  agent research admission and exact replay do not rewrite native statuses;
  revocation changes current admission without erasing the old receipt.
  Changes to packet, layer, rights, content, schema or protected configuration
  after blob creation do not publish a head. A changed input under lock also
  prevents an old replay from returning as current success.
- Existing assessment engine/journal: 62 tests, 9.472 seconds.
- Existing human-form materialization: 22 tests, 0.148 seconds; topology
  checks: 26 tests, 1.444 seconds after naming the mechanic's actual README as
  the native assessment test owner (the schema remains its source contract).
- Source-foundation validation passed, including optional present-byte fixity;
  source-home validation passed. These are mechanical, not linguistic verdicts.
  Cross-corpus documentation currentness and context guards also passed after
  rebuilding their owned companions.

An independent bounded helper authored the native integration tests and
reviewed production boundaries. It found a schema-role accounting defect when
a schema was read first as fixed support and later from cache as a grammar.
The schema still entered opaque fixity but was missing from exposed consumed
contracts. The shared reader now retains both roles regardless of cache order;
the exact regression went RED then GREEN. The helper's final 18-test run passed
in 22.519 seconds and reported no other actionable finding. Root separately
ran the integration suite and inspected both test files and the changed code.

Review checklist: source return, native versus derived identity, historical
review preservation, text/layer distinction, explicit access, separate
authority/competence and same-origin evidence are preserved. No rights,
publication, canon or semantic transition is inferred from mechanics. No
source migration, source byte replacement or historical review rewrite was
performed. Removing this new reader does not erase sources or journal history.

Next owner: ToS language/text growth for exact Occurrence binding and a
bounded native token-unit writer, followed by competent source-visible
assessment and lossless private/public consumer routes. Full graph/access,
UI, Worker/D1, CI, merge and deployment are not established by this slice.
