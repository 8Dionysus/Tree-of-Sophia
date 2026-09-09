# Native Artifact shared writer seam review

Review date: 2026-09-09. Baseline:
`3ab7098604ec26c693bfe9fa5862c98c689b9a39`.

This bounded handoff prepares shared creation mechanics for the separately
owned native Artifact adapter. It does not register that adapter, admit a
physical witness, acquire payloads, issue rights, publish or deploy anything.

- The exact new Artifact grant routes initial validation and serialization
  to its fixed adapter. The original `artifact_id` remains the source identity;
  Corpus and historical creation grants remain unchanged.
- The immutable request retains exact rights/discovery/research byte bindings.
  Shared scope checks compare them with the independently selected grant even
  on an original retry; no mutable grant is used as historical evidence.
- Native Artifact replay reads only the record, forms, history and its exact
  captured companions, including when no correction history exists. Unrelated
  descendants and original source provenance are neither scanned nor changed.
- Native serialization receives its explicit procedure name and adapter
  software binding; it remains weaker than research, review, rights and canon.

The focused native-metadata and versioned Collection compatibility selection
passed **6 tests and 12 subtests**, including a new synthetic Artifact replay
test with guarded descendant enumeration and an actual native identity.
The prior broader creation/reader/responsibility/topology run also completed:
**93 tests and 502 subtests** passed; this completes the pending local run named
in the preceding Collection review. No failed result was waived.

The Artifact adapter owner must supply registration and full creation/input
closure tests before declaring Artifact creation complete. The integration
owner retains cross-branch validation and landing responsibility. No CI,
merge, source acceptance, rights approval or runtime health is claimed.
