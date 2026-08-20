# Server Import Protocol

## Stop line

This protocol prepares a future boundary. It does not authorize deployment,
network transfer, publication, or widening access to any current payload.

## Import order

```text
ToS item manifest
  -> exact manifest and payload SHA-256 verification
    -> human-reviewed rights policy for the named jurisdictions
      -> explicit access class and derivative matrix
        -> operator-approved transfer
          -> isolated server import and verification receipt
            -> publication decision
              -> continuing takedown and expiry checks
```

## Required input

Every item plan names:

- stable item and file IDs;
- tracked item-manifest ref and SHA-256;
- each payload filename, byte size, and SHA-256 without an absolute path;
- rights-record ref and digest;
- jurisdiction and human/legal review posture;
- access class;
- allowed and prohibited derivatives separately;
- server import and publication status;
- takedown/contact route;
- provenance and version.

## Access classes

- `deny`: no payload transfer or publication;
- `metadata-only`: public-safe catalog metadata only;
- `controlled-research`: authenticated processing under explicit conditions;
- `public-payload`: source payload publication explicitly authorized.

No class is inferred from a file's age, local availability, or repository
metadata. `public-payload` requires affirmative, scope-specific rights evidence.
A metadata-only site record may state that research used a named local ToS item
and preserve its provenance without transferring or serving that item's bytes.
Verified public-domain, open-license, permission-granted, or conditional
noncommercial evidence may open a matching public route for the exact material
it covers. The importer must enforce every recorded condition; it must not
generalize permission from a work to an edition, from an edition to a scan, or
from a catalog record to source bytes.

## Derivative matrix

OCR, transcription, page images, snippets, lexical indexes, embeddings,
alignments, translations, annotations, and graph/search projections each have
their own `allowed`, `conditional`, `prohibited`, or `unknown` state. Permission
for one does not imply permission for another.

## Server behavior

The future importer must:

1. receive a frozen plan and operator approval rather than scan the checkout;
2. verify manifest and payload bytes before accepting them;
3. preserve stable ToS IDs and write an immutable import receipt;
4. enforce access and derivative policy before generating or serving content;
5. keep server storage and projections subordinate to ToS authority;
6. recheck expiry, changed rights evidence, and takedown requests;
7. support disabling publication and deleting server copies without deleting
   the ToS record of what happened.

Repository checkout, generated catalog, graph export, or future site build may
never silently discover and upload gitignored payload bytes.

For the current operator-supplied local corpus, the future public-site route is
metadata/provenance only: the source files remain local even if another
material derived through the research lineage later receives separate
publication permission.
