# Tree of Sophia memo port

Record public-safe, source-linked memory candidates for `aoa-memo` review.
[PORT.yaml](PORT.yaml) defines `write_candidate_only` and the routes; place
candidates, receipts, exports and local notes in their respective directories.

Review each packet's evidence, privacy, requested route and disposition before
handoff. `aoa-memo` validates, indexes and admits its copy. Compare the generated
[index.min.json](index.min.json) with the packets and read the packets directly
when the snapshot is stale. Handoff currently uses the packets directly.
