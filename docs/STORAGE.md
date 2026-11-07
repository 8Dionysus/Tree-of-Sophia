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

Both are in the Cloudflare account hosting `tree-of-sophia-access`, with the
default jurisdiction. The managed `r2.dev` endpoint is disabled for both.
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
the site may read them. The access Worker configuration declares the
`PUBLIC_LAYERS` binding to `tos-public-layers`, but the binding is not deployed
or used by application code as part of this setup. The binding is technically
capable of writes; application and credential policy must enforce the intended
read-only use. Do not bind `tos-source-bytes` to the public Worker.

When CI needs S3-compatible access, supply GitHub Actions secrets:
`R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, and `R2_ENDPOINT`.
Use an R2 Object Read & Write token scoped only to the buckets its workflows
use; this grant permits writes and deletes there, subject to the bucket lock.
Restrict which workflows receive these secrets and prefer a separate read-only
token when a consumer only needs reads. The Publish Bonsai workflow consumes
them on main: it reads the stands' generated layers named by each
`bonsai/<stand>/layers.json`, writes the files R2 lacks, reads each back, and
copies the layers into the published site at `/__research/layers/sha256/<hex>`
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

## Pricing

As checked 2026-11-02, [Cloudflare R2 pricing](https://developers.cloudflare.com/r2/pricing/)
for Standard is $0.015 per GB-month, $4.50 per million Class A operations,
and $0.36 per million Class B operations. The monthly Standard free tier is
10 GB-month, 1 million Class A and 10 million Class B operations; Internet
egress is free. Billing rounds up to the next billing unit. Cloudflare prices,
other product charges and account terms can change; consult the linked page.
