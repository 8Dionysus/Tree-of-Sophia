# Thought structure profiles, 2026-09-07

## Scope and source review

Bounded Foundation T02/T04/C01 implementation based on
`4b8f362de037efb32bf3b66fdb326e84b46466df`. Current authority remains with
the semantic-interchange registries, README and source schemas. This applies
the existing TOS-D-0053 semantic-reader boundary; it does not introduce a new
canon, ontology root, writer or assessment policy.

- Yes: Thesis, Argument, InferenceStep and Objection are described semantic
  referents, separate from their source record/version and research Claims.
  Their substantive content, scope and continuity criterion are mandatory.
  A nonblank check is not a judgment of substantive adequacy or truth.
- Yes: premise and conclusion are contextual roles of a thesis in a specified
  inference step, not subclasses. An objection can target a premise as a
  thesis, the transition as a step, the whole argument, or a conception.
  These targets and the argument developing an objection remain distinct.
- Yes: partial reconstruction and competing positions remain expressible.
  `step_position` is a nonnegative integer in one reconstruction context,
  not historical time, a corpus-wide unique key or a completeness check.
  The same thesis can be premise and conclusion without the serializer
  certifying or prohibiting the reasoning. No graph-wide acyclicity is added.
- Yes: all twelve new predicates use specific endpoints and reified Claims
  with evidence, statement, basis, maker, provenance, qualifiers and separate
  assessment. They are nontransitive and have both English/Russian reading
  directions. Attribution, expression, support and successful proof are not
  identified. A statement about a thought object is not the object itself.
- Yes: common source creation, exact-schema correction and atomic Claim batch
  creation are exercised without per-kind writer branches. The independently
  selected correction grant must allow `semantic_content`; schemas without
  that field continue to reject it. Scope, ID and admission cannot be changed
  through a descriptive correction. Previous package bytes are retained.
- Yes: names/notes carry complete content and scope as required reading
  context. Seven declared semantic content properties execute by property ID
  through the ordinary query constructor. A short label does not discard
  the hypothetical force or the partial-reconstruction qualification.
- Yes: existing source/canon mappings and identities remain unchanged. The
  shared semantic description constraints were extracted without changing
  the accepted shape of existing concept/conception records.
- Not applicable: historical truth, logical validity, model competence,
  actual assessment/admission, rights, canon, UI source or deployment. The
  tests use clearly marked synthetic accounts and assertions.

## Verification

The initial thought-chain contract test failed because the Thesis profile did
not exist. After the source profiles and relations were added, it passed
through catalog, both graph views, mandatory human-form context and focus.
Additional tests exercise all new endpoint families, inverse wording, missing
basis and invalid target refusals, creation/correction with retained bytes,
and all-or-nothing reasoning Claim publication and replay.

```bash
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest discover -s mechanics/growth-cycle/tests
python -m unittest discover -s access/tests -p test_knowledge_contract.py
python scripts/build_source_witness_catalog.py --check
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
```

- 63 graph tests passed in 65.338 seconds.
- 144 growth tests passed in 93.981 seconds.
- 58 knowledge-contract tests passed in 21.350 seconds.
- After adding the seven property-ID execution assertions to the existing
  thought-chain test, that affected test passed again in 0.644 seconds.
- Source catalog and graph were regenerated through their builders; both
  parity checks and the graph validator passed.

Times are concurrent local samples, not p95 or hosted budgets. Neither CI,
Cloudflare deployment nor UI interaction was performed for this data-profile
increment. The existing cross-reader engine is unchanged.

## Remaining work and rollback

Real source-grounded concept/conception/thesis/argument/objection chains, exact
occurrence links and substantive assessment remain required. Other thought
profiles, complete multilingual forms, wider growth commands, scaling,
migration and actual UI consumption are not closed by this increment; the
Foundation map keeps their states explicit.

Reader rollback must reject unsupported profiles without deleting source
records or revision histories. No existing source objects need retyping for
this additive profile change. Correcting an account is allowed only for the
same research referent; assessing that judgment remains with the source-visible
assessment owner rather than this structural test suite.
