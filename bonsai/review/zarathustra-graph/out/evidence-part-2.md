# Заратустра: основание, часть 2

Источник: немецкие первые издания I–III (1883–1884) и первое публичное издание IV (1891), транскрипции Deutsches Textarchiv / BBAW, оригиналы Staatsbibliothek zu Berlin и SUB Göttingen, CC BY-SA 4.0; состав и ссылки — `tos/bonsai/zarathustra/witnesses.json`. Единицы и порядок — немецкий слой 81 главы. Скрипт `build-evidence-part-2.py` выполняет расширенный поиск и хранит смысловой отбор. Переносы DTA с ¬ соединены; контексты ограничены 30 словами. Говорящий `null` означает, что он не установлен в этой разметке.

Набор мест — рабочая смысловая выборка, а не доказательство полноты всех непрямых выражений. Буквальные кандидаты созидания проверены по контексту; по широким шаблонам остальных узлов просмотрены релевантные формулировки и сцены, но не проведена построчная смысловая проверка каждого общего слова вроде Leben, wollen и Liebe. Поэтому этот шаг остаётся незавершённым в части требования исчерпывающего просмотра. `forms` — наблюдённые формы в отобранных единицах, а не все возможные словоизменения; некоторые обычные слова пропущены как не относящиеся к узлу. Пары вычислены по этой выборке и существующему `evidence.json`: `near` — только непосредственно соседние единицы в одной главе. Старые 14 узлов частичны, а остальные в нём пусты: пустая пара не свидетельствует об отсутствии сцены или отношения.

## creation

