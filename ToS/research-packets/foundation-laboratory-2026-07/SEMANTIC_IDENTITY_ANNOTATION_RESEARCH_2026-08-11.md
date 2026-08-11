# Semantic identity and annotation foundation research

Дата среза: 2026-08-11. Порядок исследования: официальная и классическая документация → признанные работы → наиболее свежие релевантные работы 2025–2026 годов. Свежесть сама по себе не считается качеством или основанием менять owner law.

## 1. Вопрос и граница

Исследование отвечает на практический вопрос: на каких устойчивых сущностях строить путь от письменного источника к повторениям, знакам, концептам, конкурирующим прочтениям и графу так, чтобы более поздняя интерпретация не переписывала источник и не становилась истиной из-за зелёного валидатора.

Требуемая цепочка:

1. точная форма;
2. частота и concordance;
3. контекст;
4. морфология;
5. лемма;
6. повторение внутри раздела;
7. повторение внутри произведения;
8. повторение внутри корпуса автора;
9. переводные соответствия;
10. кандидат устойчивого знака;
11. ручное подтверждение или отклонение;
12. отношения между знаками;
13. концептуальные интерпретации;
14. конкурирующие прочтения;
15. производная графовая проекция.

В исследовательский пакет не входят частные тексты или фрагменты Ницше. Локальные источники могут быть объектом лабораторной работы, но не получают права публикации из-за наличия метаданных, индекса или производного результата.

## 2. Официальная и классическая документация

### 2.1 W3C Web Annotation Data Model

