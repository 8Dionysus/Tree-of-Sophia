import { NODE_CONTENT } from './atlas-content.mjs';
import { RESEARCH_ROUTES } from './atlas-routes.mjs';
import { bindInquiryLayer } from './inquiry-layer.mjs';
import { INQUIRY_FOUNDATION, LENS_INQUIRY } from './inquiry-foundation.mjs';
import { PRIMARY_SOURCES } from './source-references.mjs';

/**
 * A prepared, authored demonstration atlas. These annotations and comparisons
 * are not quotations, source admission, canon, or evidence of historical influence.
 * Original library nodes are retained; source text comes only from that library.
 */
const bi = (ru, en) => ({ ru, en });
const demoId = (slug) => 'demo:' + slug;
const sourceIds = ['work', 'chapter-p3.r2', 'moment', 'chapter-p3.r13', 'all-things', 'same-life'];
const clusterData = [
  ['sources', 'Источники', 'Sources', 'Текст, голоса и соседние строки', 'Text, voices and surrounding passages', '#c5ba92', [-300, -155, -35]],
  ['images', 'Образы', 'Images', 'Ворота, круг, укус и смех', 'Gateway, circle, bite and laughter', '#d99b6c', [0, -165, 25]],
  ['concepts', 'Понятия', 'Concepts', 'Время, необходимость и утверждение', 'Time, necessity and affirmation', '#b6abd8', [290, -120, -10]],
  ['tensions', 'Вопросы', 'Questions', 'Места, где мысль сопротивляется', 'Where thought meets resistance', '#d4899d', [0, 130, -15]],
];
const sourceNote = bi(
  'Демонстрационный материал: авторское толкование, пересказ или сопоставление для макета. Не цитата и не принятый источник или канон ToS.',
  'Demonstration material: an authored interpretation, paraphrase or comparison for this mockup. Not a quotation or an admitted ToS source or canon.',
);


function makeNodes() {
  // Node identity and placement live here; authored prose lives in NODE_CONTENT.
  const rows = [
    ["gateway","symbol","images","Ворота «Мгновенье»","The gateway “Moment”",[]],
    ["two-paths","symbol","images","Две дороги","Two paths",[]],
    ["dwarf","character","images","Карлик у ворот","The dwarf at the gateway",[]],
    ["circle","symbol","images","Круг времени","The circle of time",[]],
    ["shepherd","character","images","Пастух и змея","The shepherd and the serpent",[]],
    ["bite","symbol","images","Укус","The bite",[]],
    ["laughter","symbol","images","Преображённый смех","Transformed laughter",[]],
    ["present","concept","concepts","Настоящий миг","The present moment",[]],
    ["paths-opposition","question","tensions","Будут ли две дороги вечно противоречить друг другу?","Will the two paths contradict each other forever?",[]],
    ["moment-returns","question","tensions","Должны ли это мгновение и мы вернуться?","Must this moment and we return?",[]],
    ["shepherd-riddle","question","tensions","Что означает видение пастуха?","What does the shepherd’s vision mean?",[]],
    ["animals","character","images","Звери и их песня","The animals and their song",[]],
    ["recurrence","concept","concepts","Вечное возвращение","Eternal recurrence",[]],
    ["becoming","concept","concepts","Становление","Becoming",[]],
    ["identity","concept","concepts","Тождество","Identity",[]],
    ["wheel","symbol","images","Колесо","The wheel",[]],
    ["ring","symbol","images","Кольцо бытия и кольцо возвращения","The ring of being and the ring of recurrence",[]],
    ["nausea","symbol","images","Отвращение","Nausea",[]],
    ["small-human","concept","concepts","Возвращение малого человека","The small human’s return",[]],
    ["body-resistance","excerpt","sources","Болезнь Заратустры","Zarathustra’s illness",[]],
    ["fate","concept","concepts","Судьба воли и учителя","The fate of the will and the teacher",[]],
    ["task","concept","concepts","Учительство вечного возвращения","Teaching eternal recurrence",[]],
    ["nausea-return","question","tensions","Что вызывает отвращение Заратустры?","What causes Zarathustra’s disgust?",[]],
    ["same-life-return","question","tensions","К какой жизни возвращаются?","To what life does one return?",[]],
    ["circle-change","question","tensions","Как возвращение соотносится с изменением?","How does recurrence relate to change?",[]],
    ["local-return","excerpt","sources","Возвращение к людям","Returning to people",[]],
    ["creation","concept","concepts","Созидание","Creation",[]],
    ["affirmation","concept","concepts","«Да» радости и горю","Saying yes to joy and sorrow",[]],
    ["joy-pain","concept","concepts","Радость и всё горе","Joy and all sorrow",[]],
    ["dance","symbol","images","Танец","Dance",[]],
    ["whole-part","question","tensions","Как из «да» одной радости возникает желание возвращения всего?","How does a yes to one joy become a desire for the return of everything?",[]],
    ["will-fate","question","tensions","Как творящая воля Заратустры относится к судьбе его пути?","How does Zarathustra’s creative will relate to the fate of his path?",[]],
  ];
  return rows.map(([slug, kind, cluster, ru, en, sourceRefs]) => ({ id: demoId(slug), kind, parentId: null, title: bi(ru, en), demo: true, cluster, sourceRefs, sourceNote }));
}

