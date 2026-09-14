# Native Artifact metadata growth

`tos_local_artifact_create_owner_v1` delegates one new physical Artifact v2
metadata record under `artifacts/<tradition>/<site>/<physical-identity>/`.
The target directory must be absent; existing independent input records live
outside it. The actual `artifact_id` is retained. A provider label or a Corpus
`record_id` shadow does not become physical identity.

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
remains distinct from native metadata serialization. This metadata check does
not inspect private snapshots, authenticate capture or grant public access;
it refuses a retained claim without the exact source account. A private HTML
snapshot may contain embedded text without becoming an admitted transcription.
Historical v1 fingerprints and all false authority fields stay unchanged.

Only a version-1, unreviewed v2 Artifact with no performed human review and an
empty `philosophy_planting_refs` is writable. The source schema retains false
text/semantic/graph/canon/publication authority. Text, transliteration,
translation, images, payloads, private inputs and owner-local paths are not
accepted into this metadata route. Earlier generic grants remain unchanged.

The shared source writer atomically publishes the Artifact, its source-copy
form set and the four conventional `source-create-*` capture companions.
The event records `native-artifact-metadata-serialization`, not discovery,
acquisition, rights assessment or planting. Its outputs are only metadata and
forms; rights, discovery and research are exact inputs. It is unsigned
mechanical evidence, not authenticated execution truth or present admission.

Creation replay and the read-only origin verifier preserve the initial bytes
through separately retained native descriptive corrections and committed
selected-file history. They do not enumerate or read Artifact descendants.
The origin verifier never consults mutable grants or runs the foundation
validator recursively. Missing/corrupt capture fails closed. If exact earlier
input bytes are no longer available, the original origin cannot be rebound to
today's rights or research; availability must be restored by their owner.
Legacy discovery provenance keeps its separate unchanged validation branch.

Run `mechanics/growth-cycle/tests/test_source_artifact_commands.py`, native
descriptive revision and exact metadata reader tests, then relevant source
foundation, discovery and topology checks. These synthetic serializer tests
do not assess any real Artifact or decide rights, publication or canon.
