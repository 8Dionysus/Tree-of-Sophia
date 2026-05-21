# AGENTS.md

Root route card for `Tree-of-Sophia`.

## Purpose

`Tree-of-Sophia` is the canonical source-first knowledge architecture for philosophy and world thought.
It owns source-linked authority, authored tree meaning, canonical nodes, relations, vocabulary governance, and bounded public/export seams subordinate to the tree.

## Owner lane

This repository owns:

- source discipline, interpretation law, and authored tree structure
- source files, intake material, canonical nodes, relations, and lineage
- public compatibility and KAG export seams that stay visibly derived

It does not own:

- AoA federation rules, runtime, SDK control-plane helpers, seed staging, or generic agent workflow machinery
- KAG substrate semantics beyond bounded export seams
- quest or RPG vocabulary as a replacement for node, relation, lineage, context, or synthesis contracts

## Start here

1. `README.md`
2. `ROADMAP.md`
3. `CHARTER.md` and `BOUNDARIES.md`
4. `docs/KNOWLEDGE_MODEL.md` and `docs/NODE_CONTRACT.md`
5. `docs/TINY_ENTRY_ROUTE.md`, `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, and `docs/KAG_EXPORT.md` when public entry or export is in scope
6. the target source, node, registry, intake, or export surface
7. `docs/AGENTS_ROOT_REFERENCE.md` for the preserved full root guidance


## AGENTS stack law

- Start with this root card, then follow the nearest nested `AGENTS.md` for every touched path.
- Root guidance owns repository identity, owner boundaries, route choice, and the shortest honest verification path.
- Nested guidance owns local contracts, local risk, exact files, and local checks.
- Authored source surfaces own meaning. Generated, exported, compact, derived, runtime, and adapter surfaces summarize, transport, or support meaning.
- Self-agency, recurrence, quest, progression, checkpoint, or growth language must stay bounded, reviewable, evidence-linked, and reversible.
- Report what changed, what was verified, what was not verified, and where the next agent should resume.

## Memory route

For recall, continuity, compaction recovery, comparison with past work, or
preserved lessons, start with `aoa-memo` and the workspace memory map. Session
grounding routes through `.aoa`; local candidate writing routes through this
repository's `memo/` port when that port exists; durable reviewed memory lands
through `aoa-memo`.

## Route away when

- ecosystem identity or federation rules belong in `Agents-of-Abyss`
- runtime or service posture belongs in `abyss-stack`
- typed workspace integration belongs in `aoa-sdk`
- derived substrate mechanics belong in `aoa-kag`
- staging, replay, or pre-canon transplant logistics belong in `Dionysus`

## GitHub landing workflow

Root `AGENTS.md` owns the repository-wide branch, PR, CI, and merge route.
`.github/AGENTS.md` owns the GitHub-native files that support it.

When the user asks to commit, push, and merge in this repository, use this route:

1. Start from a branch based on the current `origin/main`. If the worktree is already dirty, inventory it first and carry forward only the intended diff.
2. Commit the intended change with a message that names the changed surface.
3. Push the branch and open a pull request that states changed surfaces, validation run, skipped checks, and remaining risk.
4. Wait for GitHub `Repo Validation` and any required GitHub checks. If a check fails, fix the branch and wait for the new result.
5. Merge through GitHub after green validation. Use squash unless repository settings report a different required method; report the method that landed.
6. Return to `main`, fast-forward from `origin/main`, and confirm the worktree is clean before closeout.

If GitHub status or merge permissions cannot be observed, stop the landing route and report the exact blocker instead of guessing.

## Verify

The current bounded read-only battery is:

```bash
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
python -m unittest discover -s tests
```

If canonical tree mirrors, export inputs, or generated seams change, run the additional targeted validators named in `docs/AGENTS_ROOT_REFERENCE.md`.
Use `docs/REVIEW_CHECKLIST.md` for manual review posture when source, interpretation, or export meaning is touched.

## Report

Name the ToS layer touched: source, intake, tree, compatibility, interpretation, or export. State whether provenance, lineage, or interpretation posture changed, and disclose checks honestly.

## Full reference

`docs/AGENTS_ROOT_REFERENCE.md` preserves the former detailed root guidance, including planning questions, review posture, and export-specific validation branches.
