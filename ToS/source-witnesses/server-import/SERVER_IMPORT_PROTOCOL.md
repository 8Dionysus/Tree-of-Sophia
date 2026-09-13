# Server Import Protocol

## Stop line

This protocol defines the boundary; it is not itself transfer or publication
authority. The current bounded helper may execute an explicitly approved,
private controlled-research v3 import through the operator's transport
command. It does not authorize a site deployment, public publication, or
widening access to any payload.

The bounded command helper at `scripts/source_payload_import.py` implements the
mechanical part of this boundary. It accepts only an explicit plan and an
operator-supplied `--payload-source-root` (whose default layout is the
permanent `ToS/source-witnesses` root); it never scans a checkout for payloads.
`verify-local` is read-only. `import` requires `--confirm-transfer` and an
explicit managed `--scratch-root`; it checks the complete plan/manifest File
inventory before applying an optional `--file-ref`, snapshots each verified
payload into that scratch root, verifies the snapshot again, then passes only
the snapshot to the transport. It verifies a remote readback and writes an
immutable receipt with no absolute local path. The current Wrangler adapter
uses R2 Standard, `private,no-store` object metadata, and the single-object
CLI envelope of 315 MiB; a larger file is rejected until a multipart adapter is
independently implemented.

Imports sharing one receipt directory take an operator-local lock across
remote existence check, upload, readback, and receipt publication. This is a
single-writer coordination rule for the configured operator route; Wrangler
does not provide a conditional-create/CAS primitive, so no cross-host or
foreign-writer atomicity is claimed. A competing writer that is outside this
lock is accepted only if the later readback matches the frozen digest,
otherwise the import fails closed.

The receipt stores a file-SHA content-addressed object key and binds the exact
Item, File, item-manifest revision, rights-record revision, and (when
applicable) rights-review revision. Its filename and receipt ID also include
the frozen plan revision, so a changed plan cannot collide with an earlier
import. Equal bytes in two editions therefore do not share a publication
decision. The local publication registry is a separate mutable
control surface with a file lock; enabling, disabling, or revoking one exact
publication never edits the source plan or receipt. Registry-protected local
and remote reads recheck the current plan digest, Item/File identity, rights
revision, expiry, and review scope. A remote read without a registry is an
operator-only verification command, not a site or public endpoint.

## Import order

```text
ToS item manifest
  -> exact manifest and payload SHA-256 verification
  -> exact human/legal v1 or controlled agent-reviewed v3 rights policy
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
- jurisdiction and exact rights-review posture;
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

The original v1 plan semantics remain unchanged: payload transfer requires
`human-reviewed` or `legal-reviewed` rights plus separate real-human operator
approval. A v3 plan may use `rights_policy.review_status: agent-reviewed` only
with `access_class: controlled-research`, `publication_status: not-published`,
and a digest-bound `rights_review` ref. The additive
`source-payload-rights-review.schema.json` requires a model actor, exact Item,
all planned File IDs, manifest and rights-record digests, tracked local license
evidence bytes, explicit raw-cloud/server-processing limits, expiry and a
revocation check. The importer compares every scope value to the frozen plan;
an agent review cannot authorize `public-payload` and cannot replace operator
approval. Historical v1 plans must not be restamped as agent-reviewed.

## Derivative matrix

OCR, transcription, page images, snippets, lexical indexes, embeddings,
alignments, translations, annotations, and graph/search projections each have
their own `allowed`, `conditional`, `prohibited`, or `unknown` state. Permission
for one does not imply permission for another.

## Server behavior

The importer or future site boundary must:

1. receive a frozen plan and operator approval rather than scan the checkout;
2. verify manifest and payload bytes before accepting them;
3. preserve stable ToS IDs and write an immutable import receipt;
4. enforce access and derivative policy before generating or serving content;
5. keep server storage and projections subordinate to ToS authority;
6. recheck expiry, changed rights evidence, and takedown requests;
7. support disabling publication and deleting server copies without deleting
   the ToS record of what happened. The current helper implements local
   registry disable/revoke and verified CLI readback; site deletion and
   deployment remain a separate owner route.

Repository checkout, generated catalog, graph export, or future site build may
never silently discover and upload gitignored payload bytes.

For the current operator-supplied local corpus, the future public-site route is
metadata/provenance only unless a separate public-payload plan and publication
decision exists. A private controlled-research R2 receipt does not create a
public site route, even if another material derived through the research
lineage later receives separate publication permission.
