# Spark Swarm Recipe — Tree-of-Sophia

Рекомендуемый путь назначения: `Spark/SWARM.md`

## Для чего этот рой
Используй Spark здесь только на doc-drift, source-traceability wording, node/lineage wording cleanup и micro-clarifications. Это canonical high-level statement ToS; рой не должен flatten interpretation into generic summaries и не должен подменять human-reviewed synthesis автоматическим gloss.

## Читать перед стартом
- `README.md`
- `touched docs only after README`

## Форма роя
- **Coordinator**: выбирает один high-level ToS seam
- **Drift Auditor**: ищет drift в source traceability, node layering, lineage wording и repo routing
- **Micro-Patcher**: делает минимальный patch
- **Verifier**: делает manual consistency review и link/path check
- **Boundary Keeper**: охраняет source traceability и distinction between raw material, semantic extraction and human-reviewed synthesis

## Параллельные дорожки
- Lane A: wording drift or link cleanup
- Lane B: source traceability / node layering / lineage wording
- Lane C: manual consistency review
- Не запускай больше одного пишущего агента на одну и ту же семью файлов.

## Allowed
- чинить micro-drift в high-level wording
- прояснять source traceability и lineage relations
- чинить links and route hints
- делать minimal documentation polish that improves legibility

## Forbidden
- переписывать философскую доктрину широкими мазками
- сплющивать layered meaning into generic summaries
- подменять human-reviewed synthesis автоматической интерпретацией
- тащить derived KAG claims как source truth

## Launch packet для координатора
```text
We are working in Tree-of-Sophia with a one-repo one-swarm setup.
Pick exactly one high-level seam:
- source traceability wording
- node layering wording
- lineage wording
- route hint / link cleanup
- one minimal conceptual clarification

Return:
1. the seam
2. exact files to touch
3. what must remain human-reviewed
4. why this is a micro-patch and not doctrine rewrite
```

## Промпт для Scout
```text
Audit only. Do not edit.
Return:
- exact wording or routing inconsistency
- files involved
- risk of flattening meaning
- risk of source/derived confusion
- whether this belongs in ToS or downstream in aoa-kag / AoA
```

## Промпт для Builder
```text
Make the smallest patch possible.
Rules:
- preserve source traceability
- preserve node layering
- preserve lineage relations
- avoid flattening interpretation
- keep raw material, semantic extraction, and human-reviewed synthesis distinct
```

## Промпт для Verifier
```text
Do a manual consistency review.
Minimum:
- re-read README sections touched by the patch
- check all links or route hints you edited
- confirm that source traceability, node layering, and lineage wording remained intact
Report only checks you actually performed.
```

## Промпт для Boundary Keeper
```text
Review only for anti-scope.
Check:
- this stayed a micro-patch
- no doctrine-scale rewrite slipped in
- no flattening into generic summaries
- no downstream derived claim became source truth
```

## Verify
```bash
# Manual review only
# Re-read touched README/doc sections
# Re-check edited links / route hints
# Confirm source traceability, node layering, and lineage wording remained intact
```

## Done when
- один high-level seam стал яснее без доктринального расползания
- manual consistency review реально проведён и описан
- source traceability and layered meaning preserved
- human-reviewed synthesis remains distinct from automation

## Handoff
Если патч касается derived substrate or operational federation, follow-up почти всегда в `aoa-kag` или `Agents-of-Abyss`, а не тут.
