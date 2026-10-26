# Decision Records

Decision records explain a durable choice: a route, topology, owner boundary,
export posture, validation rule or rejected alternative that a future
contributor should not have to rediscover. Current behavior and meaning remain
with the source that the decision cites.

Use the owning guide for instructions, the review ledger for dated judgments,
and the changelog for release history. An ordinary documentation edit does not
need a new decision record.

## Find a decision

The generated [indexes](indexes/README.md) offer these views:

- [canonical ID](indexes/by-number.md) and [date](indexes/by-date.md);
- [surface](indexes/by-surface.md), [ToS layer](indexes/by-tos-layer.md) and
  [tree class](indexes/by-tree-class.md);
- [guard family](indexes/by-guard.md).

Indexes are lookup aids derived from decision metadata. They do not replace
the rationale or establish that an implementation is current.

## Write or supersede

Start from [TEMPLATE.md](TEMPLATE.md) and the [local card](AGENTS.md).
Use a stable `TOS-D-####` ID and the filename
`TOS-D-####-short-decision-slug.md`. Keep the title short and record context,
alternatives, rationale, consequences, exact source surfaces and validation.

The `## Index Metadata` block carries the decision ID, original date, surface
classes, ToS layers, tree classes, guard families and posture. Preserve existing
IDs. When a choice changes, add an explicit superseding record and state the
scope of supersession; historical records retain their original rationale.

## Regenerate and validate

After changing decision metadata, use the native index builder and decision
validator in [documentation validation](../VALIDATION.md). Generated indexes
are rebuilt rather than edited by hand. Validate the changed source through
its own owner route as well; index parity does not accept the decision.
