# QUESTBOOK.md — Tree-of-Sophia

<a id="frontier"></a>
<a id="near"></a>
<a id="latent--parked"></a>
<a id="latent-parked"></a>
<a id="harvest-candidates"></a>
<a id="backing-files"></a>

This file is the public tracked surface for deferred operational obligations that belong to `Tree-of-Sophia`.

Use it for:
- identifier, source-node, concept-node, lineage-template, and intake-boundary follow-through
- source-first compatibility and public contract debts
- derived export or bridge requests that survive review and need stable IDs
- reanchor or harvest work around the architecture of growth

Do not use it for:
- philosophical truth claims as backlog items
- replacing authored source material or interpretation with task objects
- collapsing ToS meaning into AoA operational language
- private reading notes or living interpretive work that belongs in source surfaces

## Obligation register

This existing register owns membership, current follow-through state, named
owners and return conditions for all five retained records. Closed history stays
addressable. The data block is the same register's mechanically checked form;
its headings, task count and IDs are not core software requirements.

- [TOS-Q-0001](quests/TOS-Q-0001.yaml): done; operational-only surface landed.
- [TOS-Q-0002](quests/TOS-Q-0002.yaml): done; identifier/template hardening landed.
- [TOS-Q-0003](quests/TOS-Q-0003.yaml): captured/manual; source intake owner.
- [TOS-Q-0004](quests/TOS-Q-0004.yaml): captured/manual; derived export boundary owner.
- [TOS-Q-AGON-0001-sophian-threshold-intake](quests/TOS-Q-AGON-0001-sophian-threshold-intake.md): review pending;
  original acceptance obligations retained, closure evidence not supplied.

```tos-obligations-v1
{
  "schema_version": "tos_obligations_v1",
  "runtime_execution": false,
  "canon_admission": false,
  "records": [
    {
      "id": "TOS-Q-0001",
      "state": "done",
      "owner_ref": "mechanics/questbook/AGENTS.md",
      "source_return": "ToS/AGENTS.md",
      "record_ref": "quests/TOS-Q-0001.yaml",
      "record_kind": "legacy_yaml",
      "return_when": "Reopen only on reviewed drift of the operational-only obligation surface; the Phase 2 closure and evidence remain in the original record.",
      "history_ref": "https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/quests/TOS-Q-0001.yaml"
    },
    {
      "id": "TOS-Q-0002",
      "state": "done",
      "owner_ref": "ToS/AGENTS.md",
      "source_return": "ToS/AGENTS.md",
      "record_ref": "quests/TOS-Q-0002.yaml",
      "record_kind": "legacy_yaml",
      "return_when": "Reopen only when identifier or node-template review establishes new drift; preserve the Stage 5 closure and source wording.",
      "history_ref": "https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/quests/TOS-Q-0002.yaml"
    },
    {
      "id": "TOS-Q-0003",
      "state": "captured",
      "owner_ref": "ToS/candidate-intake/AGENTS.md",
      "source_return": "ToS/candidate-intake/AGENTS.md",
      "record_ref": "quests/TOS-Q-0003.yaml",
      "record_kind": "legacy_yaml",
      "return_when": "ToS maintainer opens a bounded obligation manually when a new intake or counterpart route survives current source review.",
      "history_ref": "https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/quests/TOS-Q-0003.yaml"
    },
    {
      "id": "TOS-Q-0004",
      "state": "captured",
      "owner_ref": "mechanics/boundary-bridge/AGENTS.md",
      "source_return": "ToS/derived-exports/AGENTS.md",
      "record_ref": "quests/TOS-Q-0004.yaml",
      "record_kind": "legacy_yaml",
      "return_when": "ToS maintainer returns a bounded derived export request only after explicit review approval and downstream-owner acceptance; no runtime activation follows.",
      "history_ref": "https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/quests/TOS-Q-0004.yaml"
    },
    {
      "id": "TOS-Q-AGON-0001-sophian-threshold-intake",
      "state": "review_pending",
      "owner_ref": "mechanics/agon/AGENTS.md",
      "source_return": "ToS/candidate-intake/AGENTS.md",
      "record_ref": "quests/TOS-Q-AGON-0001-sophian-threshold-intake.md",
      "record_kind": "authored_note",
      "return_when": "Agon/ToS maintainer reviews the original builder-check, validator, deterministic-registry and no-direct-write acceptance evidence. The old note contains no closure event; do not infer done.",
      "history_ref": "https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/quests/TOS-Q-AGON-0001-sophian-threshold-intake.md"
    }
  ]
}
```

## Retained compatibility consumer

`tos-ops-mechanics-plan` native Questbook assertions consume the original four
`quests/*.yaml`, the [quest schema](mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json),
[dispatch schema](mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json),
[catalog example](mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json)
and [dispatch example](mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json).
These exact public records retain their original bytes and historical evidence.
The retained projections are optional for a core register; when present they must
match registered legacy records exactly. No SDK runtime consumer for ToS has been
established by this package. Unknown external readers are not excluded: the old
paths remain readable. A named consumer and its compatibility review are required
before extending these exports or removing their entrypoints.

The [integration boundary](mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md)
keeps philosophical meaning with ToS. Membership is not dispatch, activation,
rights admission or canon authority. New obligations need an actual owner and
return condition, not a demonstration quartet.
