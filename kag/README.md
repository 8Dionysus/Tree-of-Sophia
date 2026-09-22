# Tree-of-Sophia KAG integration

ToS publishes KAG independently from its software and corpus admission. This directory owns the bounded provider template and operator route.
Published provider data lives in the selected immutable integration release.

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
