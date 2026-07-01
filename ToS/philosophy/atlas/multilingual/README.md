# Multilingual Atlas

`multilingual/` owns the language route for philosophy atlas planting and
projection labels.

It does not make translations stronger than sources. It gives every
text-bearing philosophy node a stable place for original-language form,
transliteration or scholarly normalization, Russian review text, and English
review/runtime text.

## Shape

```text
multilingual/
  content-labels.json
  language-registry.json
  text-bearing-nodes.contract.json
```

## Route

Use `content-labels.json` for display labels that already appear in atlas and
graph projections.

Use `language-registry.json` for language slots, script codes, title roles, and
status vocabulary.

Use `text-bearing-nodes.contract.json` when planting works, corpora,
inscriptions, source witnesses, versions, translations, and commentarial
materials. This contract prepares the future path from source material into
nodes, relations, graph views, and downstream bilingual UI.
