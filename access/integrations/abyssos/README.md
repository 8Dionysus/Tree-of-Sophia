# Optional AbyssOS integrations

This directory is the adapter boundary for ecosystem deployment, KAG, shared
stats, and artifact admission. It intentionally contains no required runtime
code in the standalone profile.

Tree of Sophia owns the standalone product and its projection readers.
`abyss-stack` may deploy the product; `aoa-kag` and `aoa-stats` may consume
bounded ports; `abyss-machine` retains artifact policy, registry, and consumer
admission. None of those owners is imported into the query core.
