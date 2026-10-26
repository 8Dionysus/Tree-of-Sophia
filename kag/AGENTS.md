# AGENTS.md

## Applies to

This card applies to `kag/` and every nested path.

## Role

`kag/` owns the standalone source-return export for downstream KAG consumers.
Consumer provider homes and indexing belong outside this repository. Authored ToS meaning remains in `ToS/`; bounded portable records give
consumers exact handles back to that source.

## Operating Card

| Field | Route |
| --- | --- |
| input | exact accepted corpus revision and versioned ToS export producer |
| output | bounded exact source export and source-return manifest |
| owner | `kag/AGENTS.md`, `kag/README.md`, native source-export contract |
| next route | native ToS export -> consumer-owned import and indexing outside ToS |
| validation | native export membership, fixity and source-return checks; independent of software merge |

Build exports from an explicitly selected immutable ToS corpus revision. ToS
never invokes a consumer's checkout, interpreter, builder or validator. The
consumer owns provider materialization, indexing and admission from its side.
Unrelated software changes do not require a production export rebuild.

## Source Routes

- The explicitly selected export artifact and its source-return manifest
- `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- `ToS/derived-exports/README.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`

## Validation

Use [`kag/VALIDATION.md`](VALIDATION.md) for native export verification and the
real standalone CLI fixture. Authored meaning, source review, rights and canon
retain their source-owned routes. A valid export does not establish consumer
index currentness or runtime admission.
