# Retained Mysl 1996 Collection order: source reading

This agent reading concerns the already tracked metadata map, not a new PDF
inspection, accepted transcription, rights review or canon decision.

## Exact basis

The source Collection is
`tos.collection.friedrich-nietzsche.works-in-two-volumes-volume-2-mysl-1996`,
record version 4. Its unchanged metadata bytes have SHA-256
`acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af`.
Its sibling `membership-claims.jsonl` has SHA-256
`0cdba2d535721afab470c98daaf4a7a2c69067429039a1415a57bda5eb6512a8`.
These are raw-file hashes, not canonical record digests.

The [retained work-boundary map](../source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/work-boundary-map.json)
is map version 1, SHA-256
`e2f66f463fff651a68258dda1dfc6e77bd7d040991f11f2a0965218925d7d60e`.
It attributes the following seven-work sequence to the exact 831-page local
Item/File named in that map:

| Sequence | Work | Container pages |
| --- | --- | --- |
| 1 | Also sprach Zarathustra | 5–237 |
| 2 | Jenseits von Gut und Böse | 238–406 |
| 3 | Zur Genealogie der Moral | 407–524 |
| 4 | Der Fall Wagner | 525–555 |
| 5 | Götzen-Dämmerung | 556–630 |
| 6 | Der Antichrist | 631–692 |
| 7 | Ecce homo | 693–769 |

All member boundaries are explicitly `inferred` and `unreviewed` in that map.
It separately identifies notes/reference apparatus on pages 770–830 and the
digital-container colophon on page 831; those are not additional Work members.
The map's source-visible inspection claim belongs to its recorded maker and
event, not to this later reading. This pass checks the recorded sequences,
unique Work references, exact membership references and order consistency.
It does not independently verify those page numbers against a PDF.

## New attributed order and its limits

The new `collection_member_order` Claim selects exactly the map's seven Work
members. Its total precedence is the six adjacent pairs in the explicit
sequence, not JSON array order or alphabetical order. Coverage is exhaustive
only for the seven Work entries declared by this exact map, not for every
intellectual component of the publication or the author's oeuvre.

The value binds the exact Collection record version and all seven exact
`contains_work` Claim versions. The Zarathustra membership is version 2; the
other six are version 1. Membership is reused, not rewritten or created anew.
The unchanged legacy stream has no fabricated native correction history.

The new statement remains reported and unreviewed. This review does not confer
source-text, linguistic, translation, semantic, rights, publication or canon
admission. No source payload bytes were read, copied into the Claim or exposed
through access. Competing order Claims may coexist; a missing exact basis
cannot be replaced by a newer record without an explicit new revision.
