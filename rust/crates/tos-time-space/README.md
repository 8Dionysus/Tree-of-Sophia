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

Current documentary selections, exact source hashes, historical corrections and
review status are maintained in
[the package contract](CONTRACTS.md#current-historical-correction-and-remaining-bindings).
This crate contains no historical entity-ID bindings and does not treat its old
examples as validated native inputs.
