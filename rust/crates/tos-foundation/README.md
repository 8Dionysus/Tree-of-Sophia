# `tos-foundation`

Small, OS-free mechanical types for the Tree of Sophia Rust migration. This crate does not read paths, select a corpus, validate an authored schema, grant access, assess evidence, or admit source. Owner packages supply versioned descriptors and enforce their own authority.

`JsonMode::PublishedStrict` rejects duplicate decoded member names. `JsonMode::RequestLastWins` retains the first key position and the final value/number lexeme. Both retain integer versus float kind and member order. Escaped lone surrogates survive parsing and preserved JSON output, while a canonical UTF-8 byte profile refuses them. `JsonLimits` makes input size, depth, visits and integer digit ceilings explicit.

The only canonical profile in this first package is `CorpusSnapshotV1`: sorted keys, Python-compatible compact JSON, one trailing LF. It currently refuses all float values with `unsupported_canonical_number`; `capabilities()` reports this limit. Corpus snapshot revision verification may be enabled only for inputs within the profile's verified range. A later float formatter needs independent Python/native/WASM parity before the capability changes. Other JSON canonicalization schemes must have their own named profiles and fixtures.

`Digest256` hashes exact bytes; a digest alone proves neither source trust nor permission to read. `RelativePath` checks lexical normalization only; storage must check actual file identity and symlinks. `StableId` preserves spelling and does not guess a corpus kind. `ByteSpan` and `CodePointSpan` have distinct units; code-point spans bind a representation digest and declared normalization ID.

The target `wasm32-unknown-unknown` builds the same core rules. Optional `wasm` exports a small parity probe; Worker and browser I/O stay in their respective adapters. Independent vectors are owned by `tests/conformance/rust/`.
