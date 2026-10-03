# Tree-of-Sophia KAG integration

ToS publishes KAG independently from its software and corpus admission. This directory owns the bounded provider template and operator route.
Published provider data lives in the selected immutable integration release.

The maintained export and publication Python entrypoints delegate to
`tos-kag-release` (`TOS_KAG_RELEASE_BIN` selects an explicitly prepared image).
Provider controls use the same native module in publication; direct helper calls
select `tos-kag-provider-controls` with `TOS_KAG_PROVIDER_CONTROLS_BIN`.
The former Python implementations remain explicit compatibility oracles and
are never fallback defaults. The selected aoa-kag producer and probe remain
external owner adapters; moving ToS mechanics does not replace their authority.

The native local contour refuses exports over 8 MiB, V1 revision manifests over
64 MiB, generated releases over 1 GiB or 100,000 members, and manifests over
16 MiB. Publication shares a 600-second deadline; each selected owner child is
limited to 300 seconds and 16 MiB combined output. Status lock waits are bounded
to five seconds. Filesystem operations poll cancellation between bounded reads;
kernel calls retain the host execution envelope. Pre-cleanup failures retain
their staging bytes; a failure during polled cleanup retains only remaining
bytes, and a status failure after full cleanup can leave no staging directory.
Cleanup enumerates at most 100,000 members, unlinks regular files individually,
then removes directories deepest first, checking the whole clock throughout.

`provider-template.json` describes the portable node, edge, index, projection
and receipt routes. The publisher combines these declarations with one exact
corpus export in a new external directory. The selected aoa-kag producer builds
its complete family there, and its family and provider-home readers validate
the result before local publication. Template receipts declare the route. Completed validation receipts and the
selected release identify the observed integration state.

Use [VALIDATION.md](VALIDATION.md) to build an export, publish an integration and
inspect its status. Every result lives in `releases/<integration_revision>` and
binds its corpus/export revision, consumer program hashes and complete bytes.
A consumer update may create a new integration for the same corpus. Failures
retain the last successful release and remain visible in downstream status.

Existing aoa-kag consumers select the published `provider/Tree-of-Sophia` root,
including through their explicit `TREE_OF_SOPHIA_ROOT` configuration. Its cold
shards are retained inside that immutable local provider, so the provider-home
reader needs no ambient corpus checkout or hidden artifact cache. Exact queries
can also select the release's separate `artifacts` directory explicitly.

The selected provider exports a bounded source-return route to ToS
philosophical graphs and authored sources. Federation, MCP services and
AbyssOS installation require separate activation by their consuming owners. A pinned older integration stays an older
integration; software CI cannot make it current.
