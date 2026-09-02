export type ContentLanguage = "en" | "ru";

const dossierTitleEn: Record<string, string> = {
  A01: "Proto-Cuneiform and Accounting Ontologies",
  A02: "Old Babylonian School Literature, Wisdom, and Law",
  A03: "First-Millennium BCE Mesopotamia as a Node of Omen Science, Astrology, Medicine, and Commentary",
  A04: "Egyptian Sebayt, Ma'at, and Scribal Ethics",
  A05: "Egyptian Mortuary-Theological and Crisis-Dialogic Corpora",
  A06: "Late Egyptian Demotic Temple Scholarship and the Hermetic Background",
  A07: "Elam: Proto-Elamite and the Linear Elamite Frontier",
  A08: "The Elamite Cuneiform Layer and Achaemenid Trilingualism",
  A09: "The Indus Civilization: Signs, Seals, and the Limits of Reconstruction",
  A10: "Hittite State-Ritual Writing",
  A11: "West Asia: Ugarit and the Early Northwest Semitic Alphabetic World",
  A12: "Levantine Wisdom Literature and the Hebrew-Aramaic Documentary World",
  A13: "Second Temple Judea",
  A14: "Shang and Early Zhou",
  A15: "Chunqiu-Zhanguo and the Field of the Hundred Schools",
  A16: "East Asia: Imperial Confucianism from Han to Sui",
  A17: "South Asia: The Vedic-Brahmanical Tradition up to and through the Early Upanishads",
  A18: "South Asia: Shramana Traditions, Early Buddhism, Jainism, and Discipline",
  A19: "South Asia: Classical Brahmanical Shastra and the Darshanas",
  A20: "South Asia: Classical Indian Buddhist Shastra",
  A21: "Theravada and the Written Fixation of the Pali Canon",
  A22: "Ancient Iran, the Avesta, and Achaemenid Royal Inscriptions",
  A23: "The Sasanian Avesta and Pahlavi Scholastic Node",
  A35: "Hermetic, Gnostic, and Mandaean Written Complexes of Late Antiquity",
  A36: "Mani, Manichaeism, and the Multilingual Corpus",
  A37: "Coptic Egypt in Late Antiquity",
  A40: "West Asia: South Arabian, Himyaritic, and North Arabian Epigraphy before Islam",
  A41: "The Central Asian Textual Corridor",
  A42: "East Asia: Early Chinese Buddhism",
  A43: "Early Southeast Asia: Sanskrit and Pali Inscriptions",
};

// Public presentation names for source-owned records that currently carry an
// English descriptive label.  Original-language work titles remain unchanged;
// only catalogue and grouping descriptions are localized here.
const exactRu: Record<string, string> = {
  "Laws of Hammurapi": "Законы Хаммурапи",
  "law collections Ur-Namma / Lipit-Ishtar / Hammurapi":
    "Собрания законов Ур-Наммы, Липит-Иштара и Хаммурапи",
  "Old Babylonian Sumerian literary catalogue witnesses":
    "Свидетельства старовавилонских шумерских литературных каталогов",
  "curricular catalogues of Sumerian literature": "Учебные каталоги шумерской литературы",
  "RSL/RuNEB PDF scan of the Antonovsky 1911 fourth edition":
    "Цифровой скан четвёртого издания перевода Антоновского 1911 года (РГБ/RuNEB)",
};

