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

File-backed representations keep their exact payload metadata separate from
composite identity. A public branch may carry a representation without its
bytes: declare `materialization_status=not_materialized`,
`storage_posture=unmaterialized_payload`, and `git_tracked=false`. If bytes are
materialized, declare `materialization_status=materialized`,
`storage_posture=tracked_repository_payload`, and `git_tracked=true`; composite
payloads must be tracked rather than hidden by Git-ignore. Rights, local-only
visibility, and later text or semantic layers remain separate decisions.
