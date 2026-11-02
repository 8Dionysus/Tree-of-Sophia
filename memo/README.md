# Tree of Sophia memo port

This port records public-safe, source-linked memory candidates and handoffs for
review by `aoa-memo`. [PORT.yaml](PORT.yaml) defines the `write_candidate_only`
mode and routes. Place candidates, receipts, exports and local notes in their
respective directories when needed.

Review each packet's evidence, privacy, requested route and disposition before
handoff. The receiving `aoa-memo` owner validates, indexes and admits its copy.
[index.min.json](index.min.json) is a generated snapshot; compare its membership
with the packets and read those packets directly when it is stale. The local
port currently has no export or import command.
