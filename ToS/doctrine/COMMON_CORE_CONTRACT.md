# Common core contract

This source owns the shared reference, qualification, map, result and change
contracts. Domain profiles supply their own carriers, semantics, source bindings
and warrants. No universal logic, metric, vector representation or truth algebra
is selected here. Mathematical validity, source applicability, scoped admission
and executed runtime evidence are separate claims.

## Records, occurrences and qualified content

A record's stable key preserves its owner-scoped identity; an exact version is
immutable. `ExactRecordRef = (StableId, RecordVersion, Digest256)` retains its
existing foundation ABI. Resolve exactly or return a typed missing, restricted,
unsupported or stale outcome; never substitute latest. A typed reference adds
an exact profile/registry binding and owner-derived kind validation. Parsing an
ID or recognizing its prefix does not validate its kind or authority.

Same record, same exact version, same bytes, same referent in a scope, equivalent
meaning and isomorphic mathematical structure are different relations. A name
correction need not create a new referent; byte equality does not merge records.
Canonical-record digests and raw-byte fixity name different profiles. A digest
alone neither provides the bytes nor proves replay or collision impossibility.

`OriginAddress = (exact source, exact locator scheme, locator, namespace?)`.
Namespace is required when locator meaning depends on it. An address in one
version does not move to another without an explicit alignment. Native text
Occurrence, worksheet row, assertion event and classification lineage retain
their respective profiles. Work, Expression, Edition, Item, File, Artifact and
scholarly Composite remain distinct even when each acts as a consulted source.

`QualifiedContent = (signature/predicate, typed arguments, context, scope,
modality, qualification profile)`, with exact versioned bindings. Assertion
sign is separate from content, source review and admission. An assertion act
names its maker/attribution, content, basis and context. An interpretation names
its source addresses, interpreter, reading, assumptions and alternatives. A
formalization binds an interpretation to a selected theory/specification and
map. These layers may branch or remain incomplete; missing does not mean false.

Context retains every parameter needed by the selected question. World-valid,
source creation/publication, system/transaction, attributed-knowledge and query
perspective times are separate roles. Place identity, geometry/frame, name and
historical affiliation are separate roles. Current means a supplied selected
cut, not the executing host clock.

Scope names the domain, family and question. Coverage names its declared domain,
covered part, mode, basis and limits. A source's claim of completeness is not an
accepted completeness attestation. Correlated alternatives require a joint
carrier Omega or an equivalent constraint representation: marginals generally
lose correlations. No explicit exponential enumeration is required, but the
representation must retain distinctions needed by the observable.

## Specifications, maps and information preservation

A specification names signature, axioms, admissible model class and semantics.
An instance interprets the typed carrier and symbols under those conditions.
An attributed assignment from an object to an element/region/interval is a
separate, possibly partial or multivalued, source binding with its own warrant.

A partial function `f: D_f subset X -> Y` differs from a relation. Totality,
uniqueness, computability, preservation and reflection are separate promises.
Every map names exact source/target specifications, domain, direction and the
observable/law it preserves. Order, topology, distance and source address are
not interchangeable structures.

For partial functions,
`dom(g o f) = {x in D_f : f(x) in D_g}`; composition is associative on that domain.
For relations, `R;S(a,c)` requires one common intermediate `b` with both edges.
Coarsening different intermediates to one label can manufacture paths. A path
is not automatically influence, identity, responsibility or source support.
Semantic composition may additionally require one common world, context, scope,
criterion and validity witness for the entire path.

A query `F` can be computed exactly from summary `S` precisely when
`S(x)=S(x') implies F(x)=F(x')` on its declared domain. Definedness must also
factor for partial queries. Otherwise retain more information, narrow the
question or return a declared approximation/alternative set. A witness for one
answer is weaker than a universal factorization or exhaustive answer.

A theory bridge names sentence translation and backward model reduct. A target
model must reduce to an admissible source model satisfying the source premises;
satisfaction preservation is required for every transported conclusion. Model
lifting/coverage is an additional obligation for reverse or existential claims.

