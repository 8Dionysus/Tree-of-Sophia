# Заратустра: рабочие свидетельства для шести узлов

**Ограничение:** это проверенная выборка, а не завершённый полный конкорданс. Для становления, утверждения и способности действовать полная смысловая проверка широких совпадений не завершена; непрямые формулировки остальных узлов также могут быть пропущены. Пустая пара не доказывает отсутствия связи.

Источник — немецкие единицы первых изданий DTA в `tos/bonsai/zarathustra/witnesses/*.json`; их главы и названия — `witnesses.json`. Удалён знак переноса DTA `¬`, пробелы нормализованы; цитаты в JSON ограничены 30 словами. `build-evidence-part-1.py` воспроизводит выбранные единицы и пары, а `search-candidates.py` — широкий исходный поиск; поиск сложных слов и Reigen дополнен отдельно. Единица может содержать несколько голосов; для неё дано составное обозначение либо null при неопределённости.

Пары рассчитаны относительно шести новых выборок и текущего `evidence.json`. `together` — одна и та же единица, `near` — непосредственно соседние немецкие единицы одной главы, порядок ID соответствует a и b. Предшествующие выборки частичны; 17 узлов там вообще не проверены. Это совпадение выборок, а не вывод о смысловой связи.

### Звери и их песня (`animals`)

