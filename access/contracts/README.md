# Access contracts

These files define the portable consumer seam. They do not redefine the
meaning of the ToS projections they carry.

`runtime-manifest.v1.json` names component ownership and profiles;
`runtime-data.v1.json` is the exact standalone data allowlist;
`query-operations.v1.json` owns transport-neutral read operations; and
`page-commands.v1.json` owns revisioned browser context plus shared human and
WebMCP actuation. `web-actions.v1.json` remains only as a migration marker for
the former combined ABI.
