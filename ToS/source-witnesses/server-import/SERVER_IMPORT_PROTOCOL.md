# Server Import Protocol

## Stop line

This protocol describes the future source-owned server payload boundary. The
native `tos-native-owner-command source-payload` route exposes local
verification, explicit import, receipt, read, and publication-registry
mechanics, but does not authorize use of any current payload. Deployment,
network transfer, publication, or widened access still require a specific
current plan and the approvals it names.

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

A server-import plan is not a required companion of every source Item. Local-only
corpus admission may proceed without creating a speculative import plan. A plan
is required before an Item is proposed for server transfer or import; without
an exact plan, the Item remains outside that transfer route and receives no
transfer authority. Plan absence does not determine the Item's rights or
publication status.

Every present item plan names:

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

## Native command

The native owner accepts one bounded JSON request on stdin through
`tos-native-owner-command source-payload`; its request schema identifier is
`tos_source_payload_import_request_v1`. The acquisition command also routes
requests carrying `family: "payload-import"` to this owner. Available
operations are `verify-local`, `import`, `read-local`, `read-remote`,
`registry-enable`, `registry-disable`, and `batch`. See the adjacent README for
the operation boundaries. `verify-local` is read-only; `import` additionally
requires `confirm_transfer: true` and a frozen plan whose explicit operator
approval and rights evidence pass current checks. Uploads and readbacks use
the shared native R2 transport. A transfer receipt remains separate from
publication registry state, and registry-protected reads revalidate the
current plan before returning bytes. Every remote read requires both the
current plan and an enabled registry entry. The current command implements
local registry disable/revoke only. The plan's delete_supported flag declares
provider capability; it does not indicate that this command performs physical
deletion. Step 7 remains a future owner capability.
Read operations retain a shared registry lock through local output publication;
disable/revoke takes the exclusive lock and therefore waits for an admitted
read before returning. A remote read only orders the local registry decision
and verified output publication: it cannot cancel a fetch already in progress,
delete provider bytes, or recall an already published local copy.
The registry lock does not serialize concurrent edits to authored plan or
rights files.

For the current operator-supplied local corpus, the future public-site route is
metadata/provenance only: the source files remain local even if another
material derived through the research lineage later receives separate
publication permission.
