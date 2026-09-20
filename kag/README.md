# Tree-of-Sophia KAG integration

ToS publishes KAG independently from its software and corpus admission. This
directory owns the bounded provider template and the operator route; it is not
the current provider database.

`provider-template.json` describes the portable node, edge, index, projection
and receipt routes. The publisher combines these declarations with one exact
corpus export in a new external directory. The selected aoa-kag producer builds
its complete family there, and its family and provider-home readers validate
the result before local publication. Template receipts declare a route; they
do not assert successful validation or current production state.

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

The selected provider is a bounded source-return export. It does not replace
ToS philosophical graphs or source authority, and publication here does not
activate federation, MCP services or an AbyssOS installation. Their consuming
owners retain runtime admission. A pinned older integration stays an older
integration; software CI cannot make it current.
