# Release Support Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | release-facing source-home, decision, generated, or validator surface changes |
| input | release check, decision index, source-home validator |
| output | checked release route or failing owner surface |
| owner | `mechanics/release-support/` |
| stronger route | root release docs and scripts own release commands |
| next route | [Source Release Gate](parts/source-release-gate/README.md) |
| validation | `tos-release-check --repo-root "$PWD"` |

## Operations

Software release follows the Source Release Gate and existing release contract.
ToS verifies its native software and bounded exports. Consumer provider/index/
trust/admission belong to consumers under
[TOS-D-0072](../../docs/decisions/TOS-D-0072-repository-independent-execution-and-exports.md);
this package does not orchestrate them.

Each part owns one release-support operation route.


| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Source Release Gate](parts/source-release-gate/README.md) | route release-facing checks through source-home and decision validators | root release docs, validation lanes, and owning source surfaces |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts


## History

No package-local `legacy/` archive is maintained.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/release-support/PROVENANCE.md) preserves the pre-compaction route.
The retired consumer route remains at
[its exact prior source](https://github.com/8Dionysus/Tree-of-Sophia/tree/ff030ff4c3cc6ed84e49beccdc8d3c179f014ffb/mechanics/release-support/parts/artifact-bundles).
Its deletion is not a paused ToS feature to reactivate.

Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Release Support is active as source-release posture. It helps public-facing
ToS changes leave a clean route without turning release machinery into meaning.

### Current Contour

- Keep source-release-gate around release-facing ToS surfaces and public claim
  posture.
- Keep exact native software/data export checks at ToS owners; downstream
  signatures, registries, trust policy and admission stay consumer-owned.
- Keep release support below source, branch, canon, and decision authority.
- Keep public support claims tied to the ToS surface they support.
- Keep graph projection, atlas projection, and public compatibility claims
  release-supportable only when their source surfaces and checks agree.

### Next Work

- Clarify release support only when ToS repeatedly needs the same public
  support posture, rollback posture, or downstream handoff.
- Keep release direction in root/project surfaces and package future pressure
  in package roadmaps.

### When Time Comes

Time has come when: release receipts, source exports, or public support
reports become repeated reviewed objects.

Move: add payload to `source-release-gate`, or split
public support, rollback posture, and downstream handoff into narrower parts.

Guardrail: release support can defend a public claim; source meaning remains
with source, branch, doctrine, review, and canon surfaces.

### Out Of Scope

- GitHub-only release definition.
- Public claim without validation.
- Source meaning changes.
