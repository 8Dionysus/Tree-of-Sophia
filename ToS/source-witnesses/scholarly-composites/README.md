# Scholarly Composite Witnesses

`scholarly-composites/` holds tracked identities for modern scholarly objects
that reconstruct, collate, or arrange material from more than one physical or
textual witness.

```text
scholarly-composites/
└── <genre-or-method>/
    └── <tradition-or-script>/
        └── <composition-identity>/
            ├── README.md
            ├── composite-witness.json
            └── rights.json
```

The route is based on the scholarly composition, not on a current provider.
Provider pages, API records, physical members, their inscriptions, editorial
lines, translations, images, and interpretations remain separate layers.
A composite can stabilize comparison coordinates while remaining a modern,
mutable, reviewable reconstruction. It is not a physical artifact, an ancient
original, accepted source text, a fixed sign meaning, philosophy, graph truth,
or canon merely because it has a stable Q-number or a complete-looking text.

The native `composite-witness.json` format remains valid and unchanged. A
source-described textual reconstruction or arrangement can instead use
`composite.json` under the same method/tradition/identity route and stable
`tos.composite.*` family. See the
[descriptive growth contract](../../doctrine/semantic-interchange/README.md#descriptive-composite-growth-alongside-native-witnesses).
The two formats cannot carry duplicate current records for one ID. The new
format does not require fictional physical members, replace retained witness
observations or grant rights, text admission or canon authority.

File-backed representations keep their exact payload metadata separate from
composite identity. A public branch may carry a representation without its
bytes: declare `materialization_status=not_materialized`,
`storage_posture=unmaterialized_payload`, and `git_tracked=false`. If bytes are
materialized, declare `materialization_status=materialized`,
`storage_posture=tracked_repository_payload`, and `git_tracked=true`; composite
payloads must be tracked rather than hidden by Git-ignore. Rights, local-only
visibility, and later text or semantic layers remain separate decisions.
