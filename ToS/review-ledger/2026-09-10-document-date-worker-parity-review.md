# Document catalogue date: Worker parity review

## Scope and boundary

Reviewed the bounded successor to
`aba18049e4e84150edfe0dd27c5dcec882242834`: the already-declared
`document-catalogue-temporal-v1` reader now has the same guarded comparison
route in Python and the Cloudflare Worker. No source Claim, Document, Place,
calendar, grant, historical event, assessment, translation or deployment was
created or modified. This is access/read-model work, not catalogue acceptance.

The source review checklist confirms that the catalogue-assigned Document date
does not become a composition, dispatch, receipt or historical-event date.
The exact source profile, predicate/schema/layer, Document ancestry, whole
Claim/value/literal hashes and source file/line must agree. Different otherwise
comparable roles are unsupported; absent calendar/year numbering remains
undetermined. Existing historical carriers retain their earlier requirements.

## Exact bytes and limits

The new reader alone receives `semantics.claim.source_canonical_json`, using
the existing source-owner canonical JSON framing and a 262144-byte UTF-8 cap.
The producer emits null when over budget; missing, null, inconsistent or
over-budget material refuses documentary comparison. This is a bounded derived
companion, not a new identity, registry, authority or readable-context fallback.

The Worker retains the original D1 row JSON privately for the duration of the
comparison. It verifies the entire Claim, literal value and normalized raw value
with number-token-preserving canonicalization before hashing the supplied source
bytes. It never recreates source hashes through JavaScript numeric values.
Float spelling, large integers, negative zero, escaped strings, nested fields
and Unicode key ordering are retained. Ordinary HTTP JSON numbers still have
the existing IEEE-754 transport framing (including negative zero becoming zero);
the exact canonical string remains unchanged in the complete returned context.

The companion participates in Claim finalization inputs and content revision.
The existing edge build fingerprints the changed normalizer/schema, and a new
row revision must be published through the ordinary read-model route. Old
documentary rows without this companion fail closed; no live store was changed.
Only exact indexed lookups are used: at most six nodes for a documentary pair,
four for historical pairs, plus the existing three snapshot metadata reads.

## Verification and manual review

- Worker TypeScript typecheck passed.
- Existing documentary owner create/revise/replay and HumanForm end-to-end
  test passed in 26.590 seconds, 75.4 MiB service peak, zero swap.
- Python temporal API module: 17 tests passed in 35.575 seconds, 134.7 MiB
  service peak, zero swap. This includes the UTF-8 byte-cap boundary.
- Worker comparison, actual isolated D1, and Worker HTTP: 154 tests/subtests
  passed in 57.920 seconds, 624.6 MiB service peak, zero swap. All 76 cases
  compare full expected packets; 19 are native documentary positive/negative
  cases, built through the actual source builder and normalizer rather than
  fabricated semantic fields. The existing Basel no-calendar case remains.
- Native source inputs are synthetic date attributions over copied Letter
  metadata, not evidence about the archive card. Numeric and Boolean mutation,
  missing/invalid canonical material, wrong profile/Document/source/value/
  literal/field/wording, unknown calendar and cross-role cases fail closed.
- Source-home and whitespace checks passed. No full corpus import, generated
  corpus/KAG regeneration, full release suite, CI, merge or production check.

An initial fixture incorrectly used the historical file basename for the new
profile; it now uses the real `source-claims.jsonl` route. The first Worker
fixture transport exceeded its buffer because it repeated unrelated graph
nodes; fixtures now transmit only the exact required nodes with complete
records. Subsequent parity failures exposed the numeric reason-order and
negative-zero wire-framing boundaries described above. Neither source meaning
nor a validation condition was weakened to make those runs pass.

Cloudflare and Workers best-practices guidance informed use of native Web
Crypto, request-local exact-row metadata, indexed D1 reads, and isolated tests.
No Workers binding, secret, deployment setting or remote resource changed.

## Next owner

Foundation integration owns the combined read-model rebuild and current API
consumer check. Source owners retain every real identity, Claim and grant.
Local parity and this review do not grant publication, source truth, canon,
whole-Foundation acceptance or a deployment.
