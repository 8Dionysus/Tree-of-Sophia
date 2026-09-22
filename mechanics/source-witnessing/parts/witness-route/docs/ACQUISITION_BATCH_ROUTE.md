# Versioned acquisition batch route

`scripts/acquisition_batch.py` owns the repeatable preparation and local
transfer boundary for a new provider batch. The route is deliberately
selection-driven: the caller supplies one immutable manifest matching
`ToS/contracts/acquisition-batch.schema.json`, an explicit metadata root, and
an empty output root. The route never discovers records by walking the
checkout and never mutates the shared source topology.

The manifest is the frozen input selection. Every selected Item carries its
exact metadata record references and SHA-256 values, a rights record and
posture, and one provider revision/source identity. Every payload carries its
stable ToS File ID, Item root, provider URL/revision/source ID, byte size,
SHA-256, and optional Git blob SHA-1. `provenance_delta` closes over the same
record and File sets and binds them to the exact accepted base revision. A
digest supplied to the command is the operator's immutable-selection check.

`prepare` copies only the enumerated records into `source/`, emits one
batch-level `provenance-delta.json`, and writes an immutable preparation
receipt. It does not acquire bytes. On every resume and immediately before a
handoff, the route rereads every selected metadata record, including the
rights record, and compares its bytes with the manifest and preparation
receipt. It also checks the exact deterministic provenance-delta path and
digest. A changed selected record fails closed before a provider fetch or
handoff receipt can be produced.

The producer preflight requires each selection to name its exact Item record,
Item manifest, rights record, and Item provenance record, and every metadata
reference must pass the existing corpus-source membership predicate. The
prepared Item record identity is checked against the selected Item ID before
payload transfer. If preparation is interrupted, recovery removes only a
directory whose manifest bytes match the requested selection, whose source
tree contains only selected record paths and the deterministic delta, and
whose payload and receipt trees are empty. Existing payload, receipt, or
foreign source evidence is retained and causes a fail-closed error.

`acquire` uses the provider fields to fetch each payload independently, publishes it through
`source_payload_custody.publish_bytes_no_clobber`, verifies destination
readback, and appends a durable per-file journal row. A failed provider is
recorded and does not prevent other files from completing. A later invocation
rechecks successful destinations and retries the failed or missing files.
Preparation is rebuilt when an interrupted run left only the narrow
route-owned `source/`, `payload/`, and `receipts/` shape without
`receipts/preparation.json`; an output with unrelated files or an existing
receipt is never removed. A sibling lock serializes concurrent `acquire`
calls for the same output root, including first preparation and handoff run
identity.

After each run, a separate fixity pass rereads every destination and writes
`receipts/fixity-*.jsonl` plus its summary. The immutable
`receipts/handoff-*.json` carries selected record refs/digests, provider and
payload custody rows, the fixity refs and JSONL/summary SHA-256 values, the
provenance-delta ref and SHA-256, and these
boundaries:

```text
acquisition_status: acquired-not-admitted | partially-acquired-not-admitted | prepared-not-acquired
admission_status: not-admitted
publication_status: not-published
```

The handoff is an input to the corpus-intake owner. It is not a corpus batch,
accepted revision, R2 transfer, publication decision, semantic review, canon
change, or deployment receipt. Intake should select one or more explicit
handoff paths and bind each selected handoff's manifest and fixity digests;
the acquisition route intentionally provides no hardcoded historical batch
list. The route-owned `scripts/acquisition_handoff_adapter.py` is the small
consumer fixture for one selected handoff: it verifies those bindings and
emits a private `tos_corpus_batch_v1` input root with explicit `source/` and
`payload/` roots. It checks the selected revision against an explicit accepted
store pointer and loads that pointer's cryptographically bound immutable
`revisions/<base_revision>/snapshot.json`. For every selected source path, the
accepted-source view must contain the exact snapshot member and bytes; a path
absent from the snapshot must also be absent from the view. An arbitrary empty
or mismatched view therefore fails closed before candidate output. The adapter
then calls the existing `corpus_admit.read_batch` contract and does not call
admission. The caller must provide the exact validation context that produced
the selected `validator_sha256`: grammar root plus every paired historical
capture/restored root. The adapter verifies retained capture and restore
receipts, writes that context and its admission flags into the candidate
receipt, and reports `explicit-grammar-update-required` when the selected
identity differs from the accepted snapshot. It never silently rewrites the
batch identity or uses the accepted validator as a substitute. A downstream
admission run must pass the receipt's exact context to `corpus_admit`; the
adapter's read-batch check is transport evidence, not SourceValidator
acceptance. The receipt marks this as
`validation_context_posture: transport-bound; downstream-source-validator-required`;
the adapter does not recompute validator identity or claim the grammar/history
preflight has run. It also binds every custody row to all selected provider
fields and the handoff run, checks the selected Item rights scope and
acquisition event, and compares both the payload Git-blob digest and accepted
source mode before emitting a candidate. It also copies the exact selection manifest, handoff, provenance
delta, and independent fixity files under
`receipts/acquisition-evidence/`. The delta remains operational evidence and
is not added to `tos_corpus_batch_v1.updates`; its source ref and SHA remain
bound in the adapter receipt. Top-level receipt refs point to these candidate
copies, while explicit `*_source_ref` fields retain the original acquisition
paths.
The queued-corpus-intake owner can use this adapter's selector and receipt
shape while its seven-batch converter remains a separate owner surface.