Modules glue only when their common typed interpretations agree, possibly after
an explicit coherent isomorphism. Separate satisfiability is insufficient.
Chosen bijective bridges must satisfy identity and cycle coherence. Pairwise
compatible edges/alternatives need not have one joint witness. Common symbols
have one definition owner; private symbols remain disjoint.

Preserving one answer, sentence conservativity, expansion of every old model
and unique definitional expansion are different extension promises. A new total
function defined by a graph requires existence and uniqueness throughout its
domain; otherwise use a partial function/relation or establish new assumptions.
Additive schemas and append-only records alone prove none of these promises.

## Provenance, evidence and closure

Provenance retains exact used/generated/quoted/translated/interpreted/derived/
revised bases. Reachability is not semantic support. Direct source incidence,
interpretation, rule result and abstract upper bound have distinct tags. No
single confidence/provenance algebra is imposed on all queries.

Signed support uses an explicit exact proposition/scope/sign adapter. Omitted
sign remains unclassified. Nonempty plus/minus incidence can project to FOUR
support bits in that profile; Neither refers to that selected basis, not the
whole historical corpus. Both after coarse projection need not be disagreement
about one exact proposition. Counts are neither source independence nor votes.

Fixed total monotone inflationary rules on a finite lattice have a least rule
closure. Exact step and endpoint projection require constancy of the respective
step/closure observable on summary fibers. Given the actual adjunction
`alpha left-adjoint gamma` and `alpha gamma = id`, `H_best = alpha G gamma` is
the least one-step sound upper transformer; its closure is an upper result,
not discovered source evidence. Single-active-assignment states need not form
a lattice or realize every ambient abstract support state. Never import this
adjunction or a join from a carrier that the application does not have.

For fixed positive finite-premise rules on powersets, closure preserves directed
unions; a positive consequence has a finite premise witness. Compact generated
closures may themselves be infinite. Effective enumeration requires separately
specified codes and enumeration. Finite termination, negative membership,
complete historical lists and general model compatibility do not follow.
Absence guards, latest selectors and deletion are outside this positive
continuity promise.

## Change and reuse

History-prefix growth, active-base inclusion and admissible-model refinement
are different orders. A correction appends an immutable event and may reduce
current support. Classification lineage differs from source origin and Claim
identity. A selected event protocol names expected old state, unique event ID,
valid target and stale/duplicate handling. Multiple readings may share one
origin without sharing a classification lineage.

Under single-active-assignment semantics, deleting one token clears a support
bit only when its old count was one. Reclassification updates both addressed
coordinates atomically. Delete is not negative append. The new meaning is
closure of the selected current base. Old closure alone cannot reconstruct
retractions or distinguish direct from derived support. For append under a
fixed closure, `C(x join d)=C(C(x) join d)`; deletion has no such default law.

Dependency-closed reuse requires a sound complete signature for the particular
observable, including exact inputs, profiles/maps/rules, context, selected
universe, methods, policy, active selectors and relevant absence/coverage.
Equal incomplete pins do not justify reuse. Completeness evidence must bind the
actual observable, cut, manifest, capture procedure and limits independently
of the manifest's own coverage string. Existing typed/navigation dependency
indexes may be reused only in their supported grammar and observable scope.
A new run ID or certificate ID alone is not a value-affecting dependency.
Fresh access, rights and scoped admission remain separate from historical reuse.

## Result and bounded K0 implementation

A result names run, operator, selected inputs/context/dependencies and independent
coordinates `V,A,E,X,K,W`:

- V: Produced, Undefined with domain evidence, declared Partial with coverage,
  or NoCompleteValue. A codomain value such as NoMatch may be Produced.
- A: applicable, failed precondition or unresolved applicability.
- E: not relevant or an explicitly scoped semantic assessment/reference. A
  computed singleton image does not itself certify universal entailment.
- X: not started, running, completed, resource limit, unsupported, error or
  blocked upstream, for this named run.
- K: certificate references with exact subjects and verification status.
- W: source/application warrant with actual authority, competence and scope.

