# `tos-foundation`

Small, OS-free mechanical types for the Tree of Sophia Rust migration. This crate does not read paths, select a corpus, validate an authored schema, grant access, assess evidence, or admit source. Owner packages supply versioned descriptors and enforce their own authority.

`JsonMode::PublishedStrict` rejects duplicate decoded member names. `JsonMode::RequestLastWins` retains the first key position and the final value/number lexeme. Both retain integer versus float kind and member order. Escaped lone surrogates survive parsing and preserved JSON output, while a canonical UTF-8 byte profile refuses them. `JsonLimits` makes input size, depth, visits and integer digit ceilings explicit.

Canonical profiles are distinct: `CorpusSnapshotV1` uses sorted compact JSON with one trailing LF, while `SourceRecordDigestV1` and the legacy `SourceCommandInputV1` use sorted compact JSON without an LF. The latter hashes a strictly parsed source command, not an access request with last-member-wins behavior. Finite floats use Python's shortest-round-trip spelling and fixed/scientific exponent layout; arbitrary integer lexemes never pass through binary64. Profile names are versioned and unknown names fail with `unsupported_format`. The owner must still verify the selected profile's applicability and independently compare native/WASM vectors before migration cutover.

Legacy `public-source-forms apply` receipts are embedded in a whole form-set history file written with unsorted two-space indentation and a final LF. That is a separate byte contract; no standalone receipt canonicalizer is inferred from the command input profile. Current raw receipt bytes remain authoritative until the owner freezes an exact history serializer and vectors.

`Digest256` hashes exact bytes; a digest alone proves neither source trust nor permission to read. `RelativePath` checks lexical normalization only; storage must check actual file identity and symlinks. `StableId` preserves spelling and does not guess a corpus kind. `ByteSpan` and `CodePointSpan` have distinct units; code-point spans bind a representation digest and declared normalization ID.

The target `wasm32-unknown-unknown` builds the same core rules. Optional `wasm` exports a small parity probe; Worker and browser I/O stay in their respective adapters. Independent vectors are owned by `tests/conformance/rust/`.
