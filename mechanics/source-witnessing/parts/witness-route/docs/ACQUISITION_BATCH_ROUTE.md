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
receipt. It does not acquire bytes. `acquire` uses the provider fields to
fetch each payload independently, publishes it through
`source_payload_custody.publish_bytes_no_clobber`, verifies destination
readback, and appends a durable per-file journal row. A failed provider is
recorded and does not prevent other files from completing. A later invocation
rechecks successful destinations and retries the failed or missing files.

After each run, a separate fixity pass rereads every destination and writes
`receipts/fixity-*.jsonl` plus its summary. The immutable
`receipts/handoff-*.json` carries selected record refs/digests, provider and
payload custody rows, the fixity refs, the provenance delta, and these
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
list.