Слова: schaffen, Schöpfer, Schöpfung, umschaffen, erschaffen, Geschöpf, Mitschaffender. Мест: **111**. Главы: I.01 (9; Zarathustra's Vorrede.); I.02 (4; Von den drei Verwandlungen.); I.04 (8; Von den Hinterweltlern.); I.05 (4; Von den Verächtern des Leibes.); I.08 (1; Vom Lesen und Schreiben.); I.09 (1; Vom Baum am Berge.); I.12 (1; Vom neuen Götzen.); I.13 (1; Von den Fliegen des Marktes.); I.16 (7; Von tausend und Einem Ziele.); I.17 (2; Von der Nächstenliebe.); I.18 (4; Vom Wege des Schaffenden.); I.21 (4; Von Kind und Ehe.); I.23 (2; Von der schenkenden Tugend.); II.01 (1; Das Kind mit dem Spiegel.); II.02 (12; Auf den glückseligen Inseln.); II.03 (3; Von den Mitleidigen.); II.04 (1; Von den Priestern.); II.06 (1; Vom Gesindel.); II.09 (1; Das Nachtlied.); II.12 (4; Von der Selbst-Ueberwindung.); II.14 (1; Vom Lande der Bildung.); II.15 (3; Von der unbefleckten Erkenntniss.); II.20 (5; Von der Erlösung.); III.03 (2; Von der Seligkeit wider Willen.); III.12 (12; Von alten und neuen Tafeln.); III.13 (2; Der Genesende.); III.14 (1; Von der grossen Sehnsucht.); III.16 (3; Die sieben Siegel.); IV.06 (1; Ausser Dienst.); IV.07 (1; Der hässlichste Mensch.); IV.13 (6; Vom höheren Menschen.); IV.15 (1; Von der Wissenschaft.); IV.17 (2; Die Erweckung.).

ЧАСТИЧНО: Творение, созидание ценностей, мира и себя, включая переоформление прошлого. Учитываются также божественное и вымышленное творение как тематические варианты; обыденное «Recht schaffen», «also schafft es», Schuft и abschaffen исключены. Формы — обнаруженные в отобранных единицах, не исчерпывающая парадигма; список не доказывает полноты косвенных обозначений. Полный построчный смысловой обзор всех широких поисковых совпадений не выполнен; пустые места не доказывают отсутствия.

## memory

Слова: Erinnerung, Gedächtniss, gedenken, vergessen, wiedererkennen, zurücklaufen. Мест: **44**. Главы: I.01 (3; Zarathustra's Vorrede.); I.02 (1; Von den drei Verwandlungen.); I.03 (1; Von den Lehrstühlen der Tugend.); I.10 (1; Von den Predigern des Todes.); I.19 (1; Von alten und jungen Weiblein.); I.20 (1; Vom Biss der Natter.); II.03 (1; Von den Mitleidigen.); II.05 (1; Von den Tugendhaften.); II.07 (1; Von den Taranteln.); II.10 (1; Das Tanzlied.); II.11 (3; Das Grablied.); II.17 (1; Von den Dichtern.); III.01 (3; Der Wanderer.); III.02 (5; Vom Gesicht und Räthsel.); III.09 (1; Die Heimkehr.); III.12 (2; Von alten und neuen Tafeln.); III.13 (2; Der Genesende.); IV.02 (1; Der Nothschrei.); IV.06 (2; Ausser Dienst.); IV.07 (1; Der hässlichste Mensch.); IV.08 (1; Der freiwillige Bettler.); IV.10 (1; Mittags.); IV.11 (1; Die Begrüssung.); IV.12 (1; Das Abendmahl.); IV.14 (1; Das Lied der Schwermuth.); IV.16 (2; Unter Töchtern der Wüste.); IV.18 (2; Das Eselsfest.); IV.20 (2; Das Zeichen.).

ЧАСТИЧНО: Память и воспоминание, включая забывание и узнавание. III.02:36–40 — воспоминание детского воя без существительного Erinnerung; единицы сцены включены по смыслу. Исключены обычные наставления не забыть конкретный предмет и безличное vergessen как «заброшенный». Сцена узнавания не доказывает повторение целой жизни; связи с памятью у Кьеркегора и Камю сравнительны, а не цитаты или тождество. Полный построчный смысловой обзор всех широких поисковых совпадений не выполнен; пустые места не доказывают отсутствия.

## difference

Слова: Unterschied, verschieden, Ungleichheit, Ungleiches, Gleichheit, anders, anderes, gleich und selbig. Мест: **26**. Главы: I.01 (1; Zarathustra's Vorrede.); I.07 (1; Vom bleichen Verbrecher.); II.07 (11; Von den Taranteln.); III.09 (2; Die Heimkehr.); III.12 (1; Von alten und neuen Tafeln.); III.13 (5; Der Genesende.); IV.08 (1; Der freiwillige Bettler.); IV.11 (1; Die Begrüssung.); IV.15 (3; Von der Wissenschaft.).

ЧАСТИЧНО: Выборка явного различения, неравенства и тождества в вопросе различия; не всякое другое лицо, anders или сравнение с gleich есть философское различие. III.13:65–66 — отрицание новой или похожей жизни и утверждение той же; это сопоставимо с вопросом Делёза, но не его теория различия. Полный построчный смысловой обзор всех широких поисковых совпадений не выполнен; пустые места не доказывают отсутствия.

## amor-fati

Слова: Schicksal, Loos, segnen, Ja-sagen, wollen, Zufall, Ewigkeit. Мест: **71**. Главы: I.23 (1; Von der schenkenden Tugend.); II.02 (1; Auf den glückseligen Inseln.); II.16 (1; Von den Gelehrten.); II.20 (7; Von der Erlösung.); III.01 (2; Der Wanderer.); III.04 (9; Vor Sonnen-Aufgang.); III.05 (2; Von der verkleinernden Tugend.); III.12 (3; Von alten und neuen Tafeln.); III.13 (11; Der Genesende.); III.16 (21; Die sieben Siegel.); IV.01 (2; Das Honig-Opfer.); IV.19 (11; Das Nachtwandler-Lied.).

ЧАСТИЧНО: Само название amor fati принадлежит §276 «Весёлой науки»; здесь его не приписываем книге. Отобраны сцены желаемой судьбы, благословения случая, творческого отношения к прошлому и утверждения возвращения вместе со страданием; они сопоставимы с любовью к судьбе, но не равны формуле §276. В III.13:64–69 речь вложена животными в возможные слова Заратустры. Полный построчный смысловой обзор всех широких поисковых совпадений не выполнен; пустые места не доказывают отсутствия.

## providence

Слова: Vorsehung, ordnender Finger Gottes, Zweck, Zufall, Ohngefähr, ewiger Wille. Мест: **15**. Главы: I.12 (1; Vom neuen Götzen.); I.23 (1; Von der schenkenden Tugend.); II.21 (1; Von der Menschen-Klugheit.); III.04 (10; Vor Sonnen-Aufgang.); III.12 (1; Von alten und neuen Tafeln.); IV.13 (1; Vom höheren Menschen.).

ЧАСТИЧНО: Сравнение с промыслом и порядком Марка Аврелия: Vorsehung буквально дважды, о собственной судьбе и заботе беременной, не утверждение космического промысла. В I.12 богоподобный порядок — притязание государства; III.04:25–28 прямо противопоставляет миру случая вечную волю и господство цели. Аналогичный предмет — порядок целого, но утверждение божественного попечения здесь не установлено. Полный построчный смысловой обзор всех широких поисковых совпадений не выполнен; пустые места не доказывают отсутствия.

## conatus

Слова: sich erhalten, Dasein, Wille zum Dasein, Wille zum Leben, Wille zur Macht, sich überwinden, Trieb. Мест: **45**. Главы: I.01 (3; Zarathustra's Vorrede.); I.05 (5; Von den Verächtern des Leibes.); I.09 (2; Vom Baum am Berge.); I.16 (3; Von tausend und Einem Ziele.); II.07 (6; Von den Taranteln.); II.12 (19; Von der Selbst-Ueberwindung.); III.12 (2; Von alten und neuen Tafeln.); IV.13 (1; Vom höheren Menschen.); IV.19 (4; Das Nachtwandler-Lied.).

ЧАСТИЧНО: Сопоставление с conatus Спинозы: сохранение народа и человека названо в I.16; в II.12:32–35 сама жизнь отвергает Wille zum Dasein и Wille zum Leben, утверждая Wille zur Macht, а II.12:23–28 допускает риск и гибель ради власти. Это ответ на близкий предмет, не доказательство прямой полемики со Спинозой или тождества понятий. Полный построчный смысловой обзор всех широких поисковых совпадений не выполнен; пустые места не доказывают отсутствия.

Пары: 165; из них с совместными или соседними единицами: 30.

Число найденных скриптом единиц до смыслового отбора: creation — 120, memory — 70, difference — 283, amor-fati — 879, providence — 30, conatus — 476. Широкие шаблоны служат поиску кандидатов, а не определениями.
