# Operational identity correction: Plutarch De fato

The initial acquisition stopped after 26 complete targets. The 27th target retained its Greek payload and wrote incomplete metadata, then refused to append a claim whose identifier already belonged to Cicero De fato. The existing Cicero claims and discovery file were not replaced.

The initial generic target slug `de-fato` was insufficient for a global operation namespace. The corrected operational slug is `plutarch-de-fato`; Work, Expression, Edition, Item, payload path, CTS and source bytes retain their original stable identities. The three bibliographic claim identifiers and acquisition/discovery operation identifiers become author-qualified. This is not a new Work or an ancient authorship judgment.

The exact eleven incomplete metadata files are retained in preimages/ with their original refs and SHA-256 in preimages.json. They were not yet catalogued, branch-linked or admitted. The live partial metadata was moved there so the normal acquisition owner can reissue a coherent package from reviewed corrected preparation. No corpus payload was moved, removed or edited. Actual transfer evidence remains in the existing acquisition-transfers.jsonl; local reuse must verify those bytes and the original transfer receipt.

manifest.json and prepared-source-packages.jsonl remain frozen initial preparation. manifest.corrected.json carries the same 45 exact versions, changing only this operational namespace and its source evidence links. The first 26 successful acquisitions retain the original preparation evidence; resumed acquisitions cite the corrected preparation. Registry coverage may continue to enumerate the original 45 stable Item identities, while post-acquisition reading routes use the corrected operational identity.

The engine also gains a pre-write collision check, so future equivalent namespace failures are rejected before payload or metadata mutation. Source custody remains distinct from textual, semantic, canon and publication acceptance.

Retained historical metadata filenames use `.preimage` so generic source/provenance scanners cannot promote the failed attempt back into current authority. Bytes and original paths remain fixed by preimages.json.
