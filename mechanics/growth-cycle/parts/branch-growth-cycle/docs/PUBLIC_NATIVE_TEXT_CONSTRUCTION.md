# Public project-text native construction

This independent adapter implements bounded public source construction through
`source_commands.py`. Its closed, source-owned input contracts are
`ToS/contracts/public-native-text-create-owner.schema.json` and
`ToS/contracts/public-native-text-authority.schema.json`. Request discovery
uses the common grant-free handler grammar; the adapter requires its dedicated
public-construction grant.

## Scope and authority

The protected current-account mode-0600 grant names one source root, disjoint
mode-0700 recovery root, new exact package path, maker, expiry, explicit
Work/Expression/Edition/Item/File records, Item manifest, existing authored
UTF-8 documentation bytes and code-point range, native identities, rights,
license evidence and retained operator-scoped publication authority. Only
`ToS/review-ledger`, `ToS/doctrine` and `docs` source selections are supported.
Reserved private, payload, local-content, catalog and hidden paths are refused.

The source must already have been retained by normal corpus commands. The
original Item rights remain mandatory inputs and are not rewritten. Every
output-bearing rights record must affirm the exact new layer/representation
File, public payload visibility, redistribution and derivation, with retained
license and publication-authority evidence. Denied, inactive, missing, foreign
or drifted evidence fails before source text is opened. Parsing does not
authenticate authorship or manufacture licensing: the source-owned authority
records a real operator delegation and exact authorized output scope.

## Construction and downstream use

`describe` does not open source text. `prepare-create` returns exact prepared
configuration/dependency hashes and native bindings. `native-text.create`
requires those hashes, null expected prior source/revision and an exact
command ID. It constructs a literal UTF-8 range without newline, Unicode,
markup, spelling or punctuation rewrite; the source selector returns to the
original acquired File. The new layer remains an unreviewed machine
transcription with structural-extraction provenance. A separately supplied
finite partition covers the new representation with units and explicit gaps;
it does not infer language units or semantic significance.

The native resolver verifies the new public binding and exact content. A
separate `source.create` can then create an Occurrence with its own identity
and native binding. HumanForm creation, assessment, admission, canon and
external publication follow their own operations. Existing private historical
inputs remain private.

## Persistence and limits

The common no-replace native package installer retains the exact request,
public plan (not the protected grant), runtime, inputs, execution provenance,
native bindings and receipt. The public plan omits the grant's source/recovery
root fields and local UID; the protected grant is represented only by its
digest. This is not a general redactor for authorized authored source text.
Up to 128 KiB source and output, 60 seconds, 128 distinct input files,
16 MiB input bytes, 12 output files and 2 MiB output bytes are allowed.

Source and recovery locks, no-follow reads, current authority/rights/input
rechecks and atomic new-directory publication protect commit. Exact retries
return retained bytes. `inspect-recovery` addresses one command and compares
an installed package with its retained plan before reporting committed. An
interrupted exact plan can resume through the same create request; torn or
foreign evidence is preserved and refused, never automatically deleted or
overwritten. The shared installer preserves original private writer controls.

## Verification boundary

`test_source_public_native_commands.py` covers exact public native resolution,
distinct Occurrence validation, Unicode/newline preservation, no path leakage,
grant/rights/authority rejection, dependency drift, identity/partition errors,
precommit changes, interrupted resume, torn evidence and installed corruption.
Fixtures are synthetic mechanics evidence, not real corpus or rights review.
Existing discovery, private construction and pure proposal suites remain the
regression surface. Real source-visible judgment and retained operator scope
must be reviewed separately from green tests.