const resolveId = (id) => sourceIds.includes(id) ? id : demoId(id);
function makeEdges() {
  const rows = [
    ["moment","gateway","supports","Ворота получают имя","The gateway is named","Заратустра показывает карлику ворота и читает их имя над двумя дорогами.","Zarathustra points out the gateway to the dwarf and reads its name above the two paths."],
    ["moment","two-paths","supports","Две дороги сходятся","Two paths meet","В названном месте Заратустра показывает дороги, уходящие в противоположные вечности.","At the named place Zarathustra points out paths running into opposite eternities."],
    ["chapter-p3.r2","dwarf","supports","Карлик отвечает Заратустре","The dwarf replies","На подъёме карлик говорит о падении; у ворот он отвечает на вопрос о дорогах.","On the ascent the dwarf speaks of falling; at the gateway he answers the question about the paths."],
    ["chapter-p3.r2","shepherd","supports","Видение показывает пастуха","The vision shows the shepherd","После воя собаки Заратустра оказывается среди скал и видит пастуха со змеёй в горле.","After the dog’s howl, Zarathustra finds himself among rocks and sees the shepherd with a serpent in his throat."],
    ["shepherd","bite","develops","Пастух следует крику","The shepherd follows the cry","Заратустра тщетно тянет змею и кричит; пастух откусывает ей голову.","Zarathustra pulls at the serpent in vain and cries out; the shepherd bites off its head."],
    ["bite","laughter","develops","Укус сменяется смехом","Biting gives way to laughter","Выплюнув голову змеи, пастух вскакивает; рассказчик видит преображённого смеющегося.","After spitting out the serpent’s head, the shepherd leaps up; the narrator sees someone transformed and laughing."],
    ["dwarf","circle","supports","Карлик называет время кругом","The dwarf names a circle","На вопрос о дорогах карлик отвечает, что время — круг. Заратустра возражает против лёгкости ответа.","Asked about the paths, the dwarf calls time a circle. Zarathustra objects to the ease of his answer."],
    ["gateway","two-paths","supports","Дороги встречаются у ворот","Paths meet at the gateway","Заратустра показывает карлику две дороги и называет место их встречи.","Zarathustra points out the two paths to the dwarf and names their meeting place."],
    ["dwarf","gateway","supports","Карлик садится у ворот","The dwarf sits down","Карлик спрыгивает с плеча Заратустры и садится на камень там, где находятся ворота.","The dwarf jumps from Zarathustra’s shoulder and sits on a stone where the gateway stands."],
    ["gateway","recurrence","supports","Заратустра спрашивает о возвращении","Zarathustra asks about return","Заратустра спрашивает, были ли прежде ворота и собеседники и должны ли они вечно возвращаться.","He asks whether the gateway and the speakers have been before and must return."],
    ["two-paths","circle","supports","Карлик отвечает кругом","The dwarf answers: a circle","Заратустра спрашивает о вечном противоречии дорог; карлик отвечает, что время — круг.","Zarathustra asks whether the paths contradict each other forever; the dwarf replies that time is a circle."],
    ["shepherd","laughter","develops","Пастух смеётся преображённым","The shepherd laughs transformed","Пастух кусает змею и вскакивает; рассказчик видит смеющегося преображённого.","The shepherd bites the serpent and leaps up; the narrator sees a transformed figure laughing."],
    ["present","recurrence","supports","Мгновение входит в вопрос","The moment enters the question","Заратустра спрашивает, влечёт ли это мгновение само себя и должны ли вернуться собеседники.","Zarathustra asks whether this moment draws itself after it and whether the speakers must return."],
    ["two-paths","paths-opposition","questions","Заратустра спрашивает о дорогах","Zarathustra questions the paths","Он спрашивает, будут ли дороги вечно противоречить друг другу; карлик предлагает свой ответ.","He asks whether the paths will contradict each other forever; the dwarf offers an answer."],
    ["gateway","moment-returns","questions","Ворота входят в вопрос","The gateway enters the question","Заратустра спрашивает, существовали ли уже эти ворота и должны ли вернуться собеседники.","Zarathustra asks whether this gateway has already existed and whether the speakers must return."],
    ["shepherd","shepherd-riddle","questions","Заратустра спрашивает о пастухе","Zarathustra asks about the shepherd","Он просит слушателей разгадать видение и спрашивает, кто пастух со змеёй в горле.","He asks his listeners to interpret the vision and say who the shepherd with the serpent in his throat is."],
    ["chapter-p3.r13","animals","supports","Звери остаются рядом","The animals stay","Звери остаются с больным Заратустрой; когда он приподнимается, они начинают разговор.","The animals stay beside the sick Zarathustra; when he sits up, they begin to speak."],
    ["chapter-p3.r13","nausea","supports","Заратустра вспоминает отвращение","Zarathustra recalls disgust","После крика и падения он позднее рассказывает, что его душило, и вспоминает болезнь.","After his cry and collapse, he later describes what choked him and remembers the illness."],
    ["all-things","recurrence","supports","Звери поют возвращение","The animals sing recurrence","В песне вещи уходят и приходят снова; вместе с ними катится колесо бытия.","In the song things depart and return; the wheel of being rolls with them."],
    ["same-life","identity","supports","Звери воображают ту же жизнь","They imagine the same life","Звери вкладывают в воображаемую речь Заратустры возвращение к этой же жизни с теми же спутниками.","The animals imagine Zarathustra speaking of a return to this very life with the same companions."],
    ["all-things","animals","supports","Песня получает ответ","The song receives a reply","Звери поют о вещах; Заратустра улыбается и спрашивает, не сделали ли они из его испытания шарманочную песню.","The animals sing of things; Zarathustra smiles and asks whether they have turned his ordeal into a barrel-organ tune."],
    ["wheel","recurrence","supports","Колесо катится вечно","The wheel rolls eternally","В песне зверей слова о возвращении всего стоят рядом с колесом бытия.","In the animals’ song the return of all things stands beside the wheel of being."],
    ["nausea","small-human","supports","Возвращается даже малейший","Even the smallest returns","Заратустра связывает своё отвращение ко всему существованию с вечным возвращением даже самого малого человека.","Zarathustra connects his disgust at all existence with the eternal return of even the smallest human."],
    ["recurrence","becoming","supports","Повторяется год становления","A year of becoming recurs","Звери поют о смерти и новом расцвете, а затем говорят об одинаковых великих годах становления.","The animals sing of death and renewed blossoming, then speak of alike great years of becoming."],
    ["nausea","body-resistance","supports","Крик предшествует болезни","The cry precedes illness","После крика отвращения Заратустра падает и семь дней лежит; позднее его рассказ возвращается к болезни.","After crying out in disgust Zarathustra collapses and lies for seven days; his later account returns to the illness."],
    ["animals","wheel","supports","Звери поют о колесе","The animals sing the wheel","Звери помещают в свою песню вечно катящееся колесо бытия рядом с возвращением вещей.","The animals place the eternally rolling wheel of being beside the return of things in their song."],
    ["animals","recurrence","supports","Звери называют учителя","The animals name the teacher","После болезни звери называют Заратустру учителем вечного возвращения и излагают, чему, по их словам, он учит.","After the illness the animals name Zarathustra the teacher of eternal recurrence and state what they say he teaches."],
    ["animals","ring","supports","Звери поют о кольце","The animals sing the ring","В песне зверей кольцо бытия следует за распадом, соединением и новой встречей.","In the animals’ song the ring of being follows breaking apart, rejoining and meeting again."],
    ["ring","recurrence","supports","Песня называет кольцо","The song names the ring","В «Семи печатях» говорящий называет обручальное кольцо кольцом возвращения и обращается к вечности.","In “The Seven Seals” the speaker calls the wedding ring the ring of recurrence and addresses eternity."],
    ["nausea","recurrence","supports","Отвращение встречает возвращение","Disgust meets recurrence","Заратустра рассказывает, как мысль о вечном возвращении малого человека стала для него отвращением ко всему существованию.","Zarathustra recounts how the thought of the small human’s eternal return became disgust at all existence."],
    ["recurrence","identity","supports","Звери воображают ту же жизнь","They imagine the same life","В воображаемой зверями речи возвращение ведёт к той же жизни, с этой землёй и этими спутниками.","In the speech imagined by the animals, recurrence leads to the same life, with this earth and these companions."],
    ["recurrence","fate","supports","Учительство названо судьбой","Teaching is named fate","Звери называют Заратустру учителем вечного возвращения и говорят, что это его судьба.","The animals call Zarathustra the teacher of eternal recurrence and say that this is his fate."],
    ["fate","task","supports","Судьба требует учить","Fate calls for teaching","Звери называют судьбой Заратустры то, что он первым должен учить возвращению, и спрашивают об опасности этой судьбы.","The animals name it Zarathustra’s fate to be the first to teach recurrence and ask about the danger of that fate."],
    ["nausea","nausea-return","questions","Отвращение ищет причину","Disgust seeks its cause","Заратустра вспоминает своё отвращение и связывает его с возвращением даже самого малого человека.","Zarathustra recalls his disgust and connects it with the return of even the smallest human."],
    ["same-life","same-life-return","questions","Звери уточняют жизнь","The animals specify life","Звери воображают, как Заратустра противопоставляет новой, лучшей или похожей жизни ту же самую жизнь.","The animals imagine Zarathustra contrasting a new, better or similar life with the very same life."],
    ["recurrence","circle-change","questions","Песня соединяет перемены","The song joins changes","В песне зверей возвращение стоит рядом с умиранием и расцветом, распадом и новым соединением.","In the animals’ song recurrence stands beside dying and blossoming, breaking apart and rejoining."],
    ["work","local-return","supports","Избыток мудрости ведёт к людям","Abundant wisdom leads to people","Подобно солнцу, отдающему свой избыток, Заратустра хочет раздать мудрость и для этого нисходит к людям.","Like the sun giving its abundance, Zarathustra wants to share his wisdom and descends to people for that purpose."],
    ["affirmation","joy-pain","supports","«Да» радости утверждает горе","A yes to joy affirms sorrow","Заратустра связывает «да» одной радости со всем горем: все вещи сцеплены и переплетены.","Zarathustra joins a yes to one joy to all sorrow: all things are chained and entwined."],
    ["creation","fate","develops","Творящая воля хочет такой судьбы","The creative will wants such a fate","Вспомнив свои прощания, Заратустра уточняет: его творящая воля хочет именно такой судьбы.","Recalling his farewells, Zarathustra makes his meaning more precise: his creative will wants precisely such a fate."],
    ["creation","becoming","supports","Творение оправдывает становление","Creation justifies becoming","Заратустра славит становление и преходящее; творящий проходит через боль и превращение и оправдывает преходящее.","Zarathustra praises becoming and transience; the creator undergoes pain and transformation and justifies transience."],
    ["affirmation","recurrence","supports","«Да» мгновению желает возвращения всего","A yes to the moment wants the return of all","Желание пережить мгновение дважды становится в песне желанием возвращения всех связанных вещей.","In the song, the wish to experience a moment twice becomes a wish for all linked things to return."],
    ["joy-pain","recurrence","supports","Желание вечности зовёт горе вернуться","The desire for eternity calls sorrow back","Радость желает вечности; в песне к горю обращаются: «Пройди, но вернись!»","Joy wants eternity; in the song sorrow is addressed: “Pass away, but come back!”"],
    ["dance","ring","supports","Добродетель танцора ведёт к кольцу","The dancer’s virtue leads to the ring","В шестой части песни добродетель танцора и превращение тяжёлого в лёгкое ведут к припеву о кольце возвращения.","In the song’s sixth movement, the dancer’s virtue and the becoming light of what is heavy lead to the refrain about the ring of recurrence."],
    ["joy-pain","whole-part","questions","Как одна радость ведёт ко всему?","How does one joy lead to all?","Песня связывает одну радость со всем горем и возвращением всего: как связанность вещей соединяет мгновение с целым?","The song joins one joy to all sorrow and the return of everything: how does the linkage of things join the moment to the whole?"],
    ["creation","will-fate","questions","Как воля хочет своей судьбы?","How does the will want its fate?","Вспомнив прощания, Заратустра говорит, что его воля хочет именно такой судьбы; как воля относится к пережитому пути?","After recalling his farewells, Zarathustra says his will wants precisely such a fate; how does the will relate to the path he has lived?"],
  ];
  return rows.map(([from, to, kind, ru, en, ruBody, enBody]) => ({
    id: 'atlas:' + from + ':' + kind + ':' + to,
    from: resolveId(from), to: resolveId(to), kind,
    label: bi(ru, en), body: bi(ruBody, enBody), demo: true,
  }));
}

