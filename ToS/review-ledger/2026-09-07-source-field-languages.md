# Explicit language of source metadata

Scope: optional `field_languages` in corpus/historical source records and its
existing metadata-form adapter. This review does not assess translation quality
or certify the language of the current corpus.

The observed gap was an authored Russian historical name/note materializing
with null language. Inferring from the text, ID suffix or Expression language
would confuse source wording with consumer locale. The source now has an
explicit typed declaration per preferred label/note. The whole declaration,
including unknown qualifications, becomes mandatory context for a source-copy
form; it does not declare original/translation status or grant admission.

The new checks first failed on dropped language and accepted malformed metadata.
After implementation, 25 source-command tests passed in 16.888 seconds,
21 pure human-form tests in 0.179 seconds, and 49 graph tests in 43.869 seconds.
Tests cover actual CLI prepare/apply, retained predecessor forms, missing
metadata binding refusal, null and extensible tags, source-schema negatives,
and the existing historical graph/access reader. Historical fixtures are
synthetic and not assertions about the actual corpus.

All 172 current supported corpus/historical records pass their updated source
schemas. Current catalog parity, rebuilt final graph validation and source-home
checks pass. No authored source record or persisted human form was rewritten;
the first real historical episode therefore still needs an explicit source
revision and successor forms before it gains these language declarations.
Closed earlier readers must update rather than discard the additive field.
Reader rollback must preserve source history and the original creation receipt.
The 26 script/test-topology tests passed in 2.203 seconds. Comparing the rebuilt
graph against the previous commit changed only input digests and the projection
fingerprint: all existing nodes, edges and claim traces were identical.

Source traceability, layer distinction, retained qualifications, stable identity
and absence of implicit assessment are preserved. UI design, external artifact
admission, primary-letter assessment, full source revision commands, CI, merge
and deployment are not established by this slice. Next owner: source growth
for actual versioned corpus migration, with linguistic derivation and quality
remaining separate requirements in `ToS/doctrine/FOUNDATION_V1.md`.
