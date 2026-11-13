# Public research Worker

The public site uses `wrangler.public-research.jsonc` and serves the home,
Zarathustra and Antiquity stands and the `/__research/` assets. Unmatched
paths, including the retired atlas API, receive the site's 404 page. The
Worker has no D1, R2 or secret bindings.

Build the site with `npm run build:public-research` from this directory. Run
`npm run typecheck` and `npm test` for the Worker and shared contracts. The
shared tests use the matching generated `tos-web-rules` WASM; prepare it as
specified in [RELEASING](../../../docs/RELEASING.md). Inspect a local upload
with `npm run deploy:public-research:plan`. Deployment follows that release
route and is a separate operator action.

The old access Worker and its D1 database are retired from this codebase. The
operator owns D1 export, custom-domain transfer, and cloud removal after code
merge. The current route configuration is unchanged by that retirement.

## Native offline prepared-pair capture

The installed native access product provides
`PREFIX/bin/tos edge-offline-capture --request ABS.json`. It creates an
unapplied, bounded SQL capture from explicitly selected inputs. It does not
mutate D1, switch consumers, deploy, admit source meaning or rights, or claim
semantic acceptance. Request limits have no defaults. Inputs and outputs are
absolute paths; SQLite inputs must be regular, non-symlink files held with
identity checks through capture. This is offline custody, not a live selection
lease.

The request is limited to 10 MiB. Prepared bindings are limited to 1 MiB;
catch-up source-input JSON is limited to 1 MiB before escaping; each retained
projection root is limited to 256 KiB. Strict preflight rejects duplicate JSON
members and bounds parser depth, visits, integer digits and logical state.
These are admitted input/work bounds, not process-memory guarantees.

`operation` selects one of five native routes:

- `prepared-delta`: exact predecessor/successor prepared snapshots and D1 revision.
- `prepared-catchup`: exact predecessor D1/source-input state plus a successor snapshot.
- `source-navigation-delta`: an admitted prepared transition for selected navigation rows.
- `source-navigation-bootstrap`: a complete product only when native navigation tables are absent; requires exact node/edge roots and a separately selected rights root.
- `source-navigation-integrity`: a held D1 product compared with selected roots; full mode audits rows and payloads, while `header_only` checks header policy and inventory counts without claiming a row audit.

Transition captures reuse the maintained D1 row, knowledge, lens and metadata
producers with auxiliary-store guards, predecessor checks and reversible closure;
the [native capture owner](../../../rust/crates/tos-access/src/edge_offline_capture.rs)
defines those checks.

Every request names the expected predecessor `expected_d1_revision`, distinct fresh
paths for `forward_sql`, optional `rollback_sql`, and `manifest_json`, and the
operation's positive finite resource limits. Delta and catch-up may omit
rollback; bootstrap and integrity require it. The result uses
`tos_edge_offline_capture_result_v1`; receipts use
`tos_edge_native_prepared_delta_receipt_v1`,
`tos_edge_native_prepared_catchup_receipt_v1`,
`tos_edge_native_source_navigation_delta_receipt_v1`,
`tos_edge_native_source_navigation_bootstrap_receipt_v1`, or
`tos_edge_native_source_navigation_integrity_receipt_v1`. Outputs include
actual hashes, bytes, accounting and lineage. Admission, currentness, rights, semantic
acceptance, applied D1 state and consumer switch remain false.

Example command:

```sh
/absolute/prefix/bin/tos edge-offline-capture --request /absolute/scratch/request.json
```

An addressed prepared delta request has this shape. Binding objects must be the
complete exact values from the selected prepared snapshots; placeholders below
are explanatory and must be replaced:

```json
{
  "schema": "tos_edge_offline_capture_request_v1",
  "operation": "prepared-delta",
  "d1_database": "/absolute/scratch/selected-d1.sqlite",
  "before_prepared_database": "/absolute/scratch/before.sqlite",
  "after_prepared_database": "/absolute/scratch/after.sqlite",
  "expected_d1_revision": "<64 lowercase hex characters>",
  "before_binding": { "replace_with": "exact predecessor binding" },
  "after_binding": { "replace_with": "exact successor binding" },
  "before_source_inputs_json": null,
  "rights_root": null,
  "forward_sql": "/absolute/scratch/capture/forward.sql",
  "rollback_sql": "/absolute/scratch/capture/rollback.sql",
  "manifest_json": "/absolute/scratch/capture/manifest.json",
  "limits": {
    "prepared": {
      "max_changes": 10000,
      "max_row_bytes": 2097152,
      "max_metadata_bytes": 2097152,
      "max_read_bytes": 67108864,
      "max_rows": 10000,
      "max_retained_bytes": 33554432,
      "max_sql_bytes": 67108864,
      "max_postings": 100000,
      "max_manifest_rows": 10000
    }
  }
}
```

Output parents must remain exclusively owned through capture. A failed or
incomplete capture is not publication evidence; the caller separately admits
runtime memory and storage. This capture supports historical D1 tooling, which is pending separate retirement.