// Branches to grow are held until they are grown together; see atlas-held.mjs.
const extensionDefinitions = [];

function makeAssemblies(edges) {
  const definitions = [
    ["gateway-scene","У ворот «Мгновение»","At the gateway “Moment”","Дороги, карлик и вопрос о возвращении","The paths, the dwarf and the question of return","Что происходит с двумя дорогами в разговоре у ворот?","What happens to the two paths in the conversation at the gateway?",["chapter-p3.r2","moment","gateway","two-paths","dwarf","circle","present","paths-opposition","moment-returns"]],
    ["shepherd-vision","Видение пастуха","The shepherd’s vision","Змея, крик, укус и смех","The serpent, the cry, the bite and the laughter","Кто пастух и что происходит после укуса?","Who is the shepherd, and what happens after the bite?",["chapter-p3.r2","shepherd","bite","laughter","shepherd-riddle"]],
    ["convalescent-voices","Заратустра и его звери","Zarathustra and his animals","Болезнь, песня и судьба учителя","Illness, song and the teacher’s fate","Как песня зверей встречается с пережитым Заратустрой?","How does the animals’ song meet Zarathustra’s experience?",["chapter-p3.r13","body-resistance","nausea","small-human","nausea-return","all-things","animals","wheel","ring","becoming","recurrence","same-life","identity","same-life-return","circle-change","fate","task"]],
    ["joy-sorrow-song","Песнь о радости и горе","The song of joy and sorrow","Одно «да», связанность вещей и вечность","One yes, the connectedness of things and eternity","Как «да» одной радости ведёт к желанию возвращения всего?","How does a yes to one joy lead to wanting everything to return?",["affirmation","joy-pain","recurrence","whole-part"]],
  ];
  return definitions.map(([id, ru, en, ruSub, enSub, ruQuestion, enQuestion, slugs]) => {
    const nodeIds = slugs.map(resolveId);
    const included = new Set(nodeIds);
    return { id, title: bi(ru, en), subtitle: bi(ruSub, enSub), question: bi(ruQuestion, enQuestion), nodeIds, edgeIds: edges.filter((edge) => included.has(edge.from) && included.has(edge.to)).map((edge) => edge.id) };
  });
}

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  return value;
}

