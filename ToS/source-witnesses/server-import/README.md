# Server Payload Import Boundary

This route implements a native, explicit transfer boundary for tracked
server-import plans. It does not discover checkout payloads, decide rights, or
authorize transfer or publication. Every transfer still requires the exact
plan, current manifest and rights bytes, the plan's rights review, and its
real-human operator approval.
All currently tracked server-import plans remain blocked-rights; none
authorizes a payload transfer.

The source owner is the native `source-payload` command in
`tos-native-owner-command`; it accepts one bounded JSON request on stdin. The
same owner is available through the acquisition wire with
`family: "payload-import"`. Its operations are `verify-local`, `import`,
`read-local`, `read-remote`, `registry-enable`, `registry-disable`, and
`batch`. Run `tos-native-owner-command source-payload --help` for the route
summary. Request paths must be explicit absolute paths; R2 transport selection
is explicit and credentials remain with the selected native transport.

`verify-local` checks the frozen plan inventory and local fixity without
transfer. `import` requires `confirm_transfer: true`; it preflights source
rights and bytes, transfers the exact snapshot through the shared native R2
transport, verifies remote readback, then writes an immutable receipt.
`batch` verifies the plan-index digest and every plan and source file before it
creates a transport; transfer runs use one transport and preserve a journal
and summary on failure. Receipts do not enable publication. The separate
publication registry supports explicit enable, disable, and revoke operations;
local and remote reads require an enabled registry entry and verify the receipt
and current plan before publishing a no-clobber output file.
Reads hold a shared registry lock through local output publication; disable and
revoke take the exclusive lock, so they wait for admitted reads; reads begun
after disable/revoke returns see the updated registry state. For a remote read
this orders the local registry decision and output publication only: it does
not cancel an in-flight fetch, remove provider bytes, or recall a local copy
already published.
The registry lock does not serialize concurrent edits to the authored plan or
rights files.
Disable and revoke change this local read gate only; they do not remove remote
bytes. Plan delete_supported is a provider-capability declaration; the current
import owner has no remote-delete operation, and the shared R2 transport
exposes fetch and put only. Protocol step 7 remains a future owner capability.

```text
server-import/
├── README.md
├── SERVER_IMPORT_PROTOCOL.md
└── plans/                    # tracked plan metadata; current plans remain blocked
```

The contracts are `ToS/contracts/server-import-contract.schema.json`,
`ToS/contracts/server-import-receipt.schema.json`, and
`ToS/contracts/source-payload-rights-review.schema.json`. Receipts and
publication-registry entries are downstream evidence. They never replace ToS
Item/File IDs, manifests, rights records, provenance, reviewed texts, or
claims.

The *Ecce Homo* 1908 Commons/Getty plan exercises an active-term failure mode:
positive public-domain results for Nietzsche and Richter and scoped Commons
metadata licenses do not clear the mixed PDF while van de Velde's attributed
design remains in copyright in Germany through 2027-12-31. Its server policy
is `restricted`, `metadata-only`, `blocked-rights`, and operator-unapproved.
Term expiry is only a recheck trigger: any future candidate must be newly
acquired from a current upstream route, receive new fixity and provenance, and
pass layer, quality, human-review, and operator gates. The local payload is
never the upload source.