Слова: Thier, Adler, Schlange. Мест: 68. Главы: I.01 (6; Zarathustra's Vorrede.), II.01 (2; Das Kind mit dem Spiegel.), III.08 (1; Von den Abtrünnigen.), III.09 (1; Die Heimkehr.), III.13 (37; Der Genesende.), IV.01 (4; Das Honig-Opfer.), IV.02 (1; Der Nothschrei.), IV.05 (1; Der Zauberer.), IV.07 (1; Der hässlichste Mensch.), IV.08 (2; Der freiwillige Bettler.), IV.11 (2; Die Begrüssung.), IV.12 (1; Das Abendmahl.), IV.14 (3; Das Lied der Schwermuth.), IV.15 (1; Von der Wissenschaft.), IV.17 (2; Die Erweckung.), IV.19 (1; Das Nachtwandler-Lied.), IV.20 (2; Das Zeichen.).

Звери узла — спутники Заратустры: его орёл и змея, а не все животные, орлы и змеи книги. Включены их реплики и непосредственно связанное повествование; в III.13: 12–15, 23–26, 48–50 и 53–62 говорят звери; 63–69 — воображаемая ими речь Заратустры. Другие змеи, в том числе змея пастуха, не отождествляются со спутницей. Выборка сценическая: местоименные отсылки вне выбранных сцен могут быть пропущены.

### Отвращение (`nausea`)

Слова: Ekel, ekeln, Überdruss. Мест: 43. Главы: I.01 (2; Zarathustra's Vorrede.), I.07 (1; Vom bleichen Verbrecher.), II.06 (3; Vom Gesindel.), II.11 (2; Das Grablied.), II.13 (1; Von den Erhabenen.), III.02 (2; Vom Gesicht und Räthsel.), III.05 (1; Von der verkleinernden Tugend.), III.07 (2; Vom Vorübergehen.), III.10 (1; Von den drei Bösen.), III.12 (3; Von alten und neuen Tafeln.), III.13 (10; Der Genesende.), IV.03 (4; Gespräch mit den Königen.), IV.04 (1; Der Blutegel.), IV.05 (2; Der Zauberer.), IV.08 (3; Der freiwillige Bettler.), IV.11 (2; Die Begrüssung.), IV.13 (1; Vom höheren Menschen.), IV.14 (1; Das Lied der Schwermuth.), IV.17 (1; Die Erweckung.).

Ekel и Überdruss обозначают разные случаи отвращения или пресыщения, а не всегда одну болезнь: III.13 связывает возвращение малого человека с отвращением ко всему существованию. Включены другие явные случаи и контраст преодоления; не включены просто неприятный визит и общее сопротивление времени. Нелексические отсылки вне выбранных сцен могут быть пропущены.

### Танец (`dance`)

Слова: Tanz, tanzen, Tänzer, Reigen. Мест: 67. Главы: I.01 (2; Zarathustra's Vorrede.), I.08 (2; Vom Lesen und Schreiben.), II.07 (1; Von den Taranteln.), II.10 (8; Das Tanzlied.), II.11 (3; Das Grablied.), III.04 (2; Vor Sonnen-Aufgang.), III.05 (1; Von der verkleinernden Tugend.), III.08 (1; Von den Abtrünnigen.), III.10 (1; Von den drei Bösen.), III.11 (2; Vom Geist der Schwere.), III.12 (5; Von alten und neuen Tafeln.), III.13 (3; Der Genesende.), III.14 (1; Von der grossen Sehnsucht.), III.15 (7; Das andere Tanzlied.), III.16 (3; Die sieben Siegel.), IV.02 (3; Der Nothschrei.), IV.09 (1; Der Schatten.), IV.10 (1; Mittags.), IV.11 (1; Die Begrüssung.), IV.13 (12; Vom höheren Menschen.), IV.15 (1; Von der Wissenschaft.), IV.16 (2; Unter Töchtern der Wüste.), IV.19 (4; Das Nachtwandler-Lied.).

Танец — и действие, и образ лёгкости, творчества, становления, любви и преодоления тяжести; контексты не равнозначны. Сохранены также иронические и буквальные танцы как контрасты. Автоматический поиск по начальным основам дополнен поиском Reigen и сложных слов; найдены и приставочные формы getanzt, hinweggetanzt и umtänzelt. Seiltänzer здесь исключён как канатоходец, хотя это родственное слово; непрямые образы движения не исчерпаны.

### Становление (`becoming`)

Слова: Werden, Werdender, Werde-Lust, Werdelustige, Fluss des Werdens, Jahr des Werdens. Мест: 20. Главы: I.17 (1; Von der Nächstenliebe.), I.18 (1; Vom Wege des Schaffenden.), I.22 (1; Vom freien Tode.), I.23 (1; Von der schenkenden Tugend.), II.02 (2; Auf den glückseligen Inseln.), II.12 (2; Von der Selbst-Ueberwindung.), II.15 (1; Von der unbefleckten Erkenntniss.), III.09 (1; Die Heimkehr.), III.12 (2; Von alten und neuen Tafeln.), III.13 (7; Der Genesende.), IV.01 (1; Das Honig-Opfer.).

Существительное Werden и родственные образования обозначают становление; werden часто лишь вспомогательный глагол и сам по себе не доказывает философский смысл. Отобраны прямые употребления и несколько сцен изменения, в том числе распад и соединение в песне зверей. Это выборка, не полный смысловой просмотр всех 423 совпадений werden и не исчерпывающий учёт иных слов изменения.

### Утверждение жизни (`affirmation`)

Слова: Ja-sagen, Ja-sager, segnen, weltsegnend, weltliebend, das Leben lieben, Ewigkeit lieben. Мест: 66. Главы: I.02 (2; Von den drei Verwandlungen.), I.08 (1; Vom Lesen und Schreiben.), I.11 (1; Vom Krieg und Kriegsvolke.), I.22 (2; Vom freien Tode.), I.23 (3; Von der schenkenden Tugend.), II.10 (1; Das Tanzlied.), II.15 (3; Von der unbefleckten Erkenntniss.), III.04 (7; Vor Sonnen-Aufgang.), III.12 (1; Von alten und neuen Tafeln.), III.13 (2; Der Genesende.), III.14 (1; Von der grossen Sehnsucht.), III.15 (1; Das andere Tanzlied.), III.16 (21; Die sieben Siegel.), IV.17 (2; Die Erweckung.), IV.19 (18; Das Nachtwandler-Lied.).

Единого термина «утверждение жизни» тексту приписывать нельзя: есть священное Ja-sagen ребёнка, благословение мира, любовь к жизни и вечности, желание радости и боли. Песнь IV.19 включает вопрос о желании всего, а III.16 — повторяющийся припев любви к вечности; в IV.17 Ja звучит иронически в ослиной литании. Места — отобранная выборка; смысловой просмотр всех употреблений ja, Liebe и segnen не завершён.

### Способность действовать (`agency`)

Слова: Wille, wollen, schaffender Wille, Wollen befreit, That, Thäter, vermögen. Мест: 70. Главы: I.01 (2; Zarathustra's Vorrede.), I.02 (13; Von den drei Verwandlungen.), I.04 (1; Von den Hinterweltlern.), I.05 (1; Von den Verächtern des Leibes.), I.07 (5; Vom bleichen Verbrecher.), I.18 (1; Vom Wege des Schaffenden.), II.02 (8; Auf den glückseligen Inseln.), II.20 (20; Von der Erlösung.), III.05 (1; Von der verkleinernden Tugend.), III.13 (12; Der Genesende.), IV.19 (6; Das Nachtwandler-Lied.).

Отдельного слова, полностью равного современной «способности действовать», нет. О ней судим по воле и творчеству, действию и деятелю, могуществу ребёнка, границам воли перед прошлым и включённости в причинный узел. II.02 исправляет формулу о судьбе; II.20 оставляет вопрос о хотении прошлого открытым. Места — ограниченная смысловая выборка, не полный просмотр всех 645 широких совпадений.

### Сопоставления с другими книгами

Шесть выбранных узлов не называют другого мыслителя или понятие другой книги. Для дальнейшего сопоставления: III.13, 23–26 и 58–60 даёт место для вопроса об изменении и возвращении; III.13, 64 — место для вопроса об участии в причинном порядке; IV.19, 2–5 — о повторном принятии прожитого. Это возможности сравнения, а не свидетельство о Спинозе, Лейбнице, Кьеркегоре или Камю и не доказательство тождества их учений. В `evidence.json` узлы conatus, possible-worlds, repetition и sisyphus не проверены, поэтому пустые пары с ними не являются отрицательными выводами.
