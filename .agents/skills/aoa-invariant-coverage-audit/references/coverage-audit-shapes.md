# Coverage Audit Shapes

Use this reference when the validation surface is wider than an ordinary unit
test suite. It is not a checklist. Pick the smallest shape that names one
stable invariant, one existing evidence surface, one claim limit, and one
bounded next check.

## How To Use

1. Name the stable truth being claimed.
2. Name the validation or proof surface that appears to support it.
3. Choose the narrowest audit shape below.
4. Map evidence to the invariant, then state what the evidence does not prove.
5. Add only the smallest follow-up check that closes the most important gap.

## Shape Set

| Shape | Existing Surface | Invariant Question | Gap To Look For | Useful Follow-Up |
|---|---|---|---|---|
| Code test suite | Unit, integration, smoke, or regression tests. | Does behavior hold across the important input or state space? | Examples cover happy paths but not the stable rule. | Boundary, negative, property, or regression case. |
| Schema or manifest | JSON schema, YAML manifest, registry, config, or typed model. | Does structural validity protect the rule consumers rely on? | Required fields pass while semantic relationships are unchecked. | Invalid fixture, enum drift check, or cross-field validation. |
| Fixture family | Golden files, snapshots, canned inputs, or sample repos. | Do examples represent the meaningful variation? | Many fixtures repeat the same shape. | One fixture per missing state, role, boundary, or failure mode. |
| Generated/export parity | Builder output, compact index, installed copy, adapter export, or capsule. | Does generated material stay derived from source truth? | Freshness passes while source-to-output meaning is not checked. | Rebuild check plus source-ref, field-map, or stale-output failure. |
| Report or receipt | Markdown report, JSON receipt, run summary, status surface, or dashboard feed. | Does the report constrain the claim readers infer from it? | Complete-looking reports overstate verdicts or hide exclusions. | Claim-limit assertion, required exclusion field, or malformed receipt case. |
| Eval or proof result | Eval bundle, scorer output, verdict, benchmark result, or review gate. | Does the proof surface justify exactly the claim being made? | One score is treated as total quality or broad intelligence. | Claim-scope fixture, scorer edge case, or verdict-schema failure. |
| Router, SDK, or adapter compatibility | Dispatch hints, typed loader, SDK facade, CLI compatibility report, adapter bridge. | Does compatibility preserve stable consumer assumptions? | Consumer paths work only for the common route. | Unsupported-shape fixture, error contract, or version compatibility case. |
| Workflow or role scenario | Playbook phase, handoff, role contract, session route, approval gate, or operator step. | Does the workflow guard the invariant under realistic route changes? | Scenario prose states a rule but no check catches skipped gates. | Minimal route trial, handoff receipt, stop-condition, or negative path check. |
| Memory, recall, or provenance surface | Recall object, writeback envelope, source reference, retention rule, retrieval capsule. | Does retrieved or stored evidence keep provenance and freshness limits clear? | Recall output looks authoritative without source or freshness bounds. | Source-ref check, stale-recall case, retention invariant, or writeback envelope check. |
| Metrics or source coverage | Coverage report, metric summary, source-count report, adoption audit, or quality dashboard. | Does the metric actually constrain the stable claim it is used to support? | Counts are treated as proof of quality, adoption, or completeness. | Denominator check, source coverage gap, active-view rule, or metric exclusion case. |

## Compact Audit Pass

| Field | Answer |
|---|---|
| Stable invariant |  |
| Existing surface |  |
| Shape selected |  |
| Evidence that constrains it |  |
| Evidence that only repeats examples |  |
| Claim limit |  |
| Smallest useful follow-up |  |
| Downstream reader risk |  |

## Verification

- one invariant is named in plain language
- the existing validation surface is mapped to that invariant, not praised in
  general
- the claim limit is explicit enough to stop over-trust
- the follow-up check is smaller than a full test strategy
- generated, reported, scored, or retrieved evidence remains weaker than the
  source truth it describes
