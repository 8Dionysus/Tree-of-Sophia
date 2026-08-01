# Source-witness chronology

`chronology/` owns evidence-bearing temporal claims over corpus identities. It
does not own one universal timeline.

The first bounded profile is
`friedrich-nietzsche/first-publication/work-chronology-claims.jsonl`. Each
current Nietzsche Work points to one `first_publication_chronology` claim. The
claim keeps an interval, its boundary meaning, the publication sequence, and
an ordering warning together. A downstream list may order by the beginning or
end of that interval only when it names that facet; it may not silently call
either value “the date of the work.”

Composition, manuscript inscription, printing completion, title-page year,
public sale, posthumous editing, reception, and digitization remain distinct
temporal predicates. The current profile materializes only first-publication
chronology. It does not infer composition chronology, author-final text,
textual equivalence, rights, human acceptance, semantic truth, or canon.

The authored JSONL and its digest-bound provenance event are source authority.
The catalog and graph are rebuildable navigation.