const exactEn: Record<string, string> = {
  "Таблица I для ToS": "Table I for ToS",
  "Таблица I для ToS.docx": "Table I for ToS.docx",
  "Таблица II для ToS": "Table II for ToS",
  "Таблица II для ToS.docx": "Table II for ToS.docx",
  "Таблица III ToS": "Table III for ToS",
  "Таблица III ToS.docx": "Table III ToS.docx",
  "Западная Азия и Северная Африка": "West Asia and North Africa",
  "Южная, Центральная, Восточная и Юго-Восточная Азия": "South, Central, East, and Southeast Asia",
  "Восточная Азия — Имперское конфуцианство Хань–Суй": "East Asia: Imperial Confucianism from Han to Sui",
  "Восточная Азия — Ранний китайский буддизм": "East Asia: Early Chinese Buddhism",
  "Восточная Азия — Чуньцю–Чжаньго: Ста школ": "East Asia: Chunqiu-Zhanguo and the Hundred Schools",
  "Восточная Азия — Шан и ранний Чжоу: oracle bones, бронзы, политическая теология":
    "East Asia: Shang and Early Zhou: oracle bones, bronzes, and political theology",
  "Вторая храмовая Иудея": "Second Temple Judea",
  "Европа / Египет — Александрийская филология, грамматика и текстуальная критика":
    "Europe / Egypt: Alexandrian Philology, Grammar, and Textual Criticism",
  "Западная Азия / Египет — Второй Храм и иудейско-эллинистическая философская теология":
    "West Asia / Egypt: Second Temple and Judeo-Hellenistic Philosophical Theology",
  "Западная Азия / Центральная Азия / Китай — Манихейский многоязычный корпус":
    "West Asia / Central Asia / China: The Manichaean Multilingual Corpus",
  "Западная Азия — Бронзовая Анатолия: хеттско-лувийские законы, договоры, инструкции, ритуалы":
    "West Asia: Bronze Age Anatolia: Hittite-Luwian Laws, Treaties, Instructions, and Rituals",
  "Западная Азия — Левантская мудростная литература и иврито-арамейский документальный мир":
    "West Asia: Levantine Wisdom Literature and the Hebrew-Aramaic Documentary World",
  "Западная Азия — Месопотамия I тыс. до н.э.: omen-science, астрология, медицина, комментарий":
    "West Asia: First-Millennium BCE Mesopotamia: Omen Science, Astrology, Medicine, and Commentary",
  "Западная Азия — Раббинский иудаизм и цивилизация Талмуда": "West Asia: Rabbinic Judaism and the Civilization of the Talmud",
  "Западная Азия — Ранний Иран: Гаты, Авеста, ахеменидские надписи":
    "West Asia: Early Iran: The Gathas, the Avesta, and Achaemenid Inscriptions",
  "Западная Азия — Сасанидская зороастрийская канонизация и пехлевийская схоластика":
    "West Asia: Sasanian Zoroastrian Canonization and Pahlavi Scholasticism",
  "Западная Азия — Сирийская христианская литературная республика":
    "West Asia: The Syriac Christian Literary Republic",
  "Западная Азия — Скриптурализация Торы, пророков и сектантских текстов Второго Храма":
    "West Asia: Scripturalization of the Torah, Prophets, and Second Temple Sectarian Texts",
  "Западная Азия — Старовавилонская школьная словесность, мудрость и право":
    "West Asia: Old Babylonian School Literature, Wisdom, and Law",
  "Западная Азия — Угарит и ранний северо-западносемитский алфавитный мир":
    "West Asia: Ugarit and the Early Northwest Semitic Alphabetic World",
  "Западная Азия — Элам: proto-Elamite и Linear Elamite frontier":
    "West Asia: Elam: Proto-Elamite and the Linear Elamite Frontier",
  "Западная Азия — Эламский клинописный слой и ахеменидское трехъязычие":
    "West Asia: The Elamite Cuneiform Layer and Achaemenid Trilingualism",
  "Западная Азия — Южная и центральная Месопотамия: прото-клинопись и учётные онтологии":
    "West Asia: Southern and Central Mesopotamia: Proto-Cuneiform and Accounting Ontologies",
  "Западная Азия — Южноаравийская, химьяритская и североаравийская эпиграфика до ислама":
    "West Asia: South Arabian, Himyaritic, and North Arabian Epigraphy before Islam",
  "Коптский Египет между монашеством, переводом и гностико-патристическими хвостами":
    "Coptic Egypt between Monasticism, Translation, and Gnostic-Patristic Afterlives",
  "Манихейский многоязычный корпус": "The Manichaean Multilingual Corpus",
  "Северная Африка / Западная Азия — Герметические, гностические и мандейские пограничные корпуса":
    "North Africa / West Asia: Hermetic, Gnostic, and Mandaean Borderland Corpora",
  "Северная Африка — Египет: sebayt, Маат и писцовая этика": "North Africa: Egypt: Sebayt, Ma'at, and Scribal Ethics",
  "Северная Африка — Египет: погребально-теологические и кризисно-диалогические корпусы":
    "North Africa: Egypt: Mortuary-Theological and Crisis-Dialogic Corpora",
  "Северная Африка — Коптский Египет поздней античности": "North Africa: Coptic Egypt in Late Antiquity",
  "Центральная Азия — Центральноазиатский текстовый коридор": "Central Asia: The Central Asian Textual Corridor",
  "Юго-Восточная Азия — Ранняя Юго-Восточная Азия: санскритские и палийские инскрипции":
    "Southeast Asia: Early Southeast Asia: Sanskrit and Pali Inscriptions",
  "Южная Азия — Ведийско-брахманическая традиция до и через ранние Упанишады":
    "South Asia: The Vedic-Brahmanical Tradition up to and through the Early Upanishads",
  "Южная Азия — Индская цивилизация: знаки, печати и предел реконструкции":
    "South Asia: The Indus Civilization: Signs, Seals, and the Limits of Reconstruction",
  "Южная Азия — Классическая брахманическая шастра и даршаны":
    "South Asia: Classical Brahmanical Shastra and the Darshanas",
  "Южная Азия — Классическая индийская буддийская шастра": "South Asia: Classical Indian Buddhist Shastra",
  "Южная Азия — Тхеравада и письменная фиксация палийского канона":
    "South Asia: Theravada and the Written Fixation of the Pali Canon",
  "Южная Азия — Шраманские традиции: ранний буддизм, джайнизм, дисциплина":
    "South Asia: Shramana Traditions, Early Buddhism, Jainism, and Discipline",
  "Позднеурукский административно-письменный комплекс": "Late Uruk Administrative-Written Complex",
  "Протоклинописный репертуар": "Proto-Cuneiform Repertoire",
  "Глиняная табличка": "Clay Tablet",
  "Административная табличка": "Administrative Tablet",
  "Лексический список": "Lexical List",
  "Учётная онтология": "Accounting Ontology",
  "Метролого-числовая рациональность": "Metrological-Numerical Rationality",
  "Лексическая категоризация": "Lexical Categorization",
  "Старовавилонская писцово-литературная экосистема": "Old Babylonian Scribal-Literary Ecosystem",
  "Шумерский": "Sumerian",
  "Старовавилонский аккадский": "Old Babylonian Akkadian",
  "Клинопись": "Cuneiform",
  "Монументальная стела": "Monumental Stele",
  "Законы Ур-Наммы": "Laws of Ur-Namma",
  "Законы Липит-Иштара": "Laws of Lipit-Ishtar",
  "царская справедливость": "royal justice",
  "Аккадский": "Akkadian",
  "Эламский язык": "Elamite Language",
  "Эламская клинопись": "Elamite Cuneiform",
  "Древнеперсидская клинопись": "Old Persian Cuneiform",
  "Имперское многоязычие": "Imperial Multilingualism",
  "Архив как память государства": "The Archive as State Memory",
  "Индская знаковая экосистема зрелой Хараппы": "The Indus Sign Ecosystem of Mature Harappa",
  "Нерасшифрованность": "Undeciphered State",
  "Хеттский клинописный": "Hittite Cuneiform",
  "Международные договоры": "International Treaties",
};

