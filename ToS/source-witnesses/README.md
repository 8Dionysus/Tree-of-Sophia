# Source Witness Corpus

`ToS/source-witnesses/` is the physical and authored evidence root for works,
expressions, editions, collections, acquired items, source-near text, and
aligned witness packets.

The exact division between project evidence and host-managed caches/runtimes is
recorded in [`LOCAL_STORAGE_BOUNDARY.md`](LOCAL_STORAGE_BOUNDARY.md).

It does not own semantic canon, laboratory runtimes, or generated graph/index
stores.

## Speaking topology

```text
source-witnesses/
├── agents/
│   └── <person-or-organization>/agent.json
├── discovery/                              # ordered query/result evidence
├── access-requests/                        # public status + ignored private correspondence
├── server-import/                          # future no-upload/import boundary
├── catalog/
│   ├── agents.jsonl
│   ├── works.jsonl
│   ├── expressions.jsonl
│   ├── editions.jsonl
│   ├── collections.jsonl
│   └── items.jsonl
├── works/
│   └── <responsibility-or-tradition>/
│       └── <work>/
│           ├── work.json
│           ├── expressions/
│           │   └── <language-and-responsibility>/
│           │       ├── expression.json
│           │       └── editions/
│           │           └── <edition>/
│           │               ├── edition.json
│           │               └── items/
│           │                   └── <item>/
│           │                       ├── item.manifest.json
│           │                       ├── fixity.sha256
│           │                       ├── provenance.jsonl
│           │                       ├── rights.json
│           │                       ├── forensic-report.md
│           │                       └── payload/        # gitignored bytes
│           ├── texts/                              # reviewed source-near text
│           └── alignments/                         # multi-expression packets
└── collections/
    └── <responsibility-or-tradition>/
        └── <collection>/
            ├── collection.json
            ├── editions/
            └── membership-claims.jsonl
```

`<responsibility-or-tradition>` is a navigational route, not an authorship
claim. Anonymous, disputed, collective, and tradition-owned works receive
speaking routes and explicit responsibility claims in the catalog.

## Identity and path boundary

The catalog and object records own stable ToS IDs. Filesystem paths are human
navigation and may improve through reviewed migrations. A path change never
silently changes object identity.

The corpus follows `work -> expression -> edition -> item -> file`. Aggregate
volumes route through `collections/` and point to contained works/expressions
with evidence-bearing membership claims. A collection is not duplicated under
every contained work.

## Payload boundary

Only an item's `payload/` content is ignored by Git. Everything required to
identify, verify, understand, authorize, and reacquire that payload remains
tracked:

- item and file identity;
- original basename and media type;
- size and cryptographic digest;
- acquisition/source record;
- forensic inspection;
- rights and visibility posture;
- preservation and transformation events;
- references from accepted source-near text and anchors.

Payload files are immutable after intake. Changed bytes create a new file
record and, when materially distinct, a new item or item version.

Large working derivatives, model caches, OCR scratch, page renders, and
benchmark outputs belong to the `abyss-stack` laboratory or host-managed cache,
not beside the source payload. Reviewed text or annotation small enough to be
authored may return through `texts/`, `alignments/`, candidate intake, or canon
according to its authority layer and rights posture.

## Current seed

The first physical seed is *Thus Spoke Zarathustra* and one Nietzsche
collection containing it. The seed deliberately includes:

- a vector-outline Russian PDF requiring raster/OCR recovery;
- an image-plus-ABBYY Russian collection PDF with an imperfect text layer;
- an automatically generated German OCR EPUB with weak metadata and no usable
  navigation;
- the exact 529-page image-container PDF exposed by that EPUB's
  checksum-verified Internet Archive parent item;
- a checksum-verified 402-page scan of Yu. M. Antonovsky's 1913 Russian
  translation from Wikimedia Commons, kept distinct from both the later
  Antonovsky expression and the separately licensed Wikisource transcription.

The EPUB and image-container PDF are distinct acquired items descended from
the same scan family. The PDF supplies source-visible pages for independent
OCR; the EPUB text remains a sealed reference witness until variant outputs
are frozen. Internet Archive's Public Domain Mark is retained as a source
statement, while ToS keeps the bytes `local_only` and
`copyright_undetermined` pending human jurisdiction and terms review.

The 1913 scan likewise preserves the source provider's public-domain
declaration as positive rights evidence rather than flattening it into either
“authorized” or “closed.” Its local copy remains off the future public site by
operator policy. The Wikisource transcription is an independent CC BY-SA
candidate layer and remains deferred: its index reports incomplete
proofreading, so open licensing does not make it accepted text or gold.

These items are laboratory witnesses, not assumed critical editions. Their
catalog, rights, and forensic records must remain honest about what is known,
claimed, inferred, and unresolved.

## Source-near movement

```text
payload + receipt
  -> forensic inspection
    -> raw extraction or OCR proposal
      -> anchored manual correction
        -> reviewed text / alignment
          -> candidate observation or claim
```

Do not skip from acquired file to semantic canon.
