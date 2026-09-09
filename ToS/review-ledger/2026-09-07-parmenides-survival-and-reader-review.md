# Textual survival and generic structured values: implementation review

Reviewed on 2026-09-07 by `agent:codex-tos-foundation`, against the change
following `0660ac562a32fc24f044e9517212b598b3134d9b`.
This is source/command/reader integration evidence, not independent historical
assessment, scoped admission, publication or Foundation v1 completion.

## Source-owned distinction and execution

The relation registry now declares `structured-value-v1`: one specific
identity/semantic domain and one concrete mapped literal subtype. The shared
grammar requires a declared kind and nonempty language/script-bearing source
wording, independently of the domain schema. A profile cannot repurpose its
reader or value kind under the same identity. Unrecognized nested fields remain
data; keys named `date`, `places` or `relative` do not confer temporal, spatial
or identity-reference semantics on this reader.

The first profile, `textual_survival`, describes survival of an intellectual
object's text. Complete, fragmentary, not extant and unknown are reported
values, not permanent Work types, confidence levels, access decisions or
destruction of every copy. Scope and coverage notes are mandatory. Equal values
in different Claims retain separate Claim identities and their qualification
contexts; the value does not acquire the Work or Claim identity.

The ordinary separately delegated create/revise commands support exact values
through v3 configurations. Old identity-only and temporal-only grants do not
silently widen. Review found and corrected a scope hole: a relative temporal
anchor equal to the subject still requires separate object permission. Current
profile and anchor permissions apply before exact replay, including after
revocation or replacement of a v3 grant with v2. The negative self-anchor test
failed before the correction and then passed with the positive creation,
correction and denied-replay cases.

## Real source package

The native Agent `tos.agent.parmenides-of-elea` and Work
`tos.work.parmenides-of-elea.poem` were created through ordinary source commands,
without fabricated Expression, Edition, Item, manuscript or exact text.
Separate scholarly-report Claims describe authorship and fragmentary textual
survival. Their [frozen reading note](2026-09-07-parmenides-survival-source-reading.md)
records the exact Palmer SEP archive and inspected section. The note is an
immutable creation input; new findings must not rewrite it.

Preparation, creation and exact retry took 6.616 and 6.308 seconds for the two
subjects and 6.312 seconds for the two-Claim batch. Six subject forms and two
Russian Claim statements materialized as ready source copies. All remain
unverified, unreviewed and unadmitted; the bilingual descriptions are authored
paraphrases, not accepted translations. The retained creation receipts bind
their actual implementation inputs, not subsequent fixes to the commands.

## Ordinary reader and queries

The real `ToSAccessCore` retained the exact subject records in both source
carriers, Russian/English names, Russian hover descriptions, exact Claims and
their statements. The typed survival value retained source wording and the
governing Claim's uncertainty and qualifiers. Neither carrier gained canon
status or stronger authority. Assessment input resolution returned four exact
records with one bibliographic origin; no assessment was invoked.

Depth-2 focus bounded to 80 nodes/150 relations returned Agent 5/4, Work 7/7
and survival value 4/3 nodes/relations, with one selected scene vertex each.
The repeated local probe observed 0.290, 0.299 and 0.279 seconds respectively;
cold construction was 24.466 seconds and peak RSS 1,277,748 KiB. Source snapshot:
`d0d76db58e5be7a3bbc91e920eef614118c0a0807fb26023ee6d57d1f16367e3`.
These are single local observations, not p95, performance acceptance or actual
UI interaction. Later documentation rebuilding may change the snapshot.

After rebuilding the source catalog and bibliographic graph, a reproducible
query uses `access/src` on the Python path:

```python
from tos_access.core import ToSAccessCore
core = ToSAccessCore.discover('.')
result = core.compile_knowledge_lens({
    'schema_version': 'tos_lens_spec_v1', 'lens_id': 'fragmentary-texts',
    'node_query': {'filters': [{
        'property_id': 'tos.property.textual-survival-status',
        'op': 'eq', 'value': 'fragmentary'}]},
    'relation_query': {'enabled': False}, 'detail': 'compact', 'explain': True})
```

Discover the property through `knowledge_catalog()['semantic_registries']`.
The real probe selected this record for `fragmentary`, not for `not_extant`
or `unknown`. This queries a recorded report, not historical truth. Inspect
the Claim context before interpreting the result. Agent and Work can be
focused directly by their stable IDs; the literal uses its returned node ID.

## Verification and remaining boundaries

The complete bibliographic graph module passed 72 tests; the access contract
module passed 59, processing dependency tests 16 and source command tests 37.
The focused new value/permission creation and correction tests passed 3 tests
in 28.203 seconds; the final full Claim create/revise module passed 25 tests
in 205.748 seconds, including profile-permission downgrade on replay.
Source foundation, bibliographic graph, source home, catalog and graph parity,
corpus index parity, documentation-family currentness/cross-corpus guards,
route currentness and all 56 nested agent cards passed.

Manual boundary review: yes to source return, attributed scope, Claim/value
separation, explicit unknown coverage, language-neutral identity, preserved
qualifiers and authority checks. Not applicable to this change: canon,
counterpart, calibration, lived-witness consent, public payload, deployment or
UI mutation. Synthetic values and malicious-looking extension prose are tests,
not historical evidence or executable instructions.

B03 remains partial. The next ToS source/profile work is independently
addressable textual fragments, surviving quotations and scholarly textual
reconstruction, with grounded relations and exact text/witness routes.
Competence-qualified independent source assessment remains the next content
owner; no qualified reviewer was launched in this change. Full source scans,
Worker/D1, actual UI interaction, CI, merge and deployment are not resolved by
this profile. Rolling back a derived reader must preserve all new source
packages, form history and receipts; corrections use owner commands.
