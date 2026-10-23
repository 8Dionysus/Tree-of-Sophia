# Time/Space domain implementation v1.0.1

This crate implements the accepted bounded TS v1.1 patch proposal as isolated
Rust source. It depends on the single common foundation owner. It supplies
exact interval/order, joint-carrier projection, temporal paths, state filters,
and Cartesian rational rectangle/point topology. It does not issue native
profiles, source authority, semantic assessment, admission or complete manifests.

Original source-delivery status: **source-only implementation, compilation and native tests not
executed**. The selected environment has no ordinary cargo/rustc/rustfmt and no
validation-lanes command. No toolchain was installed or run from an archive.
Source review acceptance and required green gates remain necessary before merge.

v1.0.1 addresses source-review HOLD TS-CENSOR-1. Observed positive windows are
separate from necessary actual-extent constraints: after an observation cutoff,
state validity remains unresolved; disjoint observation windows do not discard
a possible path. Only disjoint actual-extent constraints permit that rejection.
See the contract and the single addressed censoring regression for details.

Prerequisite: common K0 implementation commit
`8f05ccd738d622ca47e9db47dd9844ee93759a5b` (source package v4, SHA256
`9efa367a208978af4b52b110d5d6215bba4ea10cc2e4c7fb44b1cd6063061186`).
This crate imports its actual `CheckedOperationBinding`, `ExecutionStatus` and
`ValueDisposition` exports. Base checkout `433cb19f4d621dfdf4a98fe2a7ee202e66f8e388`
does not contain K0; the package must be composed with that prerequisite.
No replacement common types or local K0 stand-in are provided.

The exact integration hunk for root Cargo.toml/Cargo.lock is supplied outside
this crate to the common owner. Shared registration, foundation lib.rs, schema,
doctrine, source records, PR267/calendar/Text, SVB and mirror are untouched.

`CONTRACTS.md` defines actual supported input/output laws, errors, costs, source
bindings, unresolved integration and the two addressed implementation deltas.
`tests/time_space_native.rs` is one native target: seven existing oracle slices,
three accepted deltas, and small regressions specific to this port's new code.
Its tests are authored source, not passed results or historical validation.

Current documentary selection: full matrix v0.3.3, Library
`libfile_ca5cae5e2a6081918cc873ecce1dbd04`, version0, 1837085B, SHA256
`f2bfab950875eb602a577319c88121e851e0ee8f215e0073385c695bbdca23d3`, together
with TS B2 erratum v1.0.1, Library `libfile_8b5ecd7c7ab481918f57ada843dc0cc5`,
version1, 83932B, SHA256
`26d614416b3826475cb1cdcec083fde255d64994fb055482a73e00ca5e4f300b`.
Its `current_implementation_dependency_override` selects these dependencies for
exact accepted TS package version1 (344893B, SHA256
`f7d552ab16cd831162f9ff67a54da7ea10065c45d138f86f36bb68ae0ba630a9`).
The package and its embedded original files retain their raw integrity hashes;
this integration note does not pretend those historical files contained the update.
Root reports Astra R5 full documentary ACCEPT, B5/N13 closed: review Library
`libfile_11cdd7a5619081919324f71c5791390a`, version4, 215531B, SHA256
`66427ecf9ecf292acfee28788d9deeae1610425fe75a8c662e946cdcd7ce4663`.
This closes the earlier documentary HOLD, not the separate native/runtime gate.

Historical source-delivery dependency: B2 erratum v1.0.0, same Library item
version0, 74461B, SHA256
`5dd0bf0f2c46cb42c4472d45f5505f7256c7f0faa397e3737b021de007daa662`,
and matrix v0.3.2 are superseded as current selections, preserved as original bytes.
Old TS v1.1 snapshots remain unchanged history; E009=EHESS is withdrawn, and
R282 year/status stays unresolved. This crate contains no historical entity-ID
bindings and does not treat the old examples as validated native inputs.