const phraseEn: [RegExp, string][] = [
  [/^Region:\s*/i, "Region: "],
  [/^Work Or Research Node:\s*/i, "Work or Research Node: "],
  [/^Corpus Or Prepared Source Document:\s*/i, "Corpus or Prepared Source Document: "],
  [/^ToS Deep Research[_:\s]*/i, "ToS Deep Research: "],
  [/\.docx$/i, ".docx"],
  [/до\s*н\.?\s*э\.?/gi, "BCE"],
  [/н\.?\s*э\.?/gi, "CE"],
  [/тысячелетия/gi, "millennium"],
  [/тыс\./gi, "millennium"],
  [/вв\./gi, "centuries"],
  [/в\./gi, "century"],
  [/Западная Азия/gi, "West Asia"],
  [/Северная Африка/gi, "North Africa"],
  [/Южная Азия/gi, "South Asia"],
  [/Центральная Азия/gi, "Central Asia"],
  [/Восточная Азия/gi, "East Asia"],
  [/Юго-Восточная Азия/gi, "Southeast Asia"],
  [/Египетский/gi, "Egyptian"],
  [/Египетско-левантийский/gi, "Egyptian-Levantine"],
  [/Египет/gi, "Egypt"],
  [/поздний/gi, "late"],
  [/ранний/gi, "early"],
  [/ранняя/gi, "early"],
  [/многоязычный/gi, "multilingual"],
  [/многоязычие/gi, "multilingualism"],
  [/письменная фиксация/gi, "written fixation"],
  [/письменный/gi, "written"],
  [/писцовая этика/gi, "scribal ethics"],
  [/писцово-литературная/gi, "scribal-literary"],
  [/школьная словесность/gi, "school literature"],
  [/мудрость/gi, "wisdom"],
  [/право/gi, "law"],
  [/закон/gi, "law"],
  [/ритуал/gi, "ritual"],
  [/храмовая ученость/gi, "temple scholarship"],
  [/ученость/gi, "scholarship"],
  [/комментарий/gi, "commentary"],
  [/корпусы/gi, "corpora"],
  [/корпус/gi, "corpus"],
  [/знаки/gi, "signs"],
  [/печати/gi, "seals"],
  [/предел реконструкции/gi, "limits of reconstruction"],
  [/клинописный слой/gi, "cuneiform layer"],
  [/клинопись/gi, "cuneiform"],
  [/трехъязычие/gi, "trilingualism"],
  [/царские надписи/gi, "royal inscriptions"],
  [/надписи/gi, "inscriptions"],
  [/эпиграфика/gi, "epigraphy"],
  [/санскритские/gi, "Sanskrit"],
  [/палийские/gi, "Pali"],
  [/буддизм/gi, "Buddhism"],
  [/джайнизм/gi, "Jainism"],
  [/дисциплина/gi, "discipline"],
  [/шастра/gi, "shastra"],
  [/даршаны/gi, "darshanas"],
  [/канон/gi, "canon"],
  [/Авеста/gi, "Avesta"],
  [/ахеменидские/gi, "Achaemenid"],
  [/центральноазиатский текстовый коридор/gi, "Central Asian textual corridor"],
  [/Маат/gi, "Ma'at"],
  [/Мани/gi, "Mani"],
  [/манихейство/gi, "Manichaeism"],
  [/Коптский/gi, "Coptic"],
  [/Эламский/gi, "Elamite"],
  [/Элам/gi, "Elam"],
  [/Индская цивилизация/gi, "Indus Civilization"],
  [/Хеттское/gi, "Hittite"],
  [/Хеттский/gi, "Hittite"],
  [/Левантская/gi, "Levantine"],
  [/иврито-арамейский/gi, "Hebrew-Aramaic"],
  [/документальный мир/gi, "documentary world"],
  [/Вторая храмовая Иудея/gi, "Second Temple Judea"],
  [/Шан/gi, "Shang"],
  [/ранний Чжоу/gi, "Early Zhou"],
  [/Ста школ/gi, "Hundred Schools"],
  [/Имперское конфуцианство/gi, "Imperial Confucianism"],
  [/Ведийско-брахманическая традиция/gi, "Vedic-Brahmanical tradition"],
  [/ранние Упанишады/gi, "Early Upanishads"],
  [/Шраманские традиции/gi, "Shramana traditions"],
  [/Тхеравада/gi, "Theravada"],
  [/палийского канона/gi, "Pali Canon"],
  [/Древний Иран/gi, "Ancient Iran"],
  [/Герметические/gi, "Hermetic"],
  [/гностические/gi, "Gnostic"],
  [/мандеистские/gi, "Mandaean"],
  [/поздней античности/gi, "Late Antiquity"],
  [/до ислама/gi, "before Islam"],
  [/(^|[\s/—-])и(?=$|[\s/—-])/gi, "$1and"],
  [/(^|[\s/—-])как(?=$|[\s/—-])/gi, "$1as"],
];