Produced/Undefined require Completed. Completed requires Produced/Undefined/
Partial. Partial cannot be NotStarted; NoCompleteValue cannot be Completed.
Execution failure is not mathematical undefinedness or historical negation.
A schema/checker success creates no source acceptance, theorem, rights, consent,
publication or canon. Competent authorized human/agent judgments follow
[KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md); actual human consent remains human.

The standalone [operation-binding-v1 schema](../contracts/operation-binding-v1.schema.json)
is a structural sidecar, not a source metadata kind or grant. The foundation
checker validates the complete native JsonValue envelope, closed shapes,
unique keys/slots, exact reference boundaries, V/X compatibility and certificate
subjects. Certificate input order matters. Certificate descriptors remain
UnverifiedReference; theorem bodies are not read. Record versions use native
positive u64 integer lexemes; identifiers/text consumed by this native profile
must be UTF-8 and IDs follow StableId's non-control rule. Original input is
borrowed unchanged, including input rejected by this execution profile.

The caller supplies finite OperationBindingLimits. Every public check re-admits
its arguments; composition/comparison share one cap across both envelopes and
follow-up scans. No source bodies, key sets or output bytes are cloned. The byte
charge conservatively bounds compact JSON encoding at six bytes per UTF-16 unit
plus syntax and number lexemes; it is not original file size or a digest.
`max_nodes` bounds visits and comparison/string work, `max_refs` reference
visits including comparison, and depth is capped by the codec's safe ceiling
128. Checked arithmetic, zero-budget rejection and precharged quadratic scans
make duplicate/slot/map comparisons finite. Host resource admission remains
outside this pure checker; no hidden default budget is supplied.

Structural composition accepts only Produced/Completed with exact left output
and profile matching one right input, and equal context/selection. A named
semantic bridge is UnverifiedBridge. Partial consumption, changed perspectives,
H2 validity, role/criterion compatibility and domain inference require separate
adapters. Structural success does not start or complete the right operation.

Dependency comparison distinguishes Changed from Unresolved. ExactPinsEqual
requires independently issued ManifestCoverageEvidence for both actual
bindings; K0 deliberately provides no constructor, deserializer or issuer for
that private-field evidence. Thus incoming self-declarations cannot issue
reuse authority. Actual owner adapters and their capture proof are future
separately reviewed seams, not synthetic production attestations.

## Shared obligations for domain consumers

A consumer declares carrier, question, partial domain, exact profile/map,
quantifier, unknown/stop outcome and dependency owner before interpreting data.
Missing binding preserves the candidate and blocks only dependent operations.

For PWE, source Claim to QualifiedContent uses one exact outer-predicate/kind/
full-account-predicate dispatch. The first bounded profile requires whole
TypedRef scope aliases and world/context aliases to agree. Source polarity,
account outcome, current admission and historical interpretation are separate.
Negative/rejected Same does not create Distinct. Historical identity paths need
one compatible world, criterion and scope, with distinct constraints retained.

Attribution questions distinguish existential, universal and joint witnesses.
Source group arrays are positive conjunctions, not complete assigned sets.
Universal/excluded answers require nonempty joint Omega and appropriate full
or sound-covering evidence. Identity closure is per world: possible identity is
`exists world exists path`; necessary identity is `forall world exists path`.
Neither unioning edges before closure nor requiring one fixed path in all worlds
implements both queries.

An output profile binds actual record `r`, projection `pi_p` and codomain:
`resolve(output_ref)=r`, `pi_p(r)` defined and `pi_p(r) in Codomain(p)`.
Wrapper, projected container and items need their respective schemas. The PWE
PreservedCandidate producer selects `/body` with its exact output profile;
its completion does not execute semantic resolution. The generic K0 checker
neither resolves this record nor runs projection/schema predicates. Producer
adapters own that postcondition and its exact dependency closure.

The first consumers keep exact scope/criterion/context equality, independent
new Claim plus exact provenance/selection lineage for a new proposition,
independent owner completeness evidence, and the existing metadata facade plus
a narrow registry-version descriptor. No new universal type system or native
profile IDs are invented. Time/space profiles additionally own TRS/CRS/rules,
partial transformations, frame/boundary validity, joint model coverage and their
finite solver budgets. Those consumers are not implemented by K0.
