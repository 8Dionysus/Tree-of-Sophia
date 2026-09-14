# Knowledge contract fixtures

These bounded JSON files are test inputs for the knowledge contract tests and
Cloudflare Worker knowledge tests. They preserve the source-shaped records
needed by the contract assertions while keeping ordinary software tests
independent of a live corpus.

## claim-navigation.json

- Source commit: 12ba439444a8f4cbd936111f00fe00acea20eceb
- Source path: ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json
- Custody view: /srv/abyss-machine/artifacts/tos-corpus-custody-r2-20260913/main-integration-view/Tree-of-Sophia
- Source root SHA-256: 788435774ef01ebb932cef90c6530280a09c06032a148318b533e51340827308
- Selection: stream ProjectionReader, take the first claim_traces row whose
  predicate is translated_by, retain edges whose claim_ref matches that
  trace, and retain nodes whose node_id is the trace's claim node or an
  endpoint of a retained edge.
- Counts: 1 claim_traces, 8 edges, 9 nodes
- Fixture SHA-256: c9d52245beb5cebeef90981b5fb34f343088d385d1d2c9c733884c786eb95b77

## semantic-annotation-synthetic.json

- Source commit: 12ba439444a8f4cbd936111f00fe00acea20eceb
- Source path: ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json
- Source SHA-256: 00a22067da28cd17ab02f2fde2e227b70f5b32ac11c9b68514fcee4133f39003
- The fixture preserves the exact bytes from git show at that commit.

These files are test fixtures only. They do not admit source, add semantic
decisions, or rewrite synthetic source material. They are excluded from the
software package and remain separate from data-release acceptance.

## Cloudflare Worker knowledge fixtures

The following files are bounded test inputs for
`access/deploy/cloudflare-worker/test/knowledge.test.ts`. They were extracted
read-only from the custody view below and are not source admission:

- Custody view: `/srv/abyss-machine/artifacts/tos-corpus-custody-r2-20260913/main-integration-view/Tree-of-Sophia`
- Source commit: `12ba439444a8f4cbd936111f00fe00acea20eceb`
- Source path: `ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
- Source root SHA-256: `788435774ef01ebb932cef90c6530280a09c06032a148318b533e51340827308`

### worker-form-node.json

- Selection: the raw node with `identity_ref` `tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese`
- Counts: 1 node
- Fixture SHA-256: `a45fbb0da3a7477bcfe3587924b877a8a61ae163e5119cadecdadb856f3b7581`

### worker-claim-form-node.json

- Selection: the raw node with `claim_ref` `tos.claim.nietzsche-letter-705.sender`
- Forms: `human_forms` was materialized once from the existing
  `prepare_claim_change` and `materialize_claim_forms` test-only call
  (`software:test-only`, `tos.form.test.claim`, `claim.statement`,
  `access_allowed=True`) before storing this raw node
- Counts: 1 node, 1 human-form packet
- Fixture SHA-256: `e108091549e306114ed41262564c0206d9794ebf673c00cc8c9d3e0a41d7f5a4`

### worker-claim-navigation.json

- Selection: exactly the three literal claim IDs retained by the embedded
  test script; retain each matching `claim_traces` row, every edge named by
  those traces' `edge_ids`, and the closure of node endpoints from the
  retained edges plus each trace's claim, subject, and object nodes
- Ordering: traces by `claim_ref`, edges by `edge_id`, and nodes by `node_id`,
  matching the current `ProjectionReader` branch
- Claim IDs:
  `tos.claim.expression.also-sprach-zarathustra.ru-nani-1899-nine-fragments.translated-by-s-p-nani`,
  `tos.claim.topology.expression-edition.friedrich-nietzsche.also-sprach-zarathustra.ru-nani-1899-nine-fragments.embodied-by.saint-petersburg-stasyulevich-1899-nine-fragments`,
  `tos.claim.topology.work-expression.friedrich-nietzsche.also-sprach-zarathustra.has-expression.ru-nani-1899-nine-fragments`
- Counts: 3 claim_traces, 20 edges, 17 nodes (16 exact extracted node
  carriers plus 1 bounded synthetic provenance-event carrier)
- Fixture SHA-256: `c761a5ccf5dd5587ebff46da0a19f091c1dbb6bd2216fd462890b787c88f7cdb`
- Synthetic carrier: the extracted event
  `tos.event.annotation.source-witness-bibliographic-topology.2026-07-31`
  had source SHA-256
  `57f0866b4d4a89239f5540eed7b0d8076276f34ed9ac7d1888dfb3c2c76937b4` at
  source commit `12ba439444a8f4cbd936111f00fe00acea20eceb`, path
  `ToS/source-witnesses/relations/provenance.jsonl`. Its 32,824 input records
  are reduced to the first two, and the bounded event carries the warning
  `Synthetic transport fixture: historical batch input list reduced to two representative inputs; not source admission.`
  The synthetic event SHA-256 is
  `97e1203821f9995664c37b8ee0fc0d0d7f8afc8fac5a0454027e3c07910d0c40`.
  Exact old node IDs are replaced only in the retained edge and trace carrier
  references; claim bodies, assertions, identity/form bytes, and the other 16
  node carriers are unchanged.

Worker fixtures remain test-only synthetic transport inputs. They introduce no
semantic decisions, overwrite no source material, and are excluded from the
software package and data-release acceptance.