function trimDocx(value: string): { body: string; docx: boolean } {
  const docx = /\.docx$/i.test(value);
  return { body: docx ? value.replace(/\.docx$/i, "") : value, docx };
}

function dossierContentTitle(value: string): string | null {
  const { body, docx } = trimDocx(value.trim());
  const match = body.match(/\b(A\d{2})\b/);
  if (!match) return null;
  const title = dossierTitleEn[match[1]];
  if (!title) return null;
  const prefix = body.match(/^Corpus Or Prepared Source Document:/i)
    ? "Corpus or Prepared Source Document: "
    : body.match(/^ToS Deep Research/i)
      ? "ToS Deep Research: "
      : "";
  return `${prefix}${match[1]} — ${title}${docx ? ".docx" : ""}`;
}

function prefixedContentTitle(value: string): string | null {
  const match = value.match(/^(Corpus Or Prepared Source Document|Region|Work Or Research Node):\s*(.+)$/i);
  if (!match) return null;
  const prefix =
    match[1].toLowerCase() === "region"
      ? "Region"
      : match[1].toLowerCase() === "work or research node"
        ? "Work or Research Node"
        : "Corpus or Prepared Source Document";
  return `${prefix}: ${localizedContentText(match[2], "en")}`;
}

export function localizedContentText(value: unknown, language: ContentLanguage): string {
  const raw = String(value ?? "");
  if (!raw) return raw;
  if (language === "ru") return exactRu[raw.trim()] || raw;
  const prefixed = prefixedContentTitle(raw);
  if (prefixed) return prefixed;
  const exact = exactEn[raw.trim()];
  if (exact) return exact;
  const dossier = dossierContentTitle(raw);
  if (dossier) return dossier;
  if (!/[А-Яа-яЁё]/.test(raw)) return raw;
  let translated = raw;
  for (const [pattern, replacement] of phraseEn) translated = translated.replace(pattern, replacement);
  translated = translated.replace(/\s+—\s+/g, ": ");
  translated = translated.replace(/\s+/g, " ").trim();
  return translated;
}

export function localizedContentPayload(value: unknown, language: ContentLanguage): unknown {
  if (language === "ru") return value;
  if (typeof value === "string") return localizedContentText(value, language);
  if (Array.isArray(value)) return value.map((item) => localizedContentPayload(item, language));
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, localizedContentPayload(item, language)]));
  }
  return value;
}

