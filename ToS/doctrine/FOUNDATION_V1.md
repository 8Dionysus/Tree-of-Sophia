# Фундамент ToS v1: замысел, законченный элемент и покрытие

Это карта выполнения операторской цели, а не второй реестр сущностей, новая
онтология или свидетельство готовности. Определения принадлежат доктрине и
контрактам ToS; операции — своим владельцам; проекции — `access/`.
Строка закрывается проверяемой работающей возможностью, не наличием названия
типа. Все обязательные строки входят в v1; сквозные примеры не сокращают объём.

## Источник и уточнения

[Исходный диалог «Философские сквозные сущности»](https://chatgpt.com/share/6a9cbd81-c004-83e8-a5e8-6cc7be939dd3)
содержит пять содержательных обменов. Реплики развивают общий замысел; вклад
собеседника не отбрасывается как противоречащий направлению оператора.
Номера ниже — номера сообщений сохранённого полного транскрипта, включая
пустые служебные записи; смысловые сообщения перечислены по точному ID:

| Ссылка | Message ID | Содержание фрагмента |
| --- | --- | --- |
| M05 | bbb21706-0142-41d0-95c0-c9c82dbf19ef | сквозные устойчивые вещи философии |
| M12 | 9f59b64b-af61-49dd-b03b-22c5bad621e0 | четырнадцать видов содержания и ход мысли |
| M13 | bbb215a0-641f-4440-971c-020deea220f1 | идентичность свободы при различии пониманий |
| M15 | 150fa5e9-9baa-4954-bb2e-2b5820a61e2d | Concept, Conception, Occurrence, Transformation; аспекты и преобразования |
| M16 | 6d37cf46-a399-40c1-96a3-5f1115ab464c | время, пространство, произведение, автор, содержание |
| M19 | 42b5c95f-0145-4357-908e-93c2c02ae064 | метаобъекты, отношения, свидетельства и универсальные структуры |
| M20 | 1381e7f4-6e31-4a9d-a038-9bf89acd5f87 | биография и среда как содержание философии |
| M22 | 7f60e2d6-a4de-4056-835a-5471b7a16e45 | жизнь, исторический мир, интеллектуальная среда, производство, рецепция |
| M23 | e7dd35ea-9dd6-4a96-b9c3-10dc436a7cfa | углубление каждой ветви |
| M25 | d70a2ea0-0c87-4f71-bb8e-4b9c32a7c42c | возврат ветвей друг в друга, письмо и изменение масштаба |

Четыре аналитических плана M19 не означают четыре базы. Библиографическая
цепочка — выразимый случай, не обязательная форма любого документа. Conception
не является технической версией записи. «Событие становления мысли» — возможная
линза, не единственный центр. Фрактальность означает открытое углубление, не
требование математического самоподобия. Исторические примеры диалога не
становятся принятыми фактами без своих источников и оценки.

Уточнения переписки и операторского goal обозначены G2–G11 по разделам цели:
устойчивое ядро, девять профилей, агентная оценка, человеческий язык, конструктор,
масштабирование, миграция, UI, организация и конечная проверка. Они дополняют
диалог. Скорость роста знания не ограничивается ручной обработкой человеком.

## Контракт законченного элемента

Для типа, роли, свойства, отношения или операции одновременно нужны:

1. Семантический ID, версия определения, языковые определения и границы: что
   означает элемент и с чем его нельзя смешивать.
2. Машинная структура: допустимые значения/участники, ограничения, наследование,
   контекст, основания, оценка и совместимость неизвестного расширения.
3. Правила идентичности и изменения: исправление, история, преемство, отмена,
   конфликты и отображение прежних ссылок без тихого переиспользования смысла.
4. Обнаруживаемые операции исследования и разрешённого роста по смысловым ID,
   не угадываемым путям внутри JSON.
5. Человеческие формы: имя, компактная подпись, наведение, точное высказывание,
   основания и история. Язык и происхождение каждой формы явны; сокращение не
   усиливает утверждение. Пробел не считается заполненным общей заглушкой.
6. Долговечные положительные и отрицательные проверки, реальные связанные
   примеры, отображение текущих данных и измеримая стоимость изменений.

Общие правила переиспользуются. Роль автора не создаёт копию человека; новое
произведение добавляет данные; новый профиль использует контракт расширения.

## Владельческие исполнения

Сокращения в матрице обозначают текущие исходные поверхности или требуемое
продолжение в них, а не обещание, что профиль уже полностью исполняется:

- **Registry** — `ToS/doctrine/semantic-interchange/`,
  `ToS/contracts/semantic-entity-type-registry.schema.json` и
  `semantic-relation-type-registry.schema.json`.
- **Corpus** — `ToS/doctrine/CORPUS_FOUNDATION.md`, `ToS/source-witnesses/` и
  соответствующие библиографические, текстовые и claim-контракты `ToS/contracts/`.
- **Assessment** — `ToS/doctrine/KNOWLEDGE_ASSESSMENT.md`, четыре assessment
  schemas, политика и `mechanics/growth-cycle/parts/branch-growth-cycle/`.
- **Research** — `access/contracts/knowledge-api.v1.json`, LensSpec, graph
  schemas и `access/src/tos_access/knowledge.py`.
- **Growth** — явный командный контракт у владельца исходного знания,
  `mechanics/growth-cycle/`; не запись через read-only `access`.
- **Forms** — исходные языковые поля и их оценки у ToS; общий контракт формы,
  контекста и выбора языка с совместимым читателем у `access`.
  [Source-owned навигация Claim](../review-ledger/2026-09-08-claim-navigation-review.md)
  теперь использует точные имена, предикат и статусы записи; это не готовая
  человеческая формулировка. Наблюдаемые legacy-пробелы форм сохраняются явно;
  их текущая полнота и следующий владелец проверяются в K4.
  [Проверка assessed consumer](../review-ledger/2026-09-09-assessed-form-consumer-parity-review.md)
  дополнительно связывает обычные source-copy формы с журналом оценки и
  Python/Worker reader: полный контекст добавляется при чтении без переписывания
  формы, отдельная оценка Claim не теряет ограничения и не становится endorsement.
  Это проверенное продолжение A02/V01/V02; final-union V03/M01 проверяются отдельно.
- **Processing** — `access/src/tos_access/normalization_cache.py`, обработка
  исходных слоёв у Corpus, текущая материализация и чтение Cloudflare/D1.
- **Consumer** — API/MCP/agent consumer и `access/web/src/observatory/`, с
  сохранением отдельного владельца UI.

Состояние **локальное исполнение** означает реализованный source/operation/form
путь с указанными положительными и отрицательными проверками. Оно не означает
оценку каждой записи, готовность всего UI, прошедший CI или закрытие v1.
**Ограниченный допуск** относится только к точному предмету, purpose, policy и
актуальным основаниям; неподтверждённое содержание сохраняет собственный статус.

### Текущая граница проверки · 2026-09-10 UTC

Матрица первоначально сведена на source baseline
`7594a9e36b763c1975632114772df959071b3be6` после native Collection/Artifact/Link,
scoped structures, text derivations, public Occurrence, bilingual alignment и
retained-page OCR. Этот commit включает исправление общей alignment dispatch
после объединения OCR-пути. Отдельные review notes остаются свидетельствами
своих точных срезов; их старые ограничения не выдаются за текущие дефекты.

На union `aa586ebc47761625052c49377d65539aed58bd27` уточнены F04/C01 и M01:
добавлен отдельный v2 semantic-subject proposal route; source-enumerated K4
имеет [ограниченный подтверждённый результат](../review-ledger/2026-09-10-foundation-source-mapping-remainder-review.md),
а [повтор на объединённом читателе](../review-ledger/2026-09-10-foundation-union-map-review.md)
подтвердил прежние 678/678 catalog identities. Полная currentness остаётся K1.
Эта сверка не закрывает K2/K3 и не превращает unknown mapping в принятую семантику.

На следующем union `5e3fb93d6cb397d36ec5e66ef6128660356b2f9f`
[реальный historical/Form reader return](../review-ledger/2026-09-10-real-historical-forms-reader-review.md)
добавил проверенное исполнение A02/A06/V01/V02: отдельные оценки 11 Claims и
44 Forms, native append/replay без новых per-record human approvals, точную
Python/Worker доставку всех четырёх ролей и фактический локальный HTTP.
Shared v2 сохраняет полные source packets, а не сокращает их смысл. Это не
публичная активация assessed reader и не совместная UI-приёмка. Размеры даже
компактных ответов и измеренные секунды запроса оставляют K3 открытым.
Временный HTTP reader завершён; его наблюдение не является текущим runtime grant.
Повторная source-enumerated проверка обычного читателя на этом union охватила
684/684 текущие catalog identities (654 direct, 30 adapted) и 408 source files.
Она не расширяет прежнюю область private/remainder audit и не закрывает K1.

**Закрытие** v1 контролируют четыре сквозных gate, а не новый бесконечный список
типов или требование оценить всю философию:

- **K1 — итоговая целостность:** source/catalog/graph/resource index/KAG и
  documentation/agent companions одного union; affected/full проверки,
  необходимые integration checks и CI. Regeneration не принимает содержание.
- **K2 — настоящее потребление:** оба обязательных реальных маршрута человеком
  и агентом, хотя бы один вне Заратустры; concept/word/Occurrence/person/work/
  event/place/Claim/relation focus, раскрытие, фильтры, сравнение, источник,
  объяснение включения и продолжение. Проверяются отрицательные/ограниченные
  состояния, сохранение выбора/сцены и плавность существующего UI.
- **K3 — адресность и стоимость:** cold/warm запросы, add/edit/delete/review/access,
  сбой/возобновление, смена/откат derived snapshot, рост данных, ограниченные
  ресурсы и проверенная local/Cloudflare-D1 согласованность без production deploy.
- **K4 — отображение источников и остаток:** source-enumerated public universe,
  полные старые поля и native adapters, явно ограниченная private metadata
  область, unknown/restricted/needs-clarification с владельцем и условием
  продолжения. Ограниченный source-owner audit завершён на своём exact snapshot;
  его публичные authored inventories не изменились до указанного union.
  Catalog mapping повторён на объединённом читателе; полная currentness и
  sealing новых companions остаются в K1;
  частный инвентарь не объявляется глобальным или повторно проверенным.
  Число узлов не определяет знаменатель и не доказывает качество.

Данные могут честно оставаться unreviewed, disputed, unknown или restricted,
когда исполняемый маршрут их оценки/уточнения и текущий владелец известны.
Отсутствующая компетенция для древнего языка либо historical OCR не выдаётся за
универсальную квалификацию, но и не требует фиктивной оценки каждой записи.
Произвольный глобальный as-of, специальный biography экран, новый sort/group
или дополнительные исторические примеры не становятся самостоятельными
обязательствами лишь из-за прежней обзорной формулировки карты. Это не отменяет
точные версии, время знания, общие запросы, биографическое содержание или
обязательные реальные маршруты цели.

## Карта различий и обязательных возможностей

Каждая строка связывает фрагмент, правило, владельческий контракт, операцию,
человеческую форму и условие проверки. Названия операций ниже обозначают
семантические возможности; их окончательные wire ID принадлежат реестру.

| ID / источник | Различие и правило | Контракт → операция | Человеческая форма | Проверка завершения / состояние |
| --- | --- | --- | --- | --- |
| F01 · M19, G2 | предмет, имя, внешний ID, запись, версия, утверждение, проекция различны | Registry/Corpus → resolve, inspect, revise | узнаваемый предмет и история описаний | **локальное исполнение**: [exact native reader/revisions](../review-ledger/2026-09-09-native-metadata-exact-reader-review.md), [восемь Corpus kinds](../review-ledger/2026-09-09-native-corpus-descriptive-revision-review.md) и [portable native history](../review-ledger/2026-09-09-legacy-object-link-context-review.md) сохраняют действительные ID, версии, предшественников и формы. Claim/object не сливаются; missing exact version не заменяется latest, исторический record не выдаёт нынешний допуск. K1/K2. |
| F02 · M19, G2 | часть/целое, членство, корпус, коллекция, порядок и последовательность не взаимозаменяемы | Registry → compose, members, ordered parts | объяснимое включение и порядок | **реализовано локально**: [общий scoped-members контракт](semantic-interchange/README.md#scoped-composition-and-research-corpora), интеллектуальные части, исследовательский корпус, точный состав и частичный/полный порядок исполняются через общие create/revise/reader; прежний Collection→Work сохраняет отдельный compound route. [Реальный исследовательский состав](../review-ledger/2026-09-10-scoped-member-structure-review.md) связывает пять сохранённых объектов с отдельным Claim, формами, повтором и фокусом на каждом участнике. [Физический состав](../review-ledger/2026-09-10-physical-member-structure-review.md) имеет отдельный Artifact-only профиль и реальный частичный reported Claim для OIM A00645 без выведенной склейки или полноты. [Порядок коллекции](../review-ledger/2026-09-10-collection-member-order-review.md) связывает семь Works сборника Мысль 1996 с точными версиями Collection и прежних membership Claims; создание, повтор, форма и фокус всех восьми центров проверены в общем читателе. Порядок не создаёт членство. Локальные циклы и неполный total order отклоняются, конкурирующие Claims не сливаются; проверка структуры не принимает историческое содержание. Общая интеграция и CI проверяются отдельно |
| F03 · M19, G2 | тип, роль, классификация и свойство | Registry/Growth → extend, classify, assign role, filter | определение и подходящий фильтр | **локальное исполнение**: Registry/source-profile reader проверяют конкретные domain/range, наследование, cardinality scope, свойства, единицы и операции; [независимые классификации](../review-ledger/2026-09-07-classification-profile-review.md) и роли аргумента не создают комбинаторные типы. Реальные формы и фильтры имеют source return. K1/K2. |
| F04 · M15/M19, G2 | тождество, сходство, отношение и преемство | Registry/Growth → compare, propose merge/split, resolve old ID | основания объединения и прежние чтения | **локальное исполнение versioned proposals**: [v1 source identities](../review-ledger/2026-09-09-identity-transition-proposals-review.md) и отдельный [v2 semantic-subject route](../review-ledger/2026-09-10-semantic-identity-proposals-review.md) сохраняют exact predecessor/successor sets, basis, migration plan и owner-derived typed descriptors. 33 конкретных профиля, включая Concept/Conception/Lexeme, явно подключены к v2; будущий профиль без adapter остаётся допустимым, но не получает это полномочие. Полные exact sets, отдельные grants, replay/revocation, история и RU/EN carriage проверяются; v1 не наследует v2 scope. Предложение не выполняет destructive merge, alias, перенос Occurrence/Sign bindings или допуска. Старые предметы и ссылки остаются неизменными. Совместное потребление — K1/K2. |
| F05 · M19, G2 | утверждение отдельно от предмета; неизвестно ≠ нет ≠ ложно | Corpus/Assessment → assert, dispute, compare | точная полярность, неопределённость, атрибуция | **локальное исполнение**: собственные Claim ID/version, polarity, epistemic/review state, basis и counterevidence различны. [Датировки](../review-ledger/2026-09-07-temporal-values-and-reader-review.md), [translator attribution](../review-ledger/2026-09-09-native-translator-responsibility-review.md) и [конкурирующие JGB чтения](../review-ledger/2026-09-07-jgb-freedom-reader-review.md) сохраняют отдельные утверждения. Unknown не false; синтетический конфликт не исторический факт. K1/K2. |
| F06 · M19/M25, G2 | мировое время, время свидетельства, получения и пересмотра | Corpus/Registry → temporal overlap, exact version, order | интервалы, приблизительность, календарь и время знания | **локальная ограниченная временная модель**: historical_dating отделён от времени свидетельства, поступления и revision/provenance; календарь, интервалы, приблизительность и относительный ориентир сохраняются. [Сравнение exact Claims](../review-ledger/2026-09-09-command-discovery-and-temporal-comparison-review.md) проверено в Python/Worker/D1 на Basel-данных без выдуманного календаря. Exact-version inspection не произвольная реконструкция всего мира as-of. K2. |
| F07 · M19, G2/G7 | байты, OCR, коррекция, нормализация, сегментация, выравнивание, интерпретация и проекция | Corpus/Processing → derive, inspect lineage, invalidate | исходник и отдельные преобразования | **локальное исполнение отдельных слоёв**: [extraction/segmentation](../review-ledger/2026-09-09-native-text-layer-quality-review.md), [correction/normalization/supplied capture](../review-ledger/2026-09-09-native-text-layer-derivation-review.md), [отдельное сравнение и реальный NFC citation admission](../review-ledger/2026-09-09-native-derived-layer-assessment-review.md), [реальный DE/RU alignment](../review-ledger/2026-09-10-real-native-bilingual-alignment-return.md) и [retained-page owner OCR](../review-ledger/2026-09-10-retained-page-ocr-image-comparison-review.md) сохраняют source bytes/версии. Supplied mapping не исполненный aligner; historical OCR unreviewed и без model disclosure. K1/K3. |
| F08 · M25, G2 | открытое расширение и отсутствие окончательных листьев | Registry/Growth → register compatible profile, deepen | понятный предел понимания старого читателя | **локальное исполнение**: metadata/Claim profiles расширяются данными через общие operations/readers/forms; [exact registry transition](../review-ledger/2026-09-08-record-revision-and-registry-transition.md) различает evolution, несовместимую смену и первое введение. Unknown fields сохраняются, непонятый смысл явно unsupported. Механика не принимает философскую эквивалентность изменённого определения. K1/K4. |
| P01 · M19/M22 | человек, коллектив, сообщество, учреждение, школа, традиция, движение | Registry → people/social profile, membership | разные виды объединений и область членства | **локальный профиль и реальный корпус**: [SocialGroup/Community/InstitutionalBody](../review-ledger/2026-09-07-social-profiles-and-reader-review.md) и [IntellectualSchool/Tradition/Movement](../review-ledger/2026-09-07-intellectual-formations-and-reader-review.md) имеют содержание, общий рост, формы и оба reader-carrier. Стоя, неостоицизм и логический эмпиризм не слиты с учреждением. Review конкретного Claim остаётся отдельным. K1/K2. |
| P02 · M19/M22/M25 | автор, переводчик, редактор, переписчик, издатель, исследователь, адресат — роли | Registry/Growth → participation, responsibility | кто и в каком отношении участвует | **локальное исполнение**: responsibility/participation — квалифицированные отношения к одному Agent, не копии человека. Письмо №705 и [отдельная translator responsibility](../review-ledger/2026-09-09-native-translator-responsibility-review.md) сохраняют exact endpoints, роль, basis и формы. Наличие роли не удостоверяет историческую истинность. K2. |
| P03 · M22/M25 | обучение, учитель/ученик, сотрудничество, переписка, дружба, конфликт | Registry/Assessment → social/intellectual paths | конкретная связь, время и основания | **локальный профиль и реальный корпус**: [восемь социальных](../review-ledger/2026-09-07-social-profiles-and-reader-review.md) и [четыре интеллектуальных](../review-ledger/2026-09-07-intellectual-formations-and-reader-review.md) предиката имеют конкретные endpoints, context/time qualification и обратное чтение. Близость не становится влиянием; неполная датировка остаётся явной. K1/K2. |
| H01 · M19/M22 | историческое событие, процесс, состояние, участие | Registry → event/participation profile | что произошло, кто участвовал и откуда известно | **локальное исполнение**: historical.create/revise, participation, отдельные Claims/forms проходят source/catalog/graph/reader; [реальный эпизод JGB](../review-ledger/2026-09-07-historical-source-creation.md) сохраняет v1/v2 историю. Участие, место, дата и свидетельство различны. Полный совместный исторический маршрут — K2, не вывод из числа записей. |
| H02 · M20/M22/M25 | биография как события и фазы, не одно поле | Registry/Corpus → biography lens | жизнь, обучение, профессии, кризисы, поездки, значимые телесные обстоятельства | **локальный профиль**: [BiographicalEpisode/Phase/LifeCircumstance](semantic-interchange/README.md#biography-periodization-generations-and-historical-environment) имеют содержание, continuity criterion, квалифицированные связи, общий рост и формы. Реальный Basel-контекст не используется для ретроспективного диагноза. Биография исследуется общими линзами и глубиной, без обязательного отдельного экрана. K2. |
| H03 · M19/M25 | место, география, принадлежность и её изменение, миграция | Registry → spatial roles, historical geography | место в соответствующий период | **локальное исполнение**: Place, historical_place, temporal/context Claims сохраняют отдельные основания и роли. [Два Basel environment](../review-ledger/2026-09-07-basel-print-environment-and-reader-review.md) связаны с одним Place без выведенной одновременности, тождества эпох или причинности. Дополнительная география добавляется данными. K2. |
| H04 · M22/M25 | эпоха, поколение, современность, фаза, скорость передачи и запаздывание рецепции | Registry/Research → time scales, sequence comparison | разные временные масштабы | **локальный профиль и реальный корпус**: периодизация и поколение различны с датой и организацией; [две классификации поколения 1898](../review-ledger/2026-09-07-generation98-and-reader-review.md) сохраняют критерии и отдельные Claims одного Унамуно. Скорость/запаздывание не вычисляются из прозы без измеримых дат. K2. |
| H05 · M22 | политическая, экономическая, культурная, религиозная, образовательная, научно-техническая среда | Registry/Assessment → context profile, context paths | конфигурация мира, области и время действия | **локальный профиль и реальный корпус**: environment имеет шесть независимых content properties и некаузальные связи; [два Basel-пакета](../review-ledger/2026-09-07-basel-print-environment-and-reader-review.md) сохраняют области, формы, property filters и один Place. Незаполненная область неизвестна, а не требует выдуманного факта. K2. |
| B01 · M19/M22/M25 | интеллектуальный объект, произведение, замысел, черновик, редакция, языковое выражение | Corpus/Registry → work/version/expression profile | что за объект и какая реализация | **локальное исполнение**: [Work De constantia](../review-ledger/2026-09-07-intellectual-formations-and-reader-review.md) и [Work → Expression Jenseits](../review-ledger/2026-09-09-native-work-expression-growth-review.md) созданы штатно с отдельными Claims/forms и сохранёнными предшественниками. Замысел, черновик/редакция и реализация различимы; исправление описания не новая редакция и не смена ID. K1/K2. |
| B02 · M19/M25 | издание, публикация, экземпляр, документ, письмо, лекция, записная книжка | Corpus/Registry → bibliographic paths | произведение, выпуск и конкретный экземпляр | **локальное исполнение, реальный корпус**: [Expression → Edition](../review-ledger/2026-09-09-native-expression-edition-review.md), [Item/File](../review-ledger/2026-09-09-native-local-item-adoption-review.md), [Collection/membership](../review-ledger/2026-09-09-native-collection-growth-review.md), Document/Letter имеют разные operations/forms. Электронное издание не названо печатным оригиналом, письмо не его физический носитель. K1/K2. |
| B03 · M19 | фрагмент, утраченное произведение, сохранённая цитата, реконструкция | Corpus/Assessment → preserved-in, cites, reconstruct | что сохранилось и что реконструировано | **локальное исполнение, реальный корпус**: textual_survival — Claim value, не тип утраченного Work; [fragment (8) и quoting passage Бёрнета](../review-ledger/2026-09-07-burnet-fragment-quotation-source-reading.md) и [editorial composite §85](../review-ledger/2026-09-07-burnet-editorial-composition-source-reading.md) сохраняют разные слои и прежние native bytes. Неизученный древний оригинал/сопоставление редакций остаются явным пределом содержания. K2/K4. |
| B04 · M19/M25 | рукопись, бумага, почерк, носитель, устная речь, аудио/видео, цифровой файл | Corpus/Registry → material/representation profile | точный материальный и цифровой объект | **локальное исполнение**: [Artifact create/revise](../review-ledger/2026-09-09-native-artifact-creation-review.md), [native witness forms](../review-ledger/2026-09-07-native-witness-forms-and-assessment-input-review.md), Document/Letter и File/representation сохраняют разные IDs. [Реальный физический компонент](../review-ledger/2026-09-10-physical-member-structure-review.md) не наследует всё описание ансамбля и не доказывает склейку. Новый медиа-процесс требует своего evidence. K2/K4. |
| B05 · M19/M25 | история хранения, приобретение, перемещение, каталогизация, оцифровка | Corpus/Registry → provenance timeline | биография вещи | **локальное исполнение**: [реальная EPUB приёмка](../review-ledger/2026-09-09-native-local-item-adoption-review.md) разделяет acquisition, container observation, deposit и metadata serialization; Artifact custody/provenance и исторические Claims не подменяются этими событиями. [Retained-page OCR](../review-ledger/2026-09-10-retained-page-ocr-image-comparison-review.md) не подписывает старый render задним числом. Неизвестная биография вещи не заполняется фиктивно. K3/K4. |
| B06 · M19/M22 | медиа, формат, жанр и интеллектуальное членство | Registry/Growth → independent classifications, create/revise, property filters | самостоятельные классификации, оговорки и обратный переход к предмету | **локальный профиль и реальный корпус**: [четыре оси классификации](../review-ledger/2026-09-07-classification-profile-review.md) — жанр, форма, среда передачи и вид носителя — используют shared structured values/Claims/forms; MIME относится к File. Гильгамеш, Penn и письмо №705 сохраняют квалификацию; atlas category не становится типом. K1/K2. |
| L01 · M19/M25 | язык, диалект, письменность, транслитерация и перевод | Corpus/Forms → language selection, independent linguistic subjects and scoped Claims | язык, разновидность, письмо и способ записи различимы в обе стороны | **локальный профиль и реальный корпус**: [Language/Variety/Script/TransliterationScheme](../review-ledger/2026-09-08-linguistic-profile-review.md) и Claims Penn/Louvre различают язык описания/надписи, стадию, письменность и способ записи. Общие forms/filters/readers проверены; схема транслитерации не её исполнение или компетенция. Native преобразования — F07; K2. |
| L02 · M15/M19/M25 | текстовый слой, адрес, фрагмент, единица, употребление, форма, лексема и значение | Corpus/Registry → focus word/occurrence, source return | слово в точном контексте | **локальное исполнение, реальный корпус**: [Lexeme/LexicalForm/LexicalSense](../review-ledger/2026-09-08-lexical-profile-review.md), [private Occurrence/Claim/form](../review-ledger/2026-09-08-real-private-claim-growth-review.md) и [публичный Occurrence слова corpus](../review-ledger/2026-09-10-public-native-project-note-source-return.md) сохраняют distinct IDs, exact intervals и native bindings. Public пример — настоящий современный текст проекта, не древний/Nietzsche witness. Private content не выходит в public reader. K2. |
| L03 · M12/M15/M25 | знак, этимология, семантический диапазон, переводимость, мотив, изменение употребления | Registry/Assessment → linguistic comparison | альтернативные разборы и основания | **локальное исполнение**: [лексическая история](../review-ledger/2026-09-08-lexical-comparison-review.md), [translatability Leitkultur](../review-ledger/2026-09-08-translatability-profile-review.md), [motif exact-member value](../review-ledger/2026-09-08-motif-reference-value-review.md) и [отдельный Sign transition](../review-ledger/2026-09-08-sign-promotion-review.md) имеют формы/историю и свои assessment boundaries. Реальные lexical Claims не превращают синтетическую Sign/motif-проверку в исторический допуск. K1/K2. |
| T01 · M05/M13/M15 | Concept, Conception, аспект, позиция автора и утверждение исследователя | Registry/Assessment → concept history, compare conceptions | единый концепт и разные трактовки | **локальный профиль и реальный корпус**: [Concept/Conception JGB19/21](../review-ledger/2026-09-07-jgb-freedom-reader-review.md) сохраняют continuity criterion, отдельное grounded conception_of и конкурирующие чтения. [Реальные scoped semantic/Claim/Form решения](../review-ledger/2026-09-09-real-native-semantic-form-assessment-review.md) не принимают автоматически остальные трактовки. Полный concept → exact text маршрут — K2. |
| T02 · M12/M15 | проблема, семейство проблем, вопрос, тезис и позиция | Registry → problem/thesis paths | вопрос, предлагаемый ответ и область | **локальный профиль и реальный корпус**: [Problem/Family/Question/Position](../review-ledger/2026-09-07-inquiry-profiles-and-reader-review.md) имеют содержательные schemas, общий create/revise/read/form и property filters. JGB19/21 связывает вопросы, ответы и позиции отдельными Claims; их неопределённость сохраняется. K1/K2. |
| T03 · M12/M15 | различение, оппозиция, категория и аспекты | Registry → distinction/category profile | что и в каком смысле противопоставлено | **локальный профиль и реальный корпус**: [Aspect/Category/Distinction/Opposition](../review-ledger/2026-09-07-inquiry-profiles-and-reader-review.md) поддерживают общий рост/чтение/формы. Члены различения — роли, категория не технический тип, оппозиция не доказанное противоречие. Scope и источники сохраняются при сокращении. K2. |
| T04 · M12 | аргумент, посылки, переход, вывод, возражение на конкретную часть | Registry/Assessment → argument, challenge step | ход довода и адресное возражение | **локальный профиль и реальный корпус**: [Thesis/Argument/InferenceStep/Objection JGB21](../review-ledger/2026-09-07-jgb-freedom-reader-review.md) сохраняют premise/conclusion как роли в шаге, конкретную цель objection и basis отдельных Claims. Связность с exact occurrence, авторским и исследовательским чтением — K2. |
| T05 · M12 | метод, операция мышления, переносимый ход мысли, мысленный эксперимент | Registry → method/experiment profile | что делается мыслью и при каких допущениях | **локальный профиль и реальный корпус**: [method/thought-operation/move/experiment](../review-ledger/2026-09-07-practice-profiles-and-reader-review.md) сохраняют условия, допущения и следствие; JGB36 связан с тезисами и обязательством через общие commands/forms. Изучаемый метод не код, мысленный опыт не событие мира. K2. |
| T06 · M12/M15 | образ, метафора, фигура, ценность, идеал, онтологическое обязательство | Registry/Assessment → thought content profile | исходный образ и интерпретация; чьё обязательство | **локальный профиль и реальный корпус**: [шесть content profiles JGB21/36/211](../review-ledger/2026-09-07-practice-profiles-and-reader-review.md) различают образ/метафору/фигуру, ценность, идеал и обязательство. Metaphor наследует RhetoricalFigure; условное обязательство не закон ядра. Source description не самопринятая оценка. K2. |
| R01 · M19/M22 | создание, публикация, перевод: действие, результат и отношение | Registry/Corpus → production/translation event | участники, оригинал, результат, время и основания | **локальное исполнение**: production/translation event, [Expression derivation](../review-ledger/2026-09-09-native-work-expression-growth-review.md), [translator responsibility](../review-ledger/2026-09-09-native-translator-responsibility-review.md) и [DE/RU alignment](../review-ledger/2026-09-10-real-native-bilingual-alignment-return.md) не сливают действие, результат, участника и сопоставление. Native mapping proposed/unassessed. K2. |
| R02 · M19/M22/M25 | чтение, цитирование, ответ, комментарий, полемика, влияние, преподавание и распространение | Registry/Assessment → reception paths | конкретный предикат, а не общая «связь» | **локальное исполнение**: [интеллектуальные связи](../review-ledger/2026-09-07-intellectual-formations-and-reader-review.md), [quotation](../review-ledger/2026-09-07-burnet-fragment-quotation-source-reading.md) и [reception Claims](../review-ledger/2026-09-07-reception-profile-review.md) имеют отдельные предикаты, basis и inverse forms. Цитирование не доказывает согласие, чтение или причинное влияние. Полный исторический маршрут — K2. |
| R03 · M22/M25 | рецепция, наследие, историческая канонизация, забвение, переоткрытие | Registry → reception timeline | последующая жизнь произведения и мысли | **локальный профиль**: [пять reception/later-life profiles и шесть связей](semantic-interchange/README.md#reception-historical-recognition-and-later-life) используют общие operations/forms/history; [реальный Penn access/reception](../review-ledger/2026-09-07-reception-profile-review.md) сохраняет ancient cluster, Artifact и modern Work. Отсутствие записи не забвение; historical canonization не ToS admission. Незаполненные исторические сведения не выдумываются. K2/K4. |
| R04 · M15 | переопределение, отвержение, сужение, расширение, секуляризация, психологизация, политизация, инверсия | Registry/Assessment → compare transformations | направленное преобразование трактовок | **локальное исполнение**: восемь разных нетранзитивных Conception predicates имеют basis/statement/inverse reading и общие commands. [Реальная инверсия JGB21](../review-ledger/2026-09-09-jgb21-inversion-growth-review.md) сохраняет exact form, stale-input refusal, replay и counter-reading. Это сравнение критических targets, не endorsement/хронология. Другие исторические трансформации требуют своих source data. K2. |
| A01 · M19, G4 | наблюдение, утверждение, интерпретация, свидетельство, контрдовод и оценка | Corpus/Assessment → assess, inspect evidence | кто, что и на каком основании заключил | **локальное исполнение**: observation/Claim/interpretation/evidence/counterevidence/assessment event различны; [реальные source assessments](../review-ledger/2026-09-08-real-source-assessment-review.md) не переписывают raw unreviewed records. Full context и current use читаются отдельно. K1/K2. |
| A02 · G4 | валидация, содержательная оценка, полномочие и текущий допуск различны | Assessment/Growth → admit, limit, reject, dispute, defer | область допуска, оценщик, основания и ограничения | **практический ограниченный допуск**: [policy/private v5](../review-ledger/2026-09-09-native-text-layer-quality-review.md), [реальные Claims](../review-ledger/2026-09-08-real-source-assessment-review.md), [RU/EN forms](../review-ledger/2026-09-09-real-human-form-assessment-review.md) и [native semantic/Claim/Form decisions](../review-ledger/2026-09-09-real-native-semantic-form-assessment-review.md) разделяют компетенцию, action/scope, assessment и use. Admit/limit/reject/dispute/withdraw не требуют human подписи под каждой записью. Local-account binding не remote authentication, права, canon или publication. K1/K2. |
| A03 · G4 | риск, язык, задача, метод, модель и независимость | Assessment → qualified reusable grant, audit | область компетенции и предел вывода | **ограниченная подтверждённая компетенция**: [English citation/observation](../review-ledger/2026-09-09-real-native-quality-assessment-review.md) и [RU/EN semantic/Form trial](../review-ledger/2026-09-09-real-native-semantic-form-assessment-review.md) связаны с actual reviewer/configuration, language/risk/method и expiring grants. Twelve-case English trial: 11 literal labels и одно проверенное различие; correlated model family не независимое подтверждение. Ancient-language/OCR qualification не выдана. K1/K2. |
| A04 · M19, G4/G7 | отзыв, supersession, конфликт и история знания | Assessment/Growth/Processing → revoke, reassess dependencies | история, спор и последствия отзыва | **практически проверенная отмена и история**: [Claim/RU Form lifecycle](../review-ledger/2026-09-09-real-human-form-assessment-review.md), [Layer/Unit withdrawal](../review-ledger/2026-09-09-real-native-quality-assessment-review.md) и [expiry/reassessment](../review-ledger/2026-09-09-real-native-semantic-form-assessment-review.md) сохраняют события и stale bases. Старый positive retry/новый grant не воскрешает use; новая оценка Layer не принимает старый Claim/Form. Общий processing cascade и стоимость — K3. |
| A05 · G4 | источник/модельный ответ — данные, не инструкции/права | Growth → authenticated command binding | видимые полномочия и причина отказа | **локальная граница полномочий**: independently protected owner configs задают subject/action/risk/competence; source/model bytes инертны. [Discovery](../review-ledger/2026-09-09-command-discovery-and-temporal-comparison-review.md) не читает grants; [public native scope](../review-ledger/2026-09-10-public-native-project-note-source-return.md) отдельно связывает фактическую делегацию. Подмена actor, schema и dependencies имеет negative controls. K1. |
| A06 · G4/G11 | реальное качество и доля необходимого участия человека | Assessment → task/language evaluation, sampled audit | измеримые ошибки, возражения и эскалации | **измеренные ограниченные пакеты**: [native quality](../review-ledger/2026-09-09-real-native-quality-assessment-review.md), [RU/EN Form](../review-ledger/2026-09-09-real-human-form-assessment-review.md) и [semantic/Form](../review-ledger/2026-09-09-real-native-semantic-form-assessment-review.md) сохраняют proposals, отказ, исправленную до append ошибку, issuer audit, committed decisions и withdrawal. [11 historical Claims/44 Forms](../review-ledger/2026-09-10-real-historical-forms-reader-review.md) прошли полный source-visible audit и native append/replay с сохранёнными ограничениями. Новых per-record human approvals = 0; полностью проверенный correlated cohort не population intervention rate, held-out accuracy или общая language/OCR qualification. K2 остаётся отдельным. |
| X01 · M19, G4/G9 | доступ, лицензия, права, согласие, авторство, публикация и canon | Corpus/Assessment → allowed use, visibility, stronger-owner route | доступность конкретного слоя и ограничение | **локальное исполнение**: [local Item rights/deposit](../review-ledger/2026-09-09-native-local-item-adoption-review.md), private assessment и [отдельная public-native authority](../review-ledger/2026-09-10-public-native-project-note-source-return.md) проверяют current access до content IO. [Native Link](../review-ledger/2026-09-09-native-object-link-growth-review.md) сообщает доступность, не лицензию. Оценщик не выдаёт чужие права; final withdrawal/restriction — K1/K2/K3. |
| V01 · M19/M25, G5 | имя, caption, hover, statement, основания и technical view | Forms/Research → select form, inspect exact | один смысл на разных уровнях подробности | **локальное исполнение форм и реальная доставка**: HUMAN_FORMS и общий materializer различают source-copy/template/freeform, roles/language/provenance/context/history. [Реальные RU/EN hover](../review-ledger/2026-09-09-real-human-form-assessment-review.md) сохраняют отрицание и спорность; oversize отдаёт exact ref вместо усечения. [Общий v2 reader](../review-ledger/2026-09-10-real-historical-forms-reader-review.md) доставил все 44 полных packet на 11 реальных Claims внутри прежнего wire budget, включая focused Claim context и HTTP. Пробел не маскируется ID; готовность UI и приемлемая стоимость остаются K2/K3. |
| V02 · G5 | язык источника, перевод и язык UI, происхождение каждой формы | Forms → fallback, translate, inspect provenance | честный выбор доступного языка | **локальное исполнение языка/происхождения**: расширяемые language/script keys и original/translation/transliteration/adaptation не сливают язык UI с источником. [Collection forms](../review-ledger/2026-09-09-native-corpus-descriptive-revision-review.md) сохранили строки/историю; [Python/Worker assessed parity](../review-ledger/2026-09-09-assessed-form-consumer-parity-review.md) и [реальный shared-v2 round trip](../review-ledger/2026-09-10-real-historical-forms-reader-review.md) сохраняют точные формы, отдельные admissions и упорядоченные ограничения, не усиливая statement. Новое преобразование требует своей quality review. K2/K4. |
| V03 · G5 | содержательная готовность ≠ непустая строка | Forms/Processing → quality coverage, regenerate dependent form | наблюдаемый missing/restricted/unsupported | **локальная диагностика, не полнота качества**: [carrier coverage](../review-ledger/2026-09-09-knowledge-coverage-review.md) различает missing/restricted/unsupported/ambiguous, заглушки, provenance и roles. [Legacy Link](../review-ledger/2026-09-09-legacy-object-link-context-review.md) сохраняет отсутствие statement вместо выдуманного текста. Source-enumerated remainder — K4; непустая строка не quality acceptance. |
| V04 · M25, G5/G6 | компактная линия может раскрывать Claim или путь | Forms/Research → explain edge composition | точное членство, направление и правило сокращения | **локальное исполнение**: [compact Claim paths](../review-ledger/2026-09-07-compact-claim-scene.md), [assessed context parity](../review-ledger/2026-09-09-assessed-form-consumer-parity-review.md) и exact inspection сохраняют membership/direction/versions/evidence/counterevidence и правило сокращения. Путь не становится одним source fact. Настоящее раскрытие — K2. |
| Q01 · M23/M25, G6 | любой предмет и отношение могут стать центром | Research → resolve ambiguity, focus, expand, deepen | устойчивый центр, близкие связи и причины | **локальное исполнение**: [node/relation origin v2](../review-ledger/2026-09-09-typed-exploration-origin-review.md) и [Item/relation core/HTTP/MCP](../review-ledger/2026-09-09-native-local-item-adoption-review.md) проверяют endpoints/context/continuation. [Публичный Occurrence реально существует](../review-ledger/2026-09-10-public-native-project-note-source-return.md); прежнее отсутствие больше не текущий диагноз. Final-union focus всех требуемых центров и сцена — K2. |
| Q02 · M25, G6 | обнаруживаемые типы, свойства, роли, фильтры и операции | Registry/Research → catalog, semantic filter | конструктор без внутренних JSON-путей | **общие контракты, consumer gate открыт**: Registry/catalog/LensSpec обнаруживают type/property/role/filter IDs; [grant-free command discovery](../review-ledger/2026-09-09-command-discovery-and-temporal-comparison-review.md) использует реальный dispatch. Node/path values/operations/unknown проверяются по тому же snapshot в Python/Worker/D1. Составление необходимых запросов человеком/агентом — K2; дополнительные sort/group или специальный экран не заменяют его. |
| Q03 · M15/M22/M25, G6 | близость по именованному правилу, пути, подклассы, время, сравнение | Research → named neighborhood, path conditions, compare | почему включён узел; не только hop count | **локальное исполнение**: named neighborhood, path conditions, subtype/role/property filters, [identity-aware traversal](../review-ledger/2026-09-07-identity-carrier-traversal.md) и [temporal comparison](../review-ledger/2026-09-09-command-discovery-and-temporal-comparison-review.md) сохраняют inclusion reasons. Provenance не содержательная близость. Реальные составные routes/compare/source/continue — K2. |
| C01 · G6 | общий язык разрешённого роста для человека и агента | Growth → create, revise, assert, assess, merge/split proposal, extend | понятная команда, diff и конфликт | **локальная грамматика роста**: create/revise/Claim/form/assessment; [Work/Expression](../review-ledger/2026-09-09-native-work-expression-growth-review.md), [Edition](../review-ledger/2026-09-09-native-expression-edition-review.md), [Item](../review-ledger/2026-09-09-native-local-item-adoption-review.md), [Collection](../review-ledger/2026-09-09-native-collection-growth-review.md), [Artifact](../review-ledger/2026-09-09-native-artifact-creation-review.md), [Link](../review-ledger/2026-09-09-native-object-link-growth-review.md), native text/alignment и versioned identity proposals имеют отдельные closed grants, expected refs и replay/conflict/recovery. Access остаётся read-only. F04 включает отдельный v2 для явно подключённых semantic subjects без расширения старых grants или исполнения слияния. Discoverable human/agent consumption — K1/K2. |
| S01 · G7 | обработка по точным зависимостям и версиям метода/словаря/политики | Processing → reuse, invalidate, incremental rebuild | причина и область пересчёта | **локальные механизмы; final адресность не принята**: source/registry/method/policy/assessment dependencies, normalization cache и immutable publication различают reuse и новое исполнение. Новые формы/линзы не запускают OCR. После последнего source batch нужны реальные измерения affected rebuild и малых изменений — K3. |
| S02 · G7 | ограниченная работа, очереди/кэш, восстановление и согласованный снимок | Processing → resume, publish snapshot, evict cache | продолжение или честное устаревание | **локальные transaction boundaries**: [selected metadata](../review-ledger/2026-09-09-native-work-expression-growth-review.md), [bounded deposit/recovery](../review-ledger/2026-09-09-native-local-item-adoption-review.md) и native no-replace сохраняют pending/commit/rollback, current grants, third-state/ABA refusals. Межкорневая атомарность не обещана. Final snapshot/restart/expiry/cache/queue и local-D1 согласованность — K3. |
| S03 · G7 | скорость измеряется при росте данных | Processing/Research → benchmark bounded deltas and requests | понятные фактические бюджеты | **K3 открыт**: обязательны cold/warm catalog/search/focus/expand/inspect; add/edit/delete/review/access; failure/resume и рост данных с обоснованными latency/compute/storage budgets. Dated source canaries не общий benchmark или waiver; холодные full-reader десятки секунд не объявлены приемлемой задержкой UI. |
| M01 · G8 | прямое отображение, адаптер, уточнение, неизвестность, ограничение | Corpus/Processing → shadow migration, classify remainder | статус каждого объекта и следующий владелец | **bounded K4 source audit выполнен; final K1 открыт**: [source-enumerated return](../review-ledger/2026-09-10-foundation-source-mapping-remainder-review.md) проверил тогдашние 678/678 catalog identities, 253 CSV rows/92 authored nodes, 16137 atlas rows/4 manifests, 17599 backlog records и 23 public Item manifests/29 File identities по отдельным знаменателям. [Новая ordinary-reader проверка](../review-ledger/2026-09-10-real-historical-forms-reader-review.md) охватила 684/684 текущие catalog identities (654 direct, 30 adapted), 408 source files, полные fields/IDs/refs и конечные source rechecks. Она не переаттестует прежнюю private metadata область или весь uncatalogued remainder; там сохраняются прежние точные owner/next-action и отсутствие глобального private denominator. Reader/registry currentness и final sealing остаются K1. [Counterevidence mapping](../review-ledger/2026-09-10-claim-counterevidence-mapping-review.md) добавляет только объявленную Claim→Evidence семантику; пять lawful atlas unknown edges остаются точными исходными связями, не поглощаются fallback и не принимаются автоматически. |
| M02 · G8 | откат читателя не откат источника и решений | Processing/Growth → switch snapshot, rollback reader | согласованная версия и история | **локальное разделение; K3 открыт**: source/Claim/form/assessment histories не стираются при rollback derived reader/snapshot. Exact-version reader выдаёт gap, не latest; [legacy adapter](../review-ledger/2026-09-09-legacy-object-link-context-review.md) не меняет старые bytes/IDs. Final snapshot switching/reader rollback и stale continuation проверяются отдельно от source rollback. |
| U01 · M25, G9 | одно Древо на разных масштабах, не копии сцен и объектов | Consumer → focus, lens, select, inspect, continue | пространство, лаконичность, плавное углубление | **K2 открыт**: отдельный observatory owner сохраняет spatial graph, identity scene map, camera/motion/gestures и компактные формы. [Локальная интеграция](../review-ledger/2026-09-08-observatory-foundation-integration.md) и последующие code/build checks не заменяют actual final-union interaction: оба маршрута, focus/expand/filter/compare/source/continue и плавность. |
| D01 · G10/G11 | source, local checks, review, CI, merge, release, runtime, acceptance различны | owner release routes → verify and handoff | честная граница готовности | **K1–K4 и разрешённая поставка ещё не закрыты**: source commits/checkpoint-review не CI/merge/release/deployment/runtime/semantic acceptance. Exact baseline, проверки и состояние поставки подтверждаются отдельно через docs/RELEASING.md. Приостановленная host production delivery не запускается этой картой. |

## История первых исполнений и их тогдашних пределов

Далее сохранена хронология предыдущих срезов. Слова «теперь», «ещё» и старые
счётчики в ней относятся к указанным исходным checkpoint, а не к текущему
union. Текущее покрытие и оставшиеся gates задаёт матрица выше; её ссылки ведут
к более поздним source-owned продолжениям. История не переписывается в новый
положительный результат и не создаёт дополнительных условий вне цели.

На базе `53078f393a5b6d9d5bada239e8b2e6d5f2573e2d` сохранены существующие
реестры, точные source records, LensSpec с условиями на пути, объяснения,
продолжение и промежуточные кэши. Старые диагнозы «всё отсутствует» не
применяются автоматически. Их полнота по настоящей карте ещё не доказана.

Первое изменение добавляет Assessment schemas, политику и чистый механизм
допуска. `mechanics/growth-cycle/tests/test_knowledge_assessment.py` проверяет
точные зависимости, доверенный scope, подмену оценщика/метода, повторение,
независимость, конфликт, ограничение и withdrawal на искусственных записях.
Журнал `assessment_journal.py` добавляет неизменяемые пакеты, атомарное
применение оценок одного предмета, expected revision, повтор команды,
восстановление и сохранение supersession при последующем отзыве полномочия.
Проверки охватывают синтетическую историю после перезапуска, конкуренцию двух
писателей, сбой вокруг публикации и повреждение данных. Они не удостоверяют
языковую компетенцию, реальный source review, аутентификацию адаптера,
межпредметные транзакции или готовность всего профиля.
Локальный command entrypoint журнала теперь связывает реальный Unix UID с
доверенной конфигурацией владельца, отдельно от запроса; проверяет exact
snapshot/subject/head, защищённые пути и отказ на подмену контекста. Проверены
append/replay и текущий отзыв допуска, CLI inspect/error и инертность команд
в source payload. Это аутентификация аккаунта, не отдельного model invocation;
реальная калибровка, corpus adapter и изоляция чужих процессов остаются открыты.
Локальная `materialize-form` теперь связывает точную исходную freeform-форму,
её предшественников и журнал с тем же движком допуска. Обязателен полный
контекст предмета; отзыв, отсутствие оценки и новая непроверенная версия не
выдают текст. [Две реальные RU/EN формы метаданных Хаммурапи](../review-ledger/2026-09-07-assessed-form-command-review.md)
первоначально были сохранены как предложения и возвращали `needs-assessment`,
без поддельного review. [Локальная сборка оценённых форм](../review-ledger/2026-09-07-assessed-graph-reader-review.md)
теперь соединяет тот же журнал с обеими существующими проекциями и общим
читателем; смешанные оценки между носителями отвергаются. На 2026-09-09 UTC
[обе формы действительно оценены и допущены в узком research scope](../review-ledger/2026-09-09-real-human-form-assessment-review.md)
после отдельного RU/EN metadata trial и source-visible issuer audit. RU прошла
реальные отзыв и новое чтение/оценку, EN и source не изменились. Обе проекции
сохраняют эту историю и полный контекст; увеличившийся пакет RU возвращает
компактную exact-reference форму и целый текст в инспекции. Это не общая
языковая компетенция, готовое открытие ссылки в UI, публичная поставка или
подключение runtime.
Конфигурация v2 добавляет ограниченный прямой reader исходных corpus/claim/form
records и `describe` для получения текущих command refs. Реальный Jenseits
Work → Expression Claim и его источники проходят CLI без копии корпуса в
конфигурации; исходный `unreviewed` сохраняется. На временных копиях проверены
изменение источника, непрозрачное расширение, подмена maker/layer, дубли,
nonpublic visibility и path escape. Это ещё не исполняемая агентная оценка.
Тот же reader теперь получает объявленные metadata/Claim профили из реестров,
связывает точные схемы и выбранные концы отношений без обхода корпуса.
Реальные Letter и три отдельных Claim проходят чтение с сохранением полных
записей и `unreviewed`; синтетический новый подтип добавлен только данными.
Ошибочные domain/range, подмена схемы старой оболочкой и устаревший контракт
отвергаются. Это подключение источников к оценке, не калибровка оценщика и
не выданный допуск.
Публичные объявленные Claim и их формы теперь наследуют точное обязательное
основание из выбранных типизированных концов и native TextUnit/layer.
Оценка должна явно процитировать каждый такой record; подставленный inline
Claim, неполное чтение, исправленное основание и конкурентный source drift
не сохраняют текущий допуск. История и исходные статусы не переписываются
([проверка](../review-ledger/2026-09-08-public-claim-assessment-grounding-review.md)).
Это общий вход в компетентную оценку, а не уже выполненная содержательная
оценка, Sign-promotion или закрытие языкового профиля.

На 2026-09-08 поверх этого входа выполнен отдельный
[реальный ограниченный пакет source observation](../review-ledger/2026-09-08-real-source-assessment-review.md):
три существующих scholarly_report Claim получили research admission с
ограничениями после source-visible агентной оценки и проверки issuer.
Семь недопустимых запросов отклонены; повтор не создаёт новую историю;
отдельных human подписей под записями не было. Один допуск затем реально
отозван и заменён новым результатом повторного чтения; старые события
сохранены, использование старого receipt/конфигурации не отменяет отзыв.
Это уточняет прежние отметки
о незапущенной оценке, но не доказывает общую языковую компетенцию,
Sign-promotion, независимое историческое подтверждение или runtime admission.

Motif теперь имеет отдельную квалифицированную форму Claim над полным
набором точных Occurrence: общий типизированный reference-value reader,
явные версии write-полномочий, исправление состава и точные повторы,
обязательное чтение всех участников/их native-оснований и полную расшифровку
компактного пути. Проверки пока синтетические; [обзор и пределы](../review-ledger/2026-09-08-motif-reference-value-review.md)
не объявляют исторический мотив принятым знаком. Поддержка отдельного UI
подготовлена; совместное HTTP-потребление, реальные Motif-оценки,
реальная Sign-оценка и произвольно большие наборы ещё не завершены.

Sign имеет отдельный source-профиль с неизменяемым основанием выдачи ID и
операцию `sign.promote` через существующий command owner. Policy v2 отделяет
разрешение этого перехода от `research`, сохраняет agent/human/mixed оценку,
риск, точного кандидата и все native-основания. Generic create не обходит
переход; формы несут ограничения, а историческое основание не является
текущим допуском. Тесты остаются синтетическими: реальная компетентная выдача,
private/annotation adapters ещё требуют доведения. Переход Sign → точная
версия Claim теперь имеет отдельный RecordVersion view: полный архивный
record и provenance либо явный gap без подстановки текущего Claim; compact
сохраняет точную ссылку и весь assertion context. Отдельный публичный metadata
reader теперь подключает историю native Agent/Place/Organization/Work и
объявленных metadata-профилей, проверяя выбранные record bytes и полную цепь
переходов без чтения companions или текущего допуска. Компактная версия
сохраняет весь record context и собственный объявленный язык; не все native
семейства и исторические свободные формы ещё подключены. [Review выдачи](../review-ledger/2026-09-08-sign-promotion-review.md)
и [review точной версии](../review-ledger/2026-09-08-exact-claim-version-review.md)
сохраняют пределы проверок; [metadata history review](../review-ledger/2026-09-08-exact-metadata-version-review.md)
отделяет выбранные record bytes от companions и текущего допуска.
Полнота Sign и языкового профиля не заявлена.

На 2026-09-06 отдельный маршрут source-visible linguistic reviewer не запущен:
запрос `aoa-models` для `source-visible-linguistic-review` и указанного в его
read-only realization exact runtime subject вернул 0 candidates
(`aoa-models@24835f908bf032767e3ee4f3bd57cd832e0c7fa1`, result digest
`sha256:25905a48d9567d3a12676dae9ee8d6bd87493930a85772237c1960c324759e3c`).
Существующая model claim ограничена currentness hypothesis, без behavioral
fit и independent review. Следующий владелец этого допуска — `aoa-models`;
требуется предметно подходящее reviewed fit evidence, не подмена его каталогом
или собственным `verified`. Этот результат не удостоверяет текущую активацию
runtime и не закрывает ни агентную компетенцию ToS, ни всю цель Foundation.

Языковой транспорт `access` и schemas словарей теперь сохраняют расширяемые
языковые/письменные ключи вместо фиксированного набора ru/en. Исходная фраза
на новом языке проходит обычный и compact-пакет без утвердительной подстановки;
каталог показывает доступные поля, LensSpec принимает их в фильтрах и сортировке.
Проверки `test_knowledge_contract.py` и Worker `knowledge.test.ts` сопоставляют
Python, чистое исполнение TypeScript и локальный D1. Lens выдаёт явный выбор
формы с причиной fallback, языком при известности и привязкой к content revision;
общая заглушка не считается содержательным описанием. Полный и compact carrier
сохраняют исходные оговорки, отрицание, уверенность с её смыслом, конкурирующие
Claim и контрдоводы с точными указателями и digest публичной исходной записи.
Связанные линии зависят от контекста Claim в кэше нормализации; изменение
оговорки инвалидирует зависимую линию, не меняя её идентичность.
Это не полная Forms-модель: поэлементное происхождение, содержательное
сокращение с оговорками, остальные пути выдачи и UI-потребление ещё нуждаются
в связанном исполнении; язык ключа и качество перевода не удостоверены.

Следующие связанные исполнения: durable Growth command/history adapter и
реальная агентная оценка; Forms/context ABI с оговорками и языками; полные
предметные профили; общие запросы; подключение корпуса, адресная обработка и
совместное потребление. Ни одна строка пока не помечена закрытой.

Общий `source-claims.jsonl` теперь имеет отдельный путь форм Claim:
`claim.statement` копирует полное поле с целым утверждением как обязательным
контекстом. Раздельно делегированные `form.create`/`form.revise` сохраняют
историю, связывают source profiles/schemas и не меняют исходные Claim.
Три утверждения письма № 705 получили такие формы; `unreviewed` и отсутствие
допуска сохранены. Python, Worker и локальный D1 проверены на общей выборке
с точной привязкой к Claim, а не к его объекту. Это не проверка содержательного
качества, реальные жесты UI или миграция всех прежних Claim-семейств.

`HUMAN_FORMS.md`, source-form/template schemas и owner-механика `human_forms.py`
теперь различают форму и её предмет, точное копирование, конечный шаблон и
свободный пересказ с текущим вызовом assessment engine. Существенные привязки
задаёт исходный владелец; нельзя убрать оговорку, подменить источник или
использовать старую оценку после изменения зависимости. Проверки механики
синтетические и не подтверждают реальную языковую компетенцию. Источниковый
адаптер для полных библиографических имён и notes теперь переносит текущие
формы и обязательный контекст в существующий библиографический граф; исходный
набор Jenseits содержит три копии полей без новой семантической оценки.
Формы сохраняются в атрибутах полной access-инспекции. Lens-выдача выбирает
по роли и языку в Python и Worker/D1, сохраняет целую форму с контекстом либо
возвращает точную ссылку при превышении бюджета. Конкурирующие формы остаются
неоднозначными; допуск исходного снимка не переоценивается читателем.
Материализатор и читатели различают источниково привязанные original,
translation, transliteration и adaptation; точные метаданные и исходная
формулировка остаются обязательным контекстом. Это ещё не содержательная
оценка языковой связи. Реальные источниковые метаданные для этого нового
контракта, содержательные шаблоны, другие адаптеры и UI пока не интегрированы.

Командный адаптер `source_commands.py` теперь обнаруживает поля по смысловым
селекторам, подготавливает source-copy и атомарно создаёт/исправляет формы
одного библиографического предмета в существующем соседнем файле. Предшественники
и квитанции сохраняются вместе, повтор после перезапуска не повторяет запись;
конфликт, отзыв полномочий и непроверенная свободная форма не дают допуска.
Проверены CLI, конкуренция, сбой до/после публикации и частичное обновление форм
после правки источника: остальные остаются явно `stale`, а не исчезают.
Проверка текущего каталога в памяти охватила 171 запись восьми видов и 460
полных source-copy форм без изменения исходных байтов (максимальный набор
9 042 bytes, суммарно 621 348 bytes канонического JSON без квитанций).
Это измерение подготовки и чтения, не факт массовой записи, исходной языковой
полноты или содержательной оценки. Реальная миграция, команды остальных
предметных профилей и межпредметные транзакции ещё не выполнены.

На 2026-09-07 UTC [миграция метаданных](../review-ledger/2026-09-07-source-metadata-forms-migration.md)
выполнена через этот командный маршрут: созданы 170 соседних наборов,
прежний набор Jenseits сохранён побитово. Все 171 библиографические записи
восьми видов теперь имеют 460 полных форм имён и примечаний; повтор 170 команд
не изменил файлы и не добавил квитанций. Исходные записи и каталоги сохранены.
Десять уже существовавших предметов без Claim стали доступны в графе через
общее правило включения каталожных идентичностей, без новых утверждений или
связей. Пять object-link записей остаются у отдельного адаптера; это не полный
перенос всего ToS и не закрытие профиля Forms. Содержательная оценка,
остальные роли форм, предметные профили, Worker/D1 и UI-потребление сохраняют
свои незавершённые проверки.

[Парное измерение кодека ревизий](../review-ledger/2026-09-07-normalization-revision-codec.md)
на тех же публичных входах уменьшило среднюю стоимость полной нормализации
примерно на 9% без изменения графа, ревизий и проверенных ответов потребителя.
Это не закрывает S03: измерены только два прохода каждого варианта; холодное
чтение, каталог/поиск, адресность запросов и бюджеты на растущем корпусе ещё
требуют доведения.

[Применимость свойств по типам](../review-ledger/2026-09-08-property-validation-cost.md)
снижает повторную работу полной валидации без переиспользования вердиктов
разных экземпляров: на одном полном графе 5,93 → 0,665 секунды при точном
совпадении отчётов и всех пробелов. Холодный default UI lens измерен отдельно;
колебания 21,7–39,7 секунды не позволяют объявить ускорение всего запроса или
закрыть прежний UI timeout. S03, конкурентный cold start и бюджеты роста
остаются незавершёнными.

[Совместная локальная интеграция observatory](../review-ledger/2026-09-08-observatory-foundation-integration.md)
соединяет отдельную UI-ветку с Foundation и пересобирает её standalone bundle.
Проверка потребительского контракта сохраняет точный контекст Claim и форм,
отдельный язык закреплённого чтения и отсутствие выдуманного допуска.
Предшествующий реальный RU/EN canary относится к точной паре ревизий;
интегрированный снимок, остальные маршруты и бюджеты U01 ещё требуют проверки.

[Индексированная инспекция](../review-ledger/2026-09-07-indexed-inspection.md)
теперь обслуживает узлы и связи без повторного обхода полного графа. На текущем
снимке обычный core разрешил все 39 461 ID узлов и 58 882 ID связей с их
концами; совпадение со старым читателем отдельно проверено на 171 предмете и
107 связях. Индекс сохраняет неоднозначность и заменяется при смене снимка.
Это адресность инспекции, не закрытие общего конструктора: холодная подготовка,
LensSpec/focus, поиск, каталог, растущие данные и runtime-потребление остаются
отдельными незавершёнными требованиями.

## История сквозного исполнения и неизменная итоговая граница

Document и Letter теперь являются отдельными интеллектуальными предметами,
не подклассами Work и не физическими носителями. Общая metadata-команда
создаёт их по объявленной схеме. В реестре отношений объявлен исполняемый
`source_claim_profile`: новый identity-to-identity предикат добавляется данными,
а не веткой Python. Общий читатель сохраняет весь Claim, происхождение,
контрдоводы и неизвестные расширения; проверяет конкретные domain/range
и разрешённый слой; каталог раскрывает контракт потребителю.
Отправитель, адресат, автор, носитель, историческая связь и соотнесение
с произведением различаются. Сквозная синтетическая проверка проходит
source → catalog → graph → access focus в обе стороны. Это проверка грамматики,
не историческое свидетельство. Отдельно делегированная `claims.create`
атомарно записывает пакет таких Claim между существующими предметами,
закрепляя точные входы, происхождение и квитанцию; новый предикат проверен
как данные реестра. Общие пересмотр/оценка этих Claim,
значения и датировки документов, полный реальный маршрут и все девять полных
предметных профилей ещё не завершены.

Реальная [запись письма № 705](../source-witnesses/documents/friedrich-nietzsche/naumann-letter-705/letter.json)
создана `source.create` по повторно прочитанному eKGWB. Два штатных
представления backend сохраняют один Letter ID, полный источник и три
source-copy формы (русские имя/описание и немецкий заголовок).
Это provisional-описание, не допуск и не доказательство исполнения просьбы
Ницше. [Три отдельных Claim](../source-witnesses/relations/nietzsche-letter-705/source-claims.jsonl)
созданы атомарной `claims.create`: указанный отправитель, связь эпизода с
письмом и атрибутированное Зоммеру соотнесение с произведением. Название книги
не выдаётся за прямое упоминание в письме; чтение первичного текста и
комментарий не считаются независимыми свидетельствами. Допуск не выдан.
Предварительная [идентичность Constantin Georg Naumann](../source-witnesses/agents/constantin-georg-naumann/agent.json)
теперь создана общей `source.create` для существующего нативного Agent с
точными именем и описанием. Человек не слит с издательством/типографией.
[Отдельный Claim адресатства](../source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl)
с собственной русской формой statement связывает его с письмом: сохранённая
заметка о чтении eKGWB остаётся исследовательским основанием, не новым
независимым свидетельством. Доставка, чтение, согласие и влияние не выводятся.
Бумажный носитель, содержательные оценки и все уровни человеческих форм
утверждений ещё требуют доведения общих операций.

Объявленный source-metadata профиль теперь поддерживает отдельно делегированную
команду `source.create`: начальный provisional-предмет, точные source-copy
формы и происхождение сериализации публикуются одной атомарной операцией.
Синтетический новый тип, отсутствующий в Python, проверяет путь schema/profile
→ создание → штатный граф → последующее уточнение формы без потери полей.
Неизвестная версия и изменение ещё не использованной схемы после подготовки
отвергаются; claims и admission не выдаются вместе с правом записи метаданных.
Это расширяемость одной общей операции, не закрытие девяти предметных профилей.

Нативные Agent/Place/Organization используют ту же транзакцию `source.create`
по своему прежнему Corpus-контракту. Положительные и отрицательные проверки
покрывают все три вида, язык/неизвестный контекст, запрет скрытой атрибуции,
конкуренцию, сбой и отзыв полномочий. Первоначальные квитанции проверяются
по связанным файлам и сохранившейся истории; законная последующая правка
не стирает и не повторяет создание. Выявленная коллизия ID новой формы с
формой Claim устранена в общем пути создания. Это не реализация общих
Expression/Edition/Collection/Item/Artifact-транзакций, роста всех ролей
или индексированного командного backend.

Начальный Work теперь также создаётся той же metadata-транзакцией:
[реальное De constantia](../review-ledger/2026-09-07-intellectual-formations-and-reader-review.md)
имеет собственную устойчивую идентичность, русские формы и латинский вариант
названия, без выдуманной языковой реализации, издания или экземпляра.
Обязательный пустой `expression_claim_refs` означает отсутствие поданных
утверждений, не отсутствие реализаций в мире. Команда отвергает исходящие
библиографические связи и поля других слоёв; более сильная ответственность
и хронологическая замкнутость существующего Nietzsche source home сохранены.
Это начальная идентичность Work, не общая многопредметная транзакция всей
библиографической цепочки.

Исторические входы схем отделены от действующего контракта: точные прежние
байты сохраняются по SHA-256, а исходные события не переписываются после
расширения схемы. Полная проверка source-foundation теперь проходит без
19 прежних ошибок сравнения исторического входа с текущей схемой;
[границы проверки](../review-ledger/2026-09-07-historical-contract-inputs.md)
не означают подлинность исполнения, содержательный допуск или готовность v1.

Обзорная линза и продолжение обхода больше не используют общих создателей
записей или события сериализации как близость предметов. `all` и точная
инспекция сохраняют технические связи; Python, Worker и D1 проверены на одном
правиле. На Claim адресатства письма № 705 обзор глубины 2 уменьшился с
предела 200 до 9 носителей (7 предметов). Backend теперь выдаёт отдельную
[карту сцены](../review-ledger/2026-09-07-scene-carrier-contract.md), которая
объединяет носители одного объявленного ToS ID в вершину, сохраняя точные
записи и все отношения. Python, Worker и D1 дают одинаковую карту после
фильтрации и пагинации. Обзорный обход теперь раскрывает носители одного
объявленного ToS ID без дополнительного шага отношения, сохраняя фильтры,
лимиты и продолжение. [Проверка identity-обхода](../review-ledger/2026-09-07-identity-carrier-traversal.md)
подтвердила реальный маршрут человек ↔ письмо № 705 на глубине 2 в обоих
направлениях. Backend также предлагает [компактный вид Claim-путей](../review-ledger/2026-09-07-compact-claim-scene.md):
точные основания и оговорки сохраняются, полный граф доступен отдельно,
конкурирующие утверждения не сливаются. При фокусе на Науманне реальный
пакет содержит две видимые вершины и один путь к письму; при фокусе на письме —
пять вершин и четыре пути. Это пока контракт представления: обычная глубина
запроса всё ещё считает шаги отношений через Claim. Потребление владельцем UI,
семантическая глубина обхода и плавность реального взаимодействия ещё требуют
работы; это не завершённый содержательный фокус.

Рост исторического профиля получил отдельную команду `historical.create` у
существующего source-владельца: новый provisional-предмет, начальные Claim и
source-copy формы появляются одним атомарным commit каталога с квитанцией.
Синтетическая проверка проходит существующие catalog/graph/access читатели,
конкуренцию, отзыв полномочий, потерю процесса и повтор после перезапуска.
Конфигурация создания v2 записывает собственное provenance-событие
сериализации вместе с запросом, описанием среды и хеш-связанной квитанцией.
Граф сохраняет исходное событие v2; оно не подменяет историческую датировку
и не удостоверяет предшествующее исследование или содержательную оценку.
Реальный начальный эпизод заказа Ницше 3 июня 1886 года теперь создан этой
командой по атрибутированному сообщению Зоммера: предмет, три Claim и две
source-copy формы дошли через текущий каталог и граф до штатного CLI/Core.
Это частичное исполнение F01/F05 и Growth. Полный исторический маршрут с
носителем письма, контрагентом, средой и рецепцией ещё не завершён.
Отдельное последующее [чтение письма № 705](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md)
теперь связывает критический текст с архивной записью и изображениями листа 8
GSA 71/BW 291,1. Оно также различает просьбу ускорить печать в письме и
характеристику начала работы у Зоммера. Это внесено в версию 2 описания
существующего эпизода без изменения первоначальных Claim и их допуска.
Техническая команда `record.revise` теперь сохраняет прежний плоский пакет
побайтно и атомарно заменяет текущую историческую запись, связанные формы и
историю исправления. Проверены выборочные поля без смены ID, неизвестные
сопутствующие байты, отказ на устаревший пакет, конкуренция с form-writer,
реальная потеря процесса до/после замены, повтор и текущий graph/form reader.
Реальная команда сохранила все семь исходных файлов (23 631 байт) в точном
архиве, согласованно выпустила описание и две формы версии 2 с `ru`/`Cyrl`.
Повтор отдельным CLI-процессом не переписал результат; старую версию вернул
`inspect-version`. Штатный Core выбрал обе русские формы с точными оговорками
и без переноса допуска. Изменился один исходный графовый предмет, а все
196 Claim traces и 1327 связей остались прежними. Это ещё не команда изменения
Claim или нескольких предметов и не решение об исторической истинности.
Точный scope и ограничения — в
[проверке создания](../review-ledger/2026-09-07-historical-source-creation.md).

Та же транзакция теперь выполняет `record.revise` для объявленных
metadata-профилей через отдельную конфигурацию с `profile_type_id`.
[Проверка профильного исправления](../review-ledger/2026-09-07-profile-source-revision.md)
охватывает Letter и новый синтетический тип, добавленный только данными:
создание, исправление форм, исправление записи и штатное чтение графа.
Реестр и точные схемы входят в зависимости подготовленной команды; старое
историческое полномочие не расширяется. На настоящем письме № 705 выполнена
только read-only подготовка с сохранением трёх исходных форм (`ru`, `ru`, `de`);
все шесть файлов остались прежними. Это не новая оценка письма, не миграция
неподдержанного формата и не завершение команд Claim, native-record,
identity merge/split или общего допуска.

Отдельная команда `claim.revise` исправляет выбранный Claim в общем
`source-claims.jsonl`, сохраняя ID, identity-endpoints, исходного maker, соседние строки,
версии форм и побайтные архивы. Общая история сохраняет порядок исправлений
разных Claim; удалённый префикс истории и версия выше 1 без истории отвергаются.
Шесть реальных связей мысли с §21 JGB получили различимые русские statements
вместо одной общей фразы, без повышения уверенности или допуска. Проверены
конкуренция с form-writer, сбой отдельного процесса до/после commit, текущий
отзыв полномочия, повтор исходного создания и штатный reader. Точные границы и
результаты — в [проверке исправления Claim](../review-ledger/2026-09-07-claim-correction-and-reader-review.md).
Это не исправление всех прежних форматов Claim, не переатрибуция и не
пересмотр оценки; адресный writer и масштабирование истории ещё нужны.

Датировки подключены к тому же конструктору как значения, а не искусственные
идентичности Date: явные v2-полномочия задают точные разрешённые значения и
относительные ориентиры. Прежние v1-полномочия не расширены. Общие source,
assessment, graph и access readers выполняют объявленный временной профиль;
исправление сохраняет прежние байты и контекст форм. Реальный сообщаемый
процесс набора, корректуры и печати JGB связан с произведением и месячным
интервалом июня–июля 1886; неопределённый календарь не подменён числовыми
ключами. [Проверка временных значений](../review-ledger/2026-09-07-temporal-values-and-reader-review.md)
отделяет реальное чтение от синтетических конкурирующих дат. Это не полная
биографическая модель, as-of-реконструкция, временная алгебра или завершение v1.

Два реальных маршрута обязательно включают разные источники, хотя бы один вне
Заратустры: Concept → Conception → тезис/аргумент/возражение → употребление и
фрагмент → авторское/исследовательское утверждение; исторический эпизод →
участники и роли → письмо/произведение и носитель → время/место/среда →
свидетельства → передача/рецепция. Остальные строки всё равно проверяются.

Вариации: неизвестность, отрицание, спорная дата/атрибуция, ложное тождество,
исправление, merge/split, несовместимый профиль, потеря поля/языка, отзыв оценки
и доступа, конфликт записи, сбой и восстановление. Свободное философское
описание требует содержательной оценки, не только проверки шаблона.

V1 закрыт лишь при выполнении всей карты на связанном существующем корпусе,
практическом агентном допуске без обязательной ручной очереди, адресной
обработке с измеренными бюджетами и реальном потреблении UI/агентом. История,
неопределённость и право углубления сохраняются. Развитие содержания остаётся
открытым; механизм его роста должен быть закончен.
