# Versioned HumanForm delivery

This is the portable `access/` transport contract for selected HumanForms. It
does not change the ToS-owned
[`tos_human_form_materialization_v1`](../../ToS/contracts/human-form.schema.json),
source records, assessment journal, admission policy, or exact inspection.
The executable envelope schemas live in
[`knowledge-graph.v1.schema.json`](knowledge-graph.v1.schema.json#/$defs/humanFormSelection).
The public knowledge contracts operation exposes those schemas and the version
map in [`knowledge-api.v1.json`](knowledge-api.v1.json).

## Versions and migration

`tos_human_form_selection_v1` carries each ready role's complete materialization
at `roles.<role>.packet`. `tos_human_form_selection_v2` carries the same logical
selection with exact common packet values factored out. V2 is a wire encoding,
not a shorter materialization or a stronger assessment. Both envelopes remain
recognized; an unknown version is unavailable, never guessed to be v1.

The Python and Worker selectors retain an explicit `inline-v1` option for
direct callers and use `shared-v2` for lens carriers. Lens execution v7 marks
this consumer transition. Before activating that producer, a consumer must
support both versions at every semantic access point: scene labels, hover,
compact Claim reading, full reading, history, and restored saved places.
Retain the received wire envelope for transport-budget checks and restoration;
decode a separately bounded logical selection before consuming its packets.
An expanded v2 selection is not a new v1 wire payload subject to the v1 16 KiB
wire test. Do not replace the source materialization with its encoded delta.

The dependency-free browser/Worker codec is
[`access/shared/human-form-selection-codec.ts`](../shared/human-form-selection-codec.ts);
the Python port is
[`tos_access.human_form_codec`](../src/tos_access/human_form_codec.py).
Decoding establishes transport structure and bounds only. Existing checks of
the source packet, selected exact form, role, language, subject, carrier
revision, mandatory context and source-snapshot admission still apply. A saved
source-snapshot admission is not a freshly evaluated runtime grant.

## Exact reconstruction

The v2 envelope retains the v1 selection metadata, candidates, issues and
seven role names (`name`, `caption`, `hover`, `statement`, `grounds`, `history`,
`technical`). Each role retains `state`, `reason`, and exact `form`; its wire
payload is `packet_delta`. A non-ready role has a null delta. A ready role has
an object delta, including `{}` when its whole packet is shared.

Each ready role's `form` is an exact ID/version/digest reference. Before
factoring the packet, the encoder verifies that its own `form` is exactly equal
to that reference, including JSON types and the absence of extra fields.
It then omits that redundant packet field on the wire. The decoder restores
an independent copy from the role reference. An inline `form` in either
`packet_base` or a ready `packet_delta` is invalid, even when equal; missing or
conflicting exact references fail closed, not by choosing one as authoritative.

`packet_base` is the recursive intersection of JSON-identical values present
in every delivered ready packet after that factoring and the limits encoding
below. Arrays are shared only when wholly equal;
they are never sorted, intersected or unioned. The per-role delta contains
complementary fields. Reconstruction recursively joins disjoint object
members and rejects overlapping leaves, even if their values are equal.
Absence, null, empty objects, empty arrays, strings, booleans and numbers keep
their JSON meanings. The encoding does not preserve insignificant JSON
whitespace or object-key order; it does not repair numeric precision already
lost by a consumer's JSON parser.

Before factoring common values, each packet's own `admission.limits` is
replaced by ordered integer `admission.limit_refs`. `shared_limits` contains
each literal string once, in first encounter order across the fixed role
order and original limits order. Restore every packet's individual limits
from those references, including repeated strings and original order. This
is not a union of admissions. An absent admission, null admission, absent
limits, and empty limits remain distinct. Null or non-string-array limits are
invalid. `admission.limit_refs` is reserved to this encoding and must not be
present in an input source packet. No inline limits remain in encoded packets.

The allocator recomputes the base and pool for the ready roles actually
included, retaining the existing role/language priority. If a complete packet
cannot fit, that role retains its exact inspection reference and an explicit
`over-budget` state. No qualifier, context entry or admission limit is trimmed
to fit. Candidate selection still does not adjudicate competing forms.

## Bounds and malformed input

The entire wire envelope retains the existing conservative 16,384-byte JSON
delivery budget. The codec additionally bounds each reconstructed packet to
65,536 conservative bytes, the expanded selection to 524,288, structural
depth to 64 and visited members to 30,000. These are delivery protections,
not a change to source admissibility. The pool has at most 512 distinct
strings. Invalid indices (including booleans and out-of-range values), unused
pool entries, duplicate pool strings, missing roles, state/delta mismatch,
conflicting leaves or form references, unsafe object keys, non-JSON values and expansion beyond
the bounds fail closed. A malformed or unsupported envelope must not yield
partial ready wording or silently fall back to a guessed reconstruction.

## Compact Claim reading

The v1 reading mode `claim-with-mandatory-context` keeps its existing pointers
to complete role packets or source-bound display fields. The explicit v2 mode
`claim-with-shared-form-context-v2` points to the raw ready role envelope, for
example `/human_form_selection/roles/caption`. Resolve it only through its
enclosing v2 selection and reconstruct the full packet before reading.
`packet_delta` alone is never wording. The node/content revision binding,
`/semantics`, `/epistemic`, relation-context IDs and `standalone: false` remain
mandatory. A display-field fallback still uses the v1 reading mode; it does
not pretend that a shared-form packet exists.

Rollback switches a derived producer/reader to the preceding compatible
execution and envelope pair. It does not remove sources, HumanForms,
assessments or admission history. Existing v1 saved selections remain
readable with their original limits and validation; v2 selections require a
v2-aware reader or an explicit unavailable state.