/** Preserve the supplied library and add the prepared atlas without mutating it. */
export async function createAtlasLibrary(sourceLibrary) {
  const sourceNodes = sourceLibrary.nodes ?? [];
  const available = new Set(sourceNodes.map((node) => node.id));
  for (const id of sourceIds) {
    if (!available.has(id)) throw new Error('Atlas requires source material: ' + id);
  }
  const clusters = clusterData.map(([id, ru, en, ruSub, enSub, color, center]) => ({ id, title: bi(ru, en), subtitle: bi(ruSub, enSub), color, center: [...center] }));
  const preparedNodes = makeNodes(), preparedEdges = makeEdges();
  const inquiry = bindInquiryLayer([...sourceIds, ...preparedNodes.map(node => node.id)], preparedEdges.map(edge => edge.id));
  const readingFor = id => {
    const reading = NODE_CONTENT[id];
    if (!reading) throw new Error('Missing authored atlas reading: ' + id);
    return { inquiry: inquiry.material(id), perspectives: reading.perspectives, ...(reading.example ? { example: reading.example } : {}), ...(reading.question ? { question: reading.question } : {}) };
  };
  const demoNodes = preparedNodes.map(node => ({ ...node, ...readingFor(node.id), body: NODE_CONTENT[node.id].body }));
  for (const node of demoNodes) {
    if (available.has(node.id)) throw new Error('Atlas material already exists: ' + node.id);
    node.sourceRefs = node.sourceRefs.map((ref) => typeof ref === 'string' ? { label: new URL(ref).hostname.replace(/^www\./, ''), ref } : ref);
  }
  const visibleSources = sourceNodes.filter((node) => sourceIds.includes(node.id)).map((node) => ({ ...node, cluster: 'sources', ...readingFor(node.id), atlasBody: NODE_CONTENT[node.id].body }));
  const positioned = [...visibleSources, ...demoNodes];
  const prominent = new Set(["work","demo:gateway","demo:recurrence","demo:affirmation","demo:shepherd"]);
  for (const cluster of clusters) {
    const members = positioned.filter((node) => node.cluster === cluster.id);
    members.forEach((node, index) => {
      node.prominent = prominent.has(node.id);
      const angle = index * Math.PI * (3 - Math.sqrt(5));
      const radius = index === 0 ? 0 : 27 + Math.sqrt(index) * 16;
      node.position = [
        Math.round(cluster.center[0] + Math.cos(angle) * radius),
        Math.round(cluster.center[1] + Math.sin(angle) * radius * 0.86),
        Math.round(cluster.center[2] + Math.sin(index * 1.73) * 22),
      ];
    });
  }
  const byId = new Map(positioned.map((node) => [node.id, node]));
  const nodes = sourceNodes.map((node) => byId.get(node.id) ?? node).concat(demoNodes);
  const edges = preparedEdges.map(edge => ({ ...edge, inquiry: inquiry.relation(edge.id) }));
  const extraIds = new Set(extensionDefinitions.flatMap((entry) => entry[5].map(demoId)));
  const defaultIds = [...sourceIds, ...demoNodes.filter((node) => !extraIds.has(node.id)).map((node) => node.id)];
  const extensions = extensionDefinitions.map(([id, ru, en, ruSub, enSub, slugs]) => {
    const nodeIds = slugs.map(demoId);
    const included = new Set(nodeIds);
    return { id, title: bi(ru, en), subtitle: bi(ruSub, enSub), nodeIds, edgeIds: edges.filter((edge) => included.has(edge.from) || included.has(edge.to)).map((edge) => edge.id) };
  });
  const { fingerprint: _sourceFingerprint, ...sourceFields } = sourceLibrary;
  const library = { ...sourceFields, schema: 'tos_constructor_library_v1', nodes, atlas: { defaultIds, edges, clusters, assemblies: makeAssemblies(edges), extensions, routes: RESEARCH_ROUTES, inquiry: INQUIRY_FOUNDATION, lensInquiry: LENS_INQUIRY, references: PRIMARY_SOURCES } };
  const encoded = new TextEncoder().encode(JSON.stringify(canonical(library)));
  const digest = await globalThis.crypto.subtle.digest('SHA-256', encoded);
  library.fingerprint = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
  return library;
}
