# Exact-basis Collection member order

Date: 2026-09-10. Source and implementation review by
`agent:codex-tos-foundation`; no independent human review is claimed.

## Meaning and operation

Entity registry 34 / relation registry 39 introduce a separate qualified
Collection-order literal and `collection_member_order` Claim profile.
Existing Collection → Work `contains_work` remains the membership authority;
the new order cannot create membership. It binds one exact Collection version
and one distinct positive membership Claim version per selected Work.
Those Claims must belong to that Collection's declared membership refs.
The bounded scoped-members adapter retains separate order, scope, coverage,
limits and local acyclicity; competing orders are not combined into a global
hierarchy. A total order requires transitive comparability, not sorted labels.

Current and retained native versions use the existing exact-version readers.
A narrow legacy branch reads only
`collections/<owner>/<collection>/membership-claims.jsonl`, without enumerating
its package or descendants. It preserves raw JSON and fields, verifies the
catalog binding, and explicitly reports no native correction chain. It never
invents an unavailable historical version. Native membership requires explicit
positive polarity; the exact legacy bibliography adapter retains its former
positive membership interpretation when no polarity field exists.

Preparation and graph projection share the same exact-basis verification.
The full qualified value, including version refs, is bound by the separate v4
write delegation. Expected configuration, dependency and input checks,
no-replace publication, exact replay, separate HumanForms and correction
history remain the existing command grammar. Final reader snapshots are
verified before returning publication output. Public graph input digests retain
the selected current or archived source files and history/catalog bindings.

## Real material

The [source reading](2026-09-10-mysl-collection-order-source-reading.md)
records exactly what was read and what was not. The new
[Claim and source package](../source-witnesses/relations/mysl-1996-volume-2-member-order/source-claims.jsonl)
orders the seven existing Works in the retained Mysl 1996 volume 2 map, with
six explicit adjacent precedence pairs and map-scoped exhaustive coverage.
Its Collection basis is version 4, the Zarathustra membership is version 2,
and the other six membership records are version 1. Every original source
byte remained unchanged. This is reported, unreviewed editorial order from a
recorded map, not a fresh PDF inspection, a new membership fact, composition
chronology, translation acceptance or canon decision.

`claims.create` and `form.create` ran under separate exact local delegations;
each exact retry returned its original receipt. The source-copy statement
retains the entire qualified Claim context, with no semantic admission.

Checklist: source return, authored/derived separation, layer distinction,
source language, stable identities, preserved history and uncertainty, and
owner boundaries are **yes**. Canon, personal/lived witness, counterpart,
calibration, translation assessment and external publication are
**not-applicable**. Neither the validator nor this engineering review accepts
the historical order. No stop condition was waived.

## Validation and remaining integration

The exact-version reader passed 17 tests in 0.705 s; the complete affected
bibliographic-topology module passed 26 tests in 1.553 s. Creation/revision/
replay/member-focus and v4 authorization checks passed 5 tests in 45.685 s,
including the new end-to-end order creation and graph rejection when the
bound legacy version disappears. Negative controls include wrong or missing
Collection/Claim refs, duplicate or negative membership, wrong endpoint types,
unavailable versions, shifted digests, inert extension refs, forbidden legacy
paths and source drift. Synthetic fixtures are not historical evidence.

Actual apply-plus-retry took 19.395 s for the Claim and 5.849 s for its form.
Source catalog, bibliography graph and corpus index were rebuilt; the source
foundation validator passed. Registry evolution against exact predecessor
`b66b2836068af8e38f6fb76d77f661daaced349b` passed with no violations.

The actual union reader snapshot
`0c30e6a0fd7c80be17c9357a13fcaf963d4b31644a015bb8f6c811d97b0822fa`
was inspected for the exact Claim, qualified value, form and membership basis.
All eight centers (the Collection and seven Works) returned the Claim on their
first bounded page and through the named structural focus; a property lens
selected its total order and exhaustive map scope. The snapshot contained
42,305 nodes and 62,304 relations. Cold construction took 39.629 s and a warm
load 0.000420 s; inspection took 0.330 s, ordinary foci 0.337–2.28 s, structural
foci 0.0245–0.0377 s, and the lens 0.376 s. The exact transient unit peaked at
1.4 GiB with zero swap. These are observed bounded canary results, not a claim
that the whole application's latency budget passed. The first private probe
used an unsupported profile name and was corrected to the supported `all`
profile before this successful run; no product exception was introduced.

Documentation currentness and source-home checks passed, as did the repeated
source-foundation validation. The combined checking unit peaked at 441.7 MiB
with zero swap. Two attempted standalone topology command names do not exist;
they are not counted as checks. The actual manifest-owned mechanics topology
validator then passed alongside rebuilt documentation currentness (188.7 MiB,
zero swap). Cross-corpus/KAG checks remain with integration.
Combined KAG, Worker/D1, UI, full release gate, CI and
landing belong to the integration owner. No deployment or whole-goal
completion is claimed by this bounded source change.
