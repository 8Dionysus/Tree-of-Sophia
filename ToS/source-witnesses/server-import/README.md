# Source Payload Import Boundary

This route imports explicitly authorized files from permanent local custody
and verifies their remote bytes. A site may receive an item only after its
manifest, fixity, rights, derivative, access, publication, and takedown gates
agree. Importing an object does not deploy the site or enable public access.

```text
server-import/
├── README.md
├── SERVER_IMPORT_PROTOCOL.md
└── plans/                    # tracked, digest-bound import plans and status records
```

The contract is `ToS/contracts/server-import-contract.schema.json`. A server
receipt is downstream evidence. It never replaces ToS item/file IDs,
manifests, rights records, provenance, reviewed texts, or claims.

## Local custody and controlled import

`scripts/source_payload_custody.py` preserves and verifies the exact files of
explicit Item manifests in a separately selected payload root. Metadata and
source bytes have distinct roots: a working branch can read or acquire files
in the permanent local corpus without retaining another payload copy inside
that branch. Follow `LOCAL_STORAGE_BOUNDARY.md` before any material write.

`scripts/source_payload_import.py` consumes the existing server-import
contract. Its `verify-local` and `import` commands bind exact Item/File IDs,
manifest, rights and review revisions, and SHA-256. Transfer requires an
explicit plan and invocation plus a managed scratch root for a second verified
payload snapshot; it does not discover or upload ignored files from a
checkout. The plan and manifest inventories must agree exactly before a
selected File is transferred. The R2 adapter uses Standard storage and
independently reads back every admitted object. Local files remain preserved
after upload.

Use `read-remote` for authenticated operator readback without the website.
The local registry commands exercise controlled enable, disable and revocation;
they do not create a public HTTP route or replace the site's future access
checks. Source manifests and backend credentials are separate inputs. Keep
credentials, absolute host paths and private operational receipts outside Git.

The current Wrangler adapter bounds individual uploads to 315 MiB. Imports
sharing one receipt directory use the operator-local single-writer lock;
Wrangler has no cross-host conditional-create guarantee. Larger
objects require a separately configured multipart transport; they are not
silently truncated or routed through D1. The tools' `--help` output names the
explicit roots, plan, transport, receipt and output arguments.

The website is undergoing an independent refactor. Local custody, R2
readback, repeat-import behavior and restricted reads can be verified without
deploying it. A successful command is not evidence of site integration.

## Preserved restricted example

The *Ecce Homo* 1908 Commons/Getty plan exercises an active-term failure mode:
positive public-domain results for Nietzsche and Richter and scoped Commons
metadata licenses do not clear the mixed PDF while van de Velde's attributed
design remains in copyright in Germany through 2027-12-31. Its server policy
is `restricted`, `metadata-only`, `blocked-rights`, and operator-unapproved.
Term expiry is only a recheck trigger: any future candidate must be newly
acquired from a current upstream route, receive new fixity and provenance, and
pass layer, quality, human-review, and operator gates. The local payload is
never the upload source.
