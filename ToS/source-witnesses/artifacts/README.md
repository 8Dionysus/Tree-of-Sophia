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
            └── rights.json
```

An artifact record keeps the physical object, mutable catalog record,
inscription or transliteration, scholarly composite, photograph, and line art
as distinct layers. A catalog label is evidence, not a fixed sign meaning.
Artifact presence does not by itself make the object philosophy, source text,
semantic truth, graph truth, or canon.

Only public-safe metadata is tracked in the first planting. Images, line art,
transliterations, translations, and other content-bearing payloads remain
absent until their exact source, rights, acquisition, fixity, and review route
has been handled separately.
