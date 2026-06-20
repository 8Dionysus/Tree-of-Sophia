# Spark Swarm Recipe - Tree-of-Sophia

Рекомендуемый путь назначения: `.agents/spark/SWARM.md`

## Для чего этот рой

Spark-рой здесь нужен для коротких правок вокруг source traceability,
node/lineage wording, route hints и micro-clarifications. Он помогает быстро
улучшить читаемость ToS-поверхности и передать дальше все, что требует
доктринального, источниковедческого или архитектурного просмотра.

## Читать перед стартом

- `README.md`
- `.agents/AGENTS.md`
- `.agents/spark/AGENTS.md`
- ближайший owner surface для изменяемых файлов

## Форма роя

- **Coordinator**: выбирает один high-level ToS seam
- **Drift Auditor**: ищет drift в source traceability, node layering, lineage wording и repo routing
- **Micro-Patcher**: делает минимальный patch
- **Verifier**: делает manual consistency review и link/path check
- **Boundary Keeper**: следит, чтобы raw material, semantic extraction и human-reviewed synthesis оставались разными слоями

## Параллельные дорожки

- Lane A: wording drift or link cleanup
- Lane B: source traceability / node layering / lineage wording
- Lane C: manual consistency review

Один пишущий агент работает с одной семьей файлов; остальные дорожки остаются
аудиторскими или reviewer-дорожками.

## Подходящие задачи

- micro-drift in high-level wording
- source traceability and lineage relation clarification
- link and route-hint repair
- minimal documentation polish that improves legibility

## Escalation Routes

- философская доктрина широкого масштаба -> ToS owner surfaces and human review
- layered meaning synthesis across many sources -> slower ToS review path
- derived KAG claims -> downstream KAG owner surfaces
- operational federation or runtime substrate -> Agents-of-Abyss / abyss-stack owners

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
Audit only.
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
Review the route boundary.
Check:
- this stayed a micro-patch
- doctrine-scale rewrite was routed away
- layered meaning was preserved
- downstream derived claims stayed downstream
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
- manual consistency review реально проведен и описан
- source traceability and layered meaning preserved
- human-reviewed synthesis remains distinct from automation

## Handoff

Если патч касается derived substrate or operational federation, follow-up почти
всегда в `aoa-kag`, `abyss-stack` или `Agents-of-Abyss`, а не в ToS Spark.
