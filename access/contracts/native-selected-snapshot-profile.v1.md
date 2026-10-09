# Native selected snapshot profile v1

`tos-native-owner-command native-original-produce` accepts the strict request
schema `tos_native_managed_original_produce_request_v2`. Its required
`selected_snapshot` object has schema
`tos_native_selected_snapshot_profile_v1`. A request without this selection
refuses; there is no historical-root or current-directory fallback. The result
uses `tos_native_managed_original_produce_result_v2` and `selected_source` for
this explicit provenance; the existing controlled cold witness ABI is unchanged.

The profile contains explicit absolute `runtime_data_root` and `manifest_path`
locators, independently expected `manifest_sha256`, `corpus_revision` and
`data_revision`, and independently expected current software/source-definition
`runtime_data_declaration_sha256` and `evidence_scenes_sha256`.

`excluded_compiled_model` contains exactly `path`, `size_bytes` and `sha256`.
Its logical `data/` path must be an output declared by the current
[runtime-data owner](runtime-data.v1.json). The selected manifest must contain
that exact row and omit it from source input bindings. The old compiled SQL
payload is neither opened nor copied. A suffix or directory scan cannot choose
an exclusion.

The bounded held manifest is authenticated against the independently supplied
SHA before its rows select data. Its sorted member rows and input bindings own
the retained census, checked byte sum and digests. They supply the maintained
capture selector's compact source-root bindings. Required roots follow that
selector and the runtime-data declaration; optional absence remains explicit.
No caller-provided member list or remembered corpus count grants membership.

Capture and final manifest validation use the same authenticated census.
Current compiled companions come from the capture owner's embedded companion
iterator, with exact current byte/digest checks. Source-data origins stay
separate from current software contracts and the source-only EvidenceLens
scene definition. The running ELF plus the documented partial embedded source
fingerprint binds producer identity; this is not a complete reproducible-source
closure claim.

The three `admission`, `built` and `verified` evidence refs keep their existing
opaque, bounded held-file/hash checks. They are provenance references, not an
implicit source selector or source/rights/publication admission.

All existing manifest, path, member, source-byte, model, filesystem custody,
fs-verity, original state/VM/deadline, tmpfs and persistent-write ceilings
remain active. The optional request field `max_work_bytes` selects one finite
cumulative capture/build work allowance from 1 byte through 512 GiB; omission
retains the existing 16 GiB default. This is cumulative byte/visitor work,
separate from RAM and physical file limits. The same work counter spans
capture, normalization, packing and build reads; phases do not reset it.
Search uses that same selected work allowance and a finite posting-count
ceiling equal to the selected cold-file byte ceiling; the actual byte limits
still decide whether the complete model fits. Catalog reduction likewise separates
intermediate SQL entries (at most the 10-million-row profile) and aggregate
bytes (at most 256 MiB, also within the existing TEMP ceiling) from the final
catalog (100,000 output entries and 16 MiB). Intermediate routes do not count
as additional output catalog entries. MAIN, TEMP and final cold-file limits
remain enforced independently. `cold_open.max_work_bytes` may
select up to 32 GiB for complete verification of expanded packed rows. These
are work allowances, not larger files or memory reservations. The producer
still requires its explicit deadline of at most three hours and all declared
state, process, VM, tmpfs, MAIN and cold-file caps. The full selected projection
can consume more than 256 GiB of checked work before search finishes; the
larger selectable work envelope covers those repeated reads. The default work
allowance and the 512 MiB build / 256 MiB cold-file limits are unchanged.

The result reports `producer_max_work_bytes` and the counter observation
`capture_build_observed_work_bytes`. A historical fixture may carry its frozen manifest and excluded
SQL expectations in its explicit caller profile. Observed historical census
numbers are evidence, not a general producer equality requirement.

The relocation and refusal regression is
`native_snapshot_manifest::selected_snapshot_tests::selected_snapshot_relocation_uses_authenticated_census_and_exact_exclusion`.
Compilation, execution and independent product receiving belong to their
bounded actual owner routes; source preparation alone does not earn them.
