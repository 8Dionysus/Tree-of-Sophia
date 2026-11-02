# Native Artifact metadata growth

`tos_local_artifact_create_owner_v1` delegates one new physical Artifact v2
metadata record under `artifacts/<tradition>/<site>/<physical-identity>/`. The
target directory must be absent; existing independent input records live
outside it. The actual `artifact_id` owns the physical identity; provider
labels remain attributed descriptive metadata.

The grant fixes the record ID/path, principal and maker kind, creation event,
source-copy form IDs, allowed operation and expiry. `source_bindings` contains
exactly `rights_ref`, `discovery_ref` and `research_ref`, each as
`{"ref": "ToS/...", "sha256": "<64 lowercase hex>"}`. The same whole binding
object is required in `prepare-create` and `source.create`; it survives in the
retained request and cannot be replaced by caller input or a later live grant.
All three inputs must already exist as public source metadata. Rights must
cover the Artifact with metadata-only visibility/redistribution. Discovery
must address that exact Artifact identity. Research bytes are bound, not
assessed, and no remote source or provider is fetched.

V2 catalog fingerprints distinguish retained (`captured: true`) and
unretained response bytes. A retained fingerprint must match a captured
snapshot in the independently bound discovery record: exact response URL,
SHA-256, acquisition SHA-256 and byte size. The snapshot's discovery event
remains distinct from native metadata serialization. The metadata check requires
the exact source account for a retained claim; private snapshots and capture
authentication remain outside its inspection. Public access and embedded HTML
transcription require their own admission.
Historical v1 fingerprints and all false authority fields stay unchanged.

Only a version-1, unreviewed v2 Artifact with no performed human review and an
empty `philosophy_planting_refs` is writable. The source schema retains false
text/semantic/graph/canon/publication authority. Text, transliteration,
translation, images, payloads, private inputs and owner-local paths are not
accepted into this metadata route. Earlier generic grants remain unchanged.

The shared source writer atomically publishes the Artifact, its source-copy
form set and the four conventional `source-create-*` capture companions.
The event records `native-artifact-metadata-serialization`. Its outputs are
metadata and forms; rights, discovery and research are exact inputs. The event
is unsigned mechanical evidence, distinct from discovery, acquisition, rights
assessment, planting and admission.

Creation replay and the read-only origin verifier preserve the initial bytes
through separately retained native descriptive corrections and committed
selected-file history. They do not enumerate or read Artifact descendants.
The origin verifier never consults mutable grants or runs the foundation
validator recursively. Missing/corrupt capture fails closed. If exact earlier
input bytes are no longer available, the original origin cannot be rebound to
today's rights or research; availability must be restored by their owner.
Legacy discovery provenance keeps its separate unchanged validation branch.

Run `rust/crates/tos-command/src/source_artifact_native.rs`, native
descriptive revision and exact metadata reader tests, then relevant source
foundation, discovery and topology checks. These synthetic checks cover
serialization and retained history.
