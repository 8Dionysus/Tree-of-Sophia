# Document identities and declared source relation reader

Date: 2026-09-07. Parent: `9fc4da965f131e7dda7dac6a1a17886d693e37cf`.
Scope: source-owned document/letter metadata and extensible identity Claim
reading through the existing catalog, projection and access contracts.

## Source and boundary review

Document is an intellectual identity independent of Work, a physical artifact,
a digital representation or a catalog record. Letter specializes Document.
Its ID persists through descriptive corrections; sender, addressee, author,
dispatch, date, content and carrier are not silently inferred metadata.
The existing authored-by relation expands to Document without duplicating its
predicate or altering the meaning of existing Work attributions.

The existing relation registry now declares a bounded `source_claim_profile`.
One `source-claims.jsonl` stream and the `identity-relation-v1` reader serve
concrete evidence-bearing predicates with exact local schema versions. No
registry selects executable code, remote schema retrieval, write capability,
assessment policy or admission. The shared Claim envelope preserves source
fields, counterevidence and unknown extensions. Claim, subject and target
identities stay distinct. Concrete inherited domain/range and source layers
are checked; Thing-to-Thing, unresolved endpoints, literal objects and private
source visibility are refused by this reader.

Five new relations distinguish sender, addressee, physical carrier,
historical-document association and scholarly identification of a document's
work topic. Neither addressing nor authorship establishes receipt, reading or
influence. Carrier attribution does not grant autograph status, identity of
copies, custody or rights. The provisional source flag cannot replace scoped
assessment. Historical/bibliographic adapters retain their prior contract;
the general writer and assessment reader for these new Claims are not yet
implemented. No actual historical source or assessment receipt changes here.

Shared base schemas avoid repeating common source fields in each new profile.
They are reusable structural components, not independently admitted records.
Incompatible predicate/reader reuse and removal of historical routes/layers
are rejected against a previous registry; compatible extension requires
version progression. All source schema resources remain owner-local and
bounded. Source-command preparations bind consumed Claim profile dependencies.

## Executed evidence

- Test-first document creation initially failed because no Document/Letter
  profiles existed. Both now use the existing generic source-owner command,
  retain exact source fields and source-copy forms, and read as intellectual
  Document identities rather than Work or Artifact.
- Test-first synthetic new Claim predicate initially did not enter the
  collector. It now traverses source, catalog, reified graph and access focus
  in both directions without a Python branch for the predicate. Its complete
  source body, unknown fields, exact schema route and dependency hashes persist.
- Eight positive documentary domain/range cases and negative sender/author,
  document/carrier and historical-situation cases pass. Additional negatives
  cover unsupported schema/predicate/layer, missing endpoints, nonidentity
  values, empty evidence, fabricated accepted status, duplicate JSON fields,
  abstract/non-evidence predicates, generic domains, nonidentity ranges,
  executable descriptors, remote schema routes and incompatible evolution.
- All 40 source-command/revision tests passed in 18.682 seconds. The existing
  declared metadata extension test also passed after shared schema resolution.
- All 56 graph tests passed in 65.155 seconds after projection regeneration.
  An earlier broad run overlapped stale generated inputs and failed exact
  currentness checks; those checks were preserved, not relaxed.
- An additional actual Document/Letter-contract synthetic integration passes
  through ordinary catalog/graph/access, with negation, uncertainty,
  counterevidence and exact source records retained. Its first run referenced
  evidence absent from the temporary fixture; correcting the fixture to a
  present source path passed in 0.394 seconds, without production changes.
- Independent old/new JSON comparison: every one of 715 nodes, 1,327 edges and
  196 Claim traces is identical. Only input digests and projection fingerprint
  change. Existing authored sources and catalog bytes remain unchanged.

- Final access knowledge contract: 51 tests passed in 24.377 seconds; corpus
  index: nine tests in 30.703 seconds; topology: 26 tests in 2.357 seconds.
  Corpus index rebuild/validator, source-home validator and diff whitespace
  check passed. Generic predicate catalog discovery passed with the full
  descriptor retained (3.278 seconds). One mistyped topology filename ran no
  tests; only the subsequent actual topology modules count as validation.

Timings are local test observations, not a scaling budget or performance
claim. The full source-foundation lane was not repeated: its known private
payload prerequisite remains separate and was not bypassed. No hosted CI
or release validation was executed for this checkpoint.

## Disposition and remaining work

Source traceability, authored/derived separation, identity stability,
non-executable data, context retention and source-language separation: yes.
No rights, canon, lived-witness consent, agent competence or human signature
is acquired by these schema/reader checks. The existing extensible profile
decision TOS-D-0044 remains the rationale; no parallel registry is created.

The reader currently supports identity relations, not all value/temporal/role
profiles. Creating a metadata record does not authorize Claim creation.
General Claim publication/revision, assessment/admission and real letter,
person and carrier source material are the next ToS/growth owner work.
No full nine-profile completion, UI acceptance, D1 consistency, CI, merge,
deployment or external artifact admission is asserted.

Rollback must retain source records and history. A reader without these
profiles must visibly reject their source route, not drop fields or coerce
documents into Work. Publish new source instances only with their declared
catalog/schema/graph consumer. This checkpoint contains no such real instance.
