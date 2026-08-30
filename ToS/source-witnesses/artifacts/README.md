# Physical Artifact Witnesses

`artifacts/` holds tracked, source-returnable metadata for physical witnesses
that do not naturally enter the bibliographic
`work -> expression -> edition -> item -> file` ladder.

The speaking route is based on physical or excavation identity, not on the
current digital provider:

```text
artifacts/
└── <tradition-or-script>/
    └── <site-or-custody-context>/
        └── <physical-or-excavation-identity>/
            ├── README.md
            ├── artifact-witness.json
            ├── rights.json
            └── representations/
                └── <view-or-object>/
                    ├── representation.json
                    ├── rights.json
                    └── payload/           # only when exact rights and acquisition permit
```

An artifact record keeps the physical object, mutable catalog record,
inscription or transliteration, scholarly composite, photograph, and line art
as distinct layers. A catalog label is evidence, not a fixed sign meaning.
Artifact presence does not by itself make the object philosophy, source text,
semantic truth, graph truth, or canon.

`artifact-source-witness.schema.json` retains the original planted-metadata
contract. `artifact-source-witness-v2.schema.json` also permits an exact
artifact witness that has no honest philosophy-backlog planting yet; discovery,
candidate, File, and provenance relations remain available without inventing a
philosophy anchor.

Images and line art may be retained only as separate File-backed visual
representations under `artifact-visual-representation.schema.json`. Their exact
provider record, byte fixity, rights scope, acquisition event, and payload
posture must close together. A public visual payload still does not admit its
embedded text, identify the physical object by itself, or create translation,
semantic, graph, canon, publication, or human-review authority.