[Web Annotation Data Model, W3C Recommendation](https://www.w3.org/TR/annotation-model/) разделяет идентичность Annotation, Body, Target и motivation. `SpecificResource` позволяет отдельно описывать источник, selector и состояние представления. Text Quote Selector удобен для повторного нахождения, но копирует исходный текст; Text Position Selector не копирует текст, однако хрупок без зафиксированного состояния. Модель допускает stand-off annotation и не требует, чтобы граф был первичным хранилищем.

Решение для ToS: selector является механизмом возврата к источнику, а не идентификатором семантической сущности. Смещения допустимы внутри selector только вместе с fixity и состоянием представления; naked character offsets не являются долговечным ID.

### 2.2 SKOS

[SKOS Simple Knowledge Organization System Reference, W3C Recommendation](https://www.w3.org/TR/skos-reference/) задаёт Concept как отдельно идентифицированную единицу мысли, а preferred/alternative labels — как свойства, которые могут меняться. Семантические отношения существуют между concept resources; mapping relations связывают схемы, но не заменяют provenance.

Решение для ToS: текущая русская формулировка, gloss или перевод не могут быть ID концепта. Знак и concept также не должны сливаться: знак остаётся источниково и корпусно обоснованной интерпретируемой сущностью, concept — более высокий интерпретационный узел.

### 2.3 OntoLex-Lemon и VarTrans

[OntoLex-Lemon final community report](https://www.w3.org/2016/05/ontolex/) разделяет LexicalEntry, Form, LexicalSense и LexicalConcept. LexicalEntry объединяет формы и смыслы; Form описывает грамматическую реализацию; LexicalSense реифицирует связь lexical entry с референцией и может нести context, register и domain; LexicalConcept совместим со SKOS Concept.

[OntoLex VarTrans](https://www.w3.org/community/ontolex/wiki/Final_Model_Specification) различает перевод через общую reference, relation между senses и более слабое `translatableAs`. Переводное соответствие зависит от sense и context и не доказывает тождество концептов.

Решение для ToS: occurrence, lexeme, lexical sense, sign и concept получают разные ID. Перевод является утверждением или отношением с provenance, а не свойством, автоматически определяющим сущность.

### 2.4 TEI P5 4.12.0

[TEI P5, Simple Analytic Mechanisms](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/SA.html) и [TEI P5, Feature Structures and Analyses](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/AI.html), release 4.12.0 от 2026-07-28, поддерживают stand-off разметку, явные spans/anchors, ответственность и certainty, а также документирование автоматического, ручного или смешанного способа порождения annotation. Такой подход позволяет сохранять read-only source и несколько несовместимых или перекрывающихся структур.

Решение для ToS: исходный текст не переписывается семантической разметкой. Конкурирующие сегментации и интерпретации сосуществуют как отдельные claims.

### 2.5 CIDOC CRM E13 и CRMinf 1.2.1

[CIDOC CRM E13 Attribute Assignment](https://cidoc-crm.org/taxonomy/term/37) отделяет activity назначения от target, assigned value и типа отношения. Это не позволяет выдать присвоенный атрибут за внутреннее свойство источника без следа акта назначения.

[CRMinf 1.2.1](https://cidoc-crm.org/extensions/crminf/html/CRMinf_v1.2.1.html), стабильный release апреля 2026 года, разделяет Argumentation, Belief, Proposition Set, evidence, inference logic и adoption of belief. В версии 1.2.1 особенно полезны связи evidence, adopted interpretation of source, assumed meaning/provenance и Meaning Comprehension.

Решение для ToS: claim является first-class record с автором, временем, методом, target anchors, evidence, certainty, status и review. Интерпретация источника и убеждение в ней не становятся характеристикой байтов источника.

### 2.6 Wikibase и WikibaseLexeme

[Wikibase Data Model](https://www.mediawiki.org/wiki/Wikibase/DataModel) и [Data Model Primer](https://www.mediawiki.org/wiki/Wikibase/DataModel/Primer/en) разделяют entity и fingerprint/labels, а statement — на claim, qualifiers, references и rank. Несовместимые statements могут сосуществовать. Rank полезен для выдачи, но не является вероятностью истины.

[WikibaseLexeme Data Model](https://www.mediawiki.org/wiki/Extension%3AWikibaseLexeme/Data_Model) присваивает отдельные ID Lexeme, Form и Sense. Lemma, representation и gloss не являются ID; совпадающие lemma/language/category не гарантируют одну lexeme. Связь Sense с Item также не означает автоматической синонимии или эквивалентности.

Решение для ToS: ID не зависит от label, а competing claims сохраняются. При этом wikibase-подобная форма хранения не заменяет источниковую эпистемологию и компетентное review.

### 2.7 OntoLex FrAC — emerging, не стабильный final report

[OntoLex Frequency, Attestation and Corpus Information](https://ontolex.github.io/frequency-attestation-corpus-information/) различает Observable, Observation и Attestation; frequency и attestation могут относиться отдельно к Form, LexicalEntry, LexicalSense и LexicalConcept. Attestation связывает точное или нормализованное occurrence с corpus locus и делегирует аннотирование корпуса Web Annotation.

Статус нельзя завышать: [public review был объявлен в январе 2025 года](https://lists.w3.org/Archives/Public/public-ontolex/2025Jan/0005.html), тогда как [страница OntoLex community group](https://www.w3.org/community/ontolex/) по состоянию среза всё ещё перечисляет final reports 2016 и 2019 годов. Поэтому FrAC используется как сильный emerging design input, а не как завершённый стандарт.

## 3. Признанные и исторически важные работы

- Thomas Gruber, *A Translation Approach to Portable Ontology Specifications* (1993), [DOI 10.1006/knac.1993.1008](https://doi.org/10.1006/knac.1993.1008): формальная спецификация общего vocabulary должна делать commitments явными и переносимыми. Для ToS это аргумент в пользу явных типов и минимальных обязательств, а не универсальной ранней онтологии философии.
- Nicola Guarino, Daniel Oberle, Steffen Staab, *What Is an Ontology?* (2009), [DOI 10.1007/978-3-540-92673-3](https://doi.org/10.1007/978-3-540-92673-3): различение conceptualization, ontology и knowledge base предупреждает слияние схемы, утверждений и данных.
- Thomas Baker et al., *Key Choices in the Design of Simple Knowledge Organization System (SKOS)* (2013), [arXiv:1302.1224](https://arxiv.org/abs/1302.1224): minimal ontological commitment и различение integrity conditions от inference поддерживают осторожный foundation layer.
- Robert Sanderson, Paolo Ciccarese, Herbert Van de Sompel, *Designing the W3C Open Annotation Data Model* (2013), [DOI 10.1145/2464464.2464474](https://doi.org/10.1145/2464464.2464474), [arXiv:1304.6709](https://arxiv.org/abs/1304.6709): переносимая annotation требует явных target/body/provenance boundaries.
- Paul Groth, Andrew Gibson, Jan Velterop, *The anatomy of a nanopublication* (2010), [DOI 10.3233/ISU-2010-0613](https://doi.org/10.3233/ISU-2010-0613): assertion, provenance и publication context должны быть разделимы.
- Tobias Kuhn, Michel Dumontier, *Trusty URIs* (2015), [DOI 10.1109/TKDE.2015.2419657](https://doi.org/10.1109/TKDE.2015.2419657): content-addressed references полезны для immutable artifacts и verification chains. Для ToS digest связывает версию данных, но не должен превращать текущий label или перевод в identity seed семантической сущности.

Общий вывод признанных работ: формальная переносимость, immutable evidence binding и graph representation полезны только после того, как сущности и акты утверждения разведены.

## 4. Наиболее свежие релевантные работы 2025–2026

### 4.1 LLM + human validation

Tsaneva et al., *Knowledge graph validation by integrating LLMs and human-in-the-loop* (Information Processing & Management, 2025), [DOI 10.1016/j.ipm.2025.104145](https://doi.org/10.1016/j.ipm.2025.104145), [institutional record](https://oro.open.ac.uk/103792/), сравнивает девять workflows. Standalone LLM validation остаётся слабой, тогда как сочетание автоматических методов и selective human verification даёт лучший баланс качества и ручной стоимости.

Решение для ToS: человек не перепечатывает и не подтверждает каждый occurrence. Human checkpoint открывается редко: при promotion устойчивого знака, высоком downstream impact, declared competence gap или устойчивой неоднозначности.

### 4.2 Историческая семантика и языковая компетенция

Hagen, *Lexical Semantic Change Annotation with LLMs* (LaTeCH-CLfL 2025), [ACL Anthology](https://aclanthology.org/2025.latechclfl-1.16.pdf), работает с историческим немецким DURel. Результаты показывают, что сильные модели могут быть полезны как annotator candidates, но retrieval context не гарантирует улучшения и иногда ухудшает более крупные модели.

Zhao, Siro, Hollink, *Can LLMs Recognize Contentious Terms?* (LREC 2026), [institutional record](https://ir.cwi.nl/pub/36414/), [DOI 10.63317/3dhy55mxo9zb](https://doi.org/10.63317/3dhy55mxo9zb), показывает near-human performance для явных случаев исторического нидерландского, но расхождения в contextual/historical reasoning, semantic shift и figurative use.

Решение для ToS: модель может предлагать morphology, sense, sign или change candidate, но отсутствие компетенции человека в немецком или другом иностранном языке должно быть явно записано. Визуальное «похоже на правильное» не является лингвистическим review.

### 4.3 Human-centred historical research

Assael et al., *Aeneas transforms historical contextualization* (Nature, 2025), [DOI 10.1038/s41586-025-09292-5](https://doi.org/10.1038/s41586-025-09292-5), [article](https://www.nature.com/articles/s41586-025-09292-5), демонстрирует staged human-centred evaluation: историки с AI сильнее по restoration/location, а retrieved parallels служат отправной точкой для критической интерпретации, не заменой ей.

Решение для ToS: система должна доставлять человеку источник, anchors, кандидаты и различия в одном review packet; итоговая authority остаётся у источника и компетентного review.

### 4.4 Свежие пределы synthetic annotators

Kasner et al., *Can Large Language Models Replace Human Annotators?* (MME 2026), [ACL Anthology](https://aclanthology.org/2026.mme-main.1/), находит умеренное agreement и снижение стоимости, но не подтверждает прямую взаимозаменяемость с человеком. Нужна калибровка на экспертной выборке.

Kulmizev et al., *Large Language Model Ensembles Can Mimic Annotator Distributions But Not Their Diversity* (ACL 2026), [DOI 10.18653/v1/2026.acl-long.752](https://doi.org/10.18653/v1/2026.acl-long.752), [ACL Anthology](https://aclanthology.org/2026.acl-long.752/), показывает, что ensemble способен приблизить распределение labels, но сохраняет idiosyncratic disagreements и меньшую diversity объяснений.

Kim et al., *Improving LLMs as Data Annotators* (ACL 2026), [DOI 10.18653/v1/2026.acl-long.1760](https://doi.org/10.18653/v1/2026.acl-long.1760), [ACL Anthology](https://aclanthology.org/2026.acl-long.1760/), показывает пользу guideline integration/refinement и reasoning models, но сохраняющийся существенный разрыв.

Решение для ToS: maker record обязан хранить model, revision, prompt digest и provenance event. Несогласие моделей не симулирует человеческую перспективу и не создаёт human review.

### 4.5 Прозрачные признаки и диахрония

*Transparent Semantic Change Detection Using Dependency-Based Co-occurrence Profiles* (LChange 2026), [DOI 10.18653/v1/2026.lchange-1.8](https://doi.org/10.18653/v1/2026.lchange-1.8), [ACL Anthology](https://aclanthology.org/2026.lchange-1.8/), показывает ценность интерпретируемых dependency co-occurrence profiles, способных конкурировать с opaque distributional representations.

*Pretraining Language Models for Diachronic Linguistic Change* (Findings of EACL 2026), [DOI 10.18653/v1/2026.findings-eacl.241](https://doi.org/10.18653/v1/2026.findings-eacl.241), [ACL Anthology](https://aclanthology.org/2026.findings-eacl.241/), показывает преимущества bounded historical corpora и train-from-scratch setup для сохранения temporal divisions и снижения anachronistic contamination.

Решение для ToS: frequency, concordance, context и recurrence являются прозрачным evidence layer. Они поддерживают, но не доказывают устойчивую семантическую идентичность.

### 4.6 Несколько корпусов и траектории концептов

*HistLens: Multi-Concept, Multi-Corpus Historical Semantic Analysis* (ACL 2026), [DOI 10.18653/v1/2026.acl-long.652](https://doi.org/10.18653/v1/2026.acl-long.652), [ACL Anthology](https://aclanthology.org/2026.acl-long.652/), подчёркивает ограничения single-concept/single-corpus analysis и исследует trajectories по времени и источникам, включая implicit concepts.

Решение для ToS: multi-corpus trajectory — полезный challenger для поздних этапов, но не owner truth и не причина заранее слить occurrences в один concept.

### 4.7 Freshness watchlist, не фундамент

[Temporal Document Analysis benchmark, arXiv:2608.08512](https://arxiv.org/abs/2608.08512), опубликованный 2026-08-09, слишком свеж и лишь косвенно относится к foundation identity. Он фиксируется в watchlist как preprint, но не влияет на текущий contract.

## 5. Архитектурные решения и отвергнутые сокращения

Принятые решения:

- occurrence → lexeme → lexical sense → sign → concept — разные сущности;
- label, lemma, gloss, перевод и текущее имя концепта изменяемы и не входят в opaque semantic ID;
- annotation является stand-off packet;
- claim хранит typed proposition, target anchors, evidence role/direction, maker, time, method, certainty, status, competing claims и review refs;
- model maker обязан замкнуть model ref, revision, prompt digest и provenance event; synthetic fixture обязана честно объявить, что ничего такого не запускалось;
- review является отдельным реальным человеческим актом с competence scope;
- promotion знака требует source-visible unassisted baseline, зафиксированный до model suggestions;
- graph принимает только reviewed/accepted claims и relations и всегда возвращается к source anchors;
- validator проверяет форму, closure и fail-closed boundaries, но не истину, компетенцию, семантику или права.

Отвергнутые сокращения:

- ID из русского перевода, lemma или названия концепта;
- автоматическое превращение частоты/повторения в один знак;
- автоматическое превращение embedding cluster в concept;
- один универсальный `body` для occurrence, morphology, sense, sign и concept;
- accepted model claim без отдельного human review;
- graph edge без claim, evidence и source return;
- routine human task для каждого occurrence;
- зелёная schema validation как доказательство реального review или semantic truth.

## 6. Contract и A/B/C laboratory design

Additive contract: `ToS/contracts/semantic-annotation-packet-v2.schema.json`. Он не меняет frozen initial sign packet и semantic ladder v4.

Публичная synthetic laboratory:

- A — только два exact occurrences и source-bound observation claims;
- B — те же occurrences плюс два разных opaque sign IDs и два взаимно конкурирующих synthetic model-shaped proposals; оба остаются `proposed`, reviews и graph effect отсутствуют;
- C — deliberate negative control: model-shaped proposal пытается стать `accepted` без real-human promotion review и обязан быть отвергнут.

Дополнительные in-memory controls проверяют label-derived ID, duplicate IDs, missing anchors, unresolved refs, one-way competition, review/competence/baseline bypass, relation/claim mismatch, premature graph projection, self-supersession и publication widening. Это проверка защитных механик, не «подготовленный зелёный результат»: после неё нужны ручные чтение fixtures, проверка exact spans и подтверждение ожидаемого A/B/C поведения.

## 7. Ограничения и следующие вопросы

- Contract не доказывает, что два occurrence образуют один lexeme, sign или concept.
- Synthetic lab не является model benchmark и не создаёт model provenance.
- В tracked positive fixtures намеренно нет human review; C доказывает fail-closed absence, а не качество будущего review.
- OntoLex FrAC остаётся emerging/public-review input и должен быть проверен при появлении stable final report.
- Свежие ACL/Nature результаты информируют метод, но не отменяют source-first owner law.
- Следующий содержательный эксперимент после механического A/B/C должен начинаться только с нового источниково обоснованного вопроса, а не с повторного прогона уже решённой задачи.
