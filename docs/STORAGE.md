# Source-byte and public-layer storage

Tree of Sophia designates Cloudflare R2 as the immutable store for source
bytes and for public layers that cannot be rebuilt from Git. Existing data
remains at its current source until an authorized migration is verified. This
infrastructure does not itself admit rights or authorize a migration. The
operator's machine keeps a separate recovery copy.

| Bucket | Purpose | Location and class |
| --- | --- | --- |
| `tos-source-bytes` | Source bytes and witness inputs, including restricted material | WNAM, Standard |
| `tos-public-layers` | Redistributable public-text and generated public layers | WNAM, Standard |

Both are in the Cloudflare account, with the default jurisdiction. The managed `r2.dev` endpoint is disabled for both.
Keep source material private; publication depends on its separate rights and
review decision. A public-layer designation does not grant permission to
publish source material.

## Identity, retention and access

Use `sha256/<lowercase-hex-digest>` where the 64 hexadecimal digits are the
SHA-256 of the exact object bytes. Verify the bytes against the key on upload
and retrieval. Both buckets have the enabled rule
`retain-sha256-indefinitely`: prefix `sha256/`, indefinite retention. It blocks
overwrite and deletion while the rule applies. Cloudflare allows authorized
bucket administrators to change or remove lock rules; protect those privileges
and do not treat this as administrator-proof WORM. `scratch/` is outside the
rule and is reserved for disposable tests; delete test objects after use.

Authorized corpus operators write source bytes after the relevant rights and
admission decisions. The future migration package owns that transfer and its
manifest. Authorized CI and operators may write admitted public layers; CI and
the site may read them. The public research Worker has no bucket bindings.
Do not bind `tos-source-bytes` to the public Worker.

When CI needs S3-compatible access, supply GitHub Actions secrets:
`R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, and `R2_ENDPOINT`.
Use an R2 Object Read & Write token scoped only to the buckets its workflows
use; this grant permits writes and deletes there, subject to the bucket lock.
Restrict which workflows receive these secrets and prefer a separate read-only
token when a consumer only needs reads. The Publish Bonsai workflow consumes
them on main: it reads the stands' generated layers named by each
`bonsai/<stand>/layers.json` and the public corpus snapshot named by
`access/public-corpus/layers.json`, writes the files R2 lacks, reads each back,
and copies them into the published site at `/__research/layers/sha256/<hex>`
for checkouts without R2 access. A producer without R2 access leaves new files
on the `layers/transfer` branch; the publication keeps there only what R2 still
lacks ([bonsai layers](../bonsai/README.md#layers-kept-in-r2-tosbonsailayersv1)).
The endpoint for the default jurisdiction is
`https://<R2_ACCOUNT_ID>.r2.cloudflarestorage.com`.

## Recovery and verification

Maintain the local recovery copy and a separately preserved digest manifest
listing each bucket, key, byte size and SHA-256, along with its source/revision
and rights provenance. After an authorized transfer, enumerate the expected
objects, retrieve and hash their bytes, compare them with the manifest and
verify exact membership; report missing, unexpected and mismatched objects.
Periodically repeat this verification and test restoration to a separate path.
A key name or an ETag alone is not proof of the bytes. Keep the recovery copy
until the owner has independently verified both the cloud copy and restore.

## Private witness custody preparation

The selected artifact photographs are inventoried in
`ToS/source-witnesses/artifacts/private-custody.v1.json`. The five Zarathustra
research families retain their own `r2-manifest.v1.json` files. Their entries
identify the exact source revision, logical path, size, SHA-256, private access,
`tos-source-bytes` bucket and content-addressed key. Earlier source-rights
records and provenance remain separate from this custody designation.

The paragraph alignment and the parallel lexical candidates keep three small
files in git: `manifest.v1.json`, `coverage-receipt.v1.json` and
`summary.v1.json`. Edition curations pin the manifest and the coverage receipt
as frozen evidence, and the curation tests read all three. The bucket also
holds these bytes, so each present file is checked in full against its custody
entry.

`node bonsai/lib/private-custody.mjs check` validates those manifests and hashes
any locally present files. Missing bytes are reported as not hydrated; that
result does not establish that the objects exist in R2. An authorized operator
can run `node bonsai/lib/private-custody.mjs hydrate` with `R2_ENDPOINT`,
`R2_ACCESS_KEY_ID` and `R2_SECRET_ACCESS_KEY`, scoped for private bucket reads.
For offline verification, `--objects ABSOLUTE_DIRECTORY` reads keys from a local
object store, `--recovery ABSOLUTE_DIRECTORY` reads the logical paths from a
recovery copy, and `--target ABSOLUTE_DIRECTORY` selects an existing restore root.
Choose only one offline source. The selected roots and their path components
inside each root must not be symlinks.
The command verifies size and digest before writing, refuses differing existing
files, and verifies the installed bytes. It does not upload, publish, or use the
public mirror or transfer branch. Neither the inventory nor a successful local
test establishes R2 read-back or a recovery copy.

The native witness and research readers validate the relevant private inventory
when bytes are absent and report that content verification is incomplete.
Present bytes are checked for fixity; full cross-file content checks run when
the required bytes are present. The inventory does not establish that an absent
object is recoverable.

The photograph representation records retain their original acquisition-time
`tracked_public_payload` and `git_tracked` declarations as provenance. For the
14 paths in `artifacts/private-custody.v1.json`, the custody inventory describes
the later storage selection; the older declarations do not establish current
Git membership or grant publication rights.

## Pricing

As checked 2026-11-02, [Cloudflare R2 pricing](https://developers.cloudflare.com/r2/pricing/)
for Standard is $0.015 per GB-month, $4.50 per million Class A operations,
and $0.36 per million Class B operations. The monthly Standard free tier is
10 GB-month, 1 million Class A and 10 million Class B operations; Internet
egress is free. Billing rounds up to the next billing unit. Cloudflare prices,
other product charges and account terms can change; consult the linked page.
