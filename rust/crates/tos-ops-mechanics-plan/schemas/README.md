# Portable stats export format

Native ToS embeds this bounded v1 format snapshot. It requires no source checkout,
interpreter, schema service or network retrieval. The existing JSON Schema library
checks complete port/measurement/packet structure; ToS owns reference-only census
ratio bindings and publication. Shared format ownership remains with its author.

The three Apache-2.0 schema inputs are from `8Dionysus/aoa-stats` at
`eadef646bc096285169124d995214ad13268fba3`. The license is retained in the repository root `LICENSE`.
Original schema IDs were removed and the sole cross-file reference was made local.
All validation references in the embedded bundle are internal. Updating the format
requires a reviewed source change and new portable fixtures; it never follows an
ambient consumer checkout.

| Original path | SHA-256 |
| --- | --- |
| `stats/federation/local-port.schema.json` | `78a46202a17452ca42c4dd9349d3a3131bcbecc07ab000ba08ab18b44c70f280` |
| `stats/measurement-contract/measurement-contract.schema.json` | `553abdf23826442407461261abcc5ed800c3a9bab1be03f79e5f08cf531c0e42` |
| `stats/measurement-contract/measurement-packet.schema.json` | `7fd29eccaba716858a6346c7e7011a475b46dae493e1378be60c50b04215d1bc` |
