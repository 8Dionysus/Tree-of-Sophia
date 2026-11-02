# TOS-D-0075 JCS Canonical Bytes

## Index Metadata

- Decision ID: TOS-D-0075
- Original date: 2026-11-02
- Surface classes: docs/route-law, ToS/contracts, rust/foundation
- ToS layers: contracts, docs
- Tree classes: none
- Guard families: artifact integrity, compatibility, source-first authority
- Posture: accepted; migration remains future work

## Context

Canonical JSON participates in record references, request identities, snapshot
revisions and derived artifacts. Existing profiles use Python-compatible
ordering and numeric spellings, with different carrier line endings. Bare
hashes do not identify their serialization contract. Exact byte hashes also
appear alongside canonical JSON hashes and retain independent meaning.

## Decision

The new canonical profile is `jcs-rfc8785-v1`: RFC 8785 UTF-8 bytes with UTF-16
code-unit member ordering, ECMAScript number serialization, no whitespace and
no trailing LF. Strings retain their exact Unicode scalar sequences without
normalization. Duplicate names, lone surrogates and non-finite values fail.
Each carrier contract states explicitly whether storage appends one LF and
whether its digest covers JCS bytes alone or JCS plus that LF. The encoder
itself never appends LF.

| Carrier | Current byte and digest rule | Future JCS rule |
| --- | --- | --- |
| Corpus snapshot revision | Legacy compact JSON plus one LF; the digest covers that LF. | Its owner must choose and tag JCS alone or JCS plus LF before cutover. |
| Source record or form reference | Legacy compact JSON without LF; digest covers those bytes. | The owner defines whether its carrier adds LF and exactly what the digest covers. |
| Source command request or configuration | Legacy compact JSON without LF; digest covers those bytes. | The owner defines whether its carrier adds LF and exactly what the digest covers. |
| Historical receipt or chain | Original stored bytes and original digest rule remain authoritative. | No rewrite; the archived verifier uses the original rule. |
| Other new carriers | Their existing owner rules remain in force. | The carrier contract declares storage LF and digest coverage explicitly before use. |

The ToS admission policy converts the exact decimal value to the nearest
binary64 using round half to even, requires a finite result, and admits it
only when the exact decimal value of its ECMAScript number serialization is
equal to the input value; negative zero is refused. JCS has one number type,
so `1` and `1.0` share the canonical form `1`. The later migration inventory
reports lexemes whose spelling changes in hashed carriers. A later reviewed
schema migration represents precision-sensitive values as defined strings.
New digest references and revisions carry an explicit profile tag beside the
hash, for example `jcs1` mapped unambiguously to `jcs-rfc8785-v1`. Stable
opaque IDs remain unchanged. Existing profiles and stored values remain in
force until their respective reviewed cutovers.

The migration order is inventory, codec, verified old-to-new reference map,
source owners, immutable stores, PostgreSQL, exports, and retirement of legacy
writers. `tos-digest-inventory` is a transitional migration tool, removed when
the legacy canonical profiles are retired. Each phase resolves dependencies and preserves exact source evidence.
Historical receipts and chains retain their original bytes. A protected Git
tag at the pre-migration commit, retained original inputs and runtime
artifacts, and an archived reproducible verifier preserve verification; the
checked map connects current data to new profile-tagged references. A tag or
map alone cannot reconstruct missing bytes.

## Options Considered

- Continue legacy profiles without a versioned convergence path.
- Convert all numeric values through binary64 and rewrite historical chains.
- Admit only lossless values, tag profiles, map current references, and retain
  original historical evidence and verifier.

The third option preserves both reproducibility and the ability to review
unsupported values before migration.

## Rationale

Byte identity requires a named algorithm, a carrier boundary and preserved
inputs. The separate inventory distinguishes canonical JSON from raw bytes
and unresolved producers. Source and semantic review remain owner decisions.

## Consequences

The new codec is independently usable without changing any existing digest.
Migration remains blocked on complete producer and storage inventory, mapping,
unsupported-value review and coordinated consumer readiness. Production
verification of historical receipts requires access to the archived verifier
and exact original inputs; whether it is invoked directly by production remains
an implementation decision.

## Source Surfaces

- `ToS/contracts/digest-field-classes.v1.json`
- `rust/crates/tos-foundation/src/json.rs`
- `rust/crates/tos-foundation/src/jcs.rs`
- `rust/crates/tos-command/src/source_command.rs`
- `rust/crates/tos-source-store/src/manifest.rs`
- RFC 8785, <https://www.rfc-editor.org/rfc/rfc8785>

## Validation

Run foundation codec vectors and refusal tests, inventory command tests and
workspace checks. Regenerate and validate decision indexes with the owner
route in `docs/VALIDATION.md`. Migration, historical verification and deployed
consumer acceptance require their own future evidence.
