import {createAtlasLibrary} from '../../constructor/atlas-data.mjs';
import {corpusWorkTitle} from '../observatory/human-presentation.mjs';
import antiquityData from './antiquity-space-data.json' with {type: 'json'};

const PUBLIC_PROFILE_IDS = Object.freeze({
  zarathustra: 'tos.work.friedrich-nietzsche.also-sprach-zarathustra',
  antiquity: Object.freeze({
    apology: 'tos.work.plato.apology',
    meno: 'tos.work.plato.meno',
    ethics: 'tos.work.aristotle.nicomachean-ethics',
  }),
});
const SOURCE_ROOT = Object.freeze({
  'chapter-p3.r2': {ru: 'Третья часть · «О видении и загадке»', en: 'Part Three · “On the Vision and the Riddle”', label: 'Vom Gesicht und Räthsel'},
  'chapter-p3.r13': {ru: 'Третья часть · «Исцелившийся»', en: 'Part Three · “The Convalescent”', label: 'Der Genesende'},
  moment: {ru: 'Образ мгновения', en: 'The image of the moment'},
  'all-things': {ru: 'Песня зверей о целом', en: 'The animals’ song of the whole'},
  'same-life': {ru: 'Та же самая жизнь', en: 'The very same life'},
  dossier: {ru: 'Сопоставленные источники', en: 'Sources brought together'},
});
const bi = (ru, en) => ({ru, en});
const ANTIQUITY_BRANCHES = antiquityData.branches;
const text = value => typeof value === 'string' ? value : '';
const englishLocator = value => text(value).replace(/Глава/giu, 'Chapter').replace(/Книга/giu, 'Book');
const sourceRef = (label, ref) => ({label, ...(ref ? {ref} : {})});
const digest = async value => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify(value)))), byte => byte.toString(16).padStart(2, '0')).join('');

async function selectedDocuments(provider, profile) {
  if (!provider || typeof provider.describe !== 'function' || typeof provider.catalog !== 'function' || typeof provider.document !== 'function' || typeof provider.structure !== 'function') {
    throw new TypeError('A bound public corpus provider is required.');
  }
  const manifest = await provider.describe();
  const selected = manifest.profiles?.[profile];
  if (!selected || !Array.isArray(selected.documentIds)) throw new Error(`Public corpus profile is unavailable: ${profile}`);
  const page = await provider.catalog({query: '', limit: 100});
  const catalog = new Map(page.items.map(item => [item.id, item]));
  const documents = new Map();
  for (const id of selected.documentIds) {
    const entry = manifest.documents.find(item => item.id === id);
    const listed = catalog.get(id);
    if (!entry || !listed) throw new Error(`Public source metadata is missing for ${id}`);
    const document = await provider.document({documentId: id});
    if (document.id !== id || !Array.isArray(document.versions) || !document.versions.length) throw new Error(`Public source versions are missing for ${id}`);
    documents.set(id, {entry, listed, document});
  }
  return {manifest, selected, documents};
}

async function exactStructure(provider, document, version) {
  const items = [];
  let cursor = null;
  let pages = 0;
  do {
    const page = await provider.structure({documentId: document.id, versionId: version.id, limit: 100, cursor});
    items.push(...page.items);
    cursor = page.nextCursor;
    pages += 1;
    if (pages > 40) throw new Error(`Public structure exceeds the bounded lookup for ${document.id}`);
  } while (cursor);
  return items;
}

function preferredVersion(document, language = 'en') {
  return document.versions.find(version => version.language === language) ?? document.versions.find(version => version.language === 'en') ?? document.versions[0];
}

function publicWorkNode(item, version, id = item.id) {
  const title = bi(corpusWorkTitle(item, 'ru'), corpusWorkTitle(item, 'en'));
  const author = item.author ?? '';
  return {
    id, kind: 'work', parentId: null, title,
    body: bi(`${author} · общедоступный источник`, `${author} · public source`),
    sourceRefs: [sourceRef(title, item.id)],
    publicDocumentId: item.id, publicVersionId: version.id, sourceAccess: 'public-document',
    sourcePosture: 'published metadata; text is read only through the selected public provider',
  };
}

function exactUnit(items, locator) {
  if (!locator) return null;
  const clean = value => value.normalize('NFKC').toLocaleLowerCase('en').replace(/[–—]/g, '-').replace(/\s+/g, ' ').trim();
  const candidate = clean(locator);
  // Locators here are bibliographic sections. Bind only a unique public unit
  // whose owner-published structural label is the same section.
  const matches = items.filter(item => {
    const label = clean(item.label);
    return label === candidate || candidate.startsWith(label + ' · ');
  });
  return matches.length === 1 ? matches[0] : null;
}

async function makeZarathustra(sources, structure) {
  const id = PUBLIC_PROFILE_IDS.zarathustra;
  const item = sources.documents.get(id);
  if (!item) throw new Error('The Zarathustra public source is not selected.');
  const version = preferredVersion(item.document, 'de');
  const nodes = [{...publicWorkNode(item.listed, version, 'work'), id: 'work'}];
  const ids = ['chapter-p3.r2', 'moment', 'chapter-p3.r13', 'all-things', 'same-life', 'dossier'];
  const bindings = new Map();
  const documentBindings = new Map([['work', {documentId: id, versionId: version.id}]]);
  for (const sourceId of ids) {
    const label = SOURCE_ROOT[sourceId];
    const isChapter = sourceId === 'chapter-p3.r2' || sourceId === 'chapter-p3.r13';
    const matched = isChapter ? structure.find(item => item.label === label.label) : null;
    const node = {
      id: sourceId, kind: sourceId === 'dossier' ? 'dossier' : isChapter ? 'chapter' : 'fragment', parentId: 'work',
      title: bi(label.ru, label.en), sourceRefs: [sourceRef(bi(item.listed.title, item.listed.title), id)],
      publicDocumentId: id, publicVersionId: version.id,
      sourceAccess: matched ? 'public-exact-unit' : 'metadata-only',
      sourcePosture: matched ? 'exact public structural unit' : 'metadata or interpretation; no exact text anchor',
    };
    nodes.push(node);
    if (sourceId !== 'dossier') documentBindings.set(sourceId, {documentId: id, versionId: version.id});
    if (matched) bindings.set(sourceId, {documentId: id, versionId: version.id, unitId: matched.unitId});
  }
  const sourceLibrary = {schema: 'tos_constructor_library_v1', rootId: 'work', fingerprint: '0'.repeat(64), nodes};
  const library = await createAtlasLibrary(sourceLibrary);
  for (const node of library.nodes) {
    if (node.id === 'work' || ids.includes(node.id)) continue;
    node.sourceAccess = node.demo ? 'proposed-interpretation' : 'metadata-only';
    node.sourcePosture = node.demo ? 'authored research interpretation; proposed, not reviewed or canonical' : 'metadata-only source reference';
  }
  for (const node of library.nodes.filter(value => value.id === 'work' || ids.includes(value.id))) {
    if (!node.sourceAccess) node.sourceAccess = node.id === 'work' ? 'public-document' : 'metadata-only';
  }
  library.space = {
    profile: 'zarathustra', title: bi('Заратустра', 'Zarathustra'),
    intro: bi('Исследуйте подготовленную карту вечного возвращения и переходите к точным местам опубликованного издания.', 'Explore the prepared map of eternal recurrence and open exact locations in the published edition.'),
    posture: bi('Подготовленное исследовательское прочтение. Толкования и связи предложены для исследования и не являются каноном.', 'Prepared research readings. Interpretations and relations are proposed for inquiry and are not canon.'),
    routes: library.atlas.routes,
  };
  return {library, bindings, documentBindings};
}

function sourceWorkId(oldWorkId) {
  return PUBLIC_PROFILE_IDS.antiquity[oldWorkId] ?? null;
}

async function makeAntiquity(sources, structureByDocument) {
  const docs = [...sources.documents.entries()].map(([id, item]) => [id, item]);
  const listedById = new Map(docs.map(([id, item]) => [id, item.listed]));
  const versionById = new Map(docs.map(([id, item]) => [id, preferredVersion(item.document)]));
  const sourceNodes = new Map();
  const readingNodes = new Map();
  const routeNodes = [];
  const edges = [];
  const bindings = new Map();
  const documentBindings = new Map();
  const centers = [[-360, -150, 0], [-120, -160, 0], [120, -160, 0], [360, -150, 0], [-230, 150, 0], [220, 150, 0]];

  for (const [index, [documentId, item]] of docs.entries()) {
    const version = versionById.get(documentId);
    const workNode = publicWorkNode(item.listed, version, `antiquity:work:${documentId}`);
    workNode.position = centers[index % centers.length];
    sourceNodes.set(workNode.id, workNode);
    documentBindings.set(workNode.id, {documentId, versionId: version.id});
  }

  for (const [branchIndex, branch] of ANTIQUITY_BRANCHES.entries()) {
    const rootId = `antiquity:route:${branch.id}`;
    const rootPosition = centers[branchIndex % centers.length];
    const routeNode = {
      id: rootId, kind: 'question', parentId: null, demo: true,
      title: branch.rootTitle, body: branch.summary, question: branch.question,
      position: rootPosition, cluster: 'routes', sourceAccess: 'proposed-reading',
      sourcePosture: 'authored route from the earlier Antiquity space; proposed comparison, not canon',
      sourceRefs: branch.nodes.map(node => sourceRef(bi(`${text(node.source.workTitle.ru)} · ${node.source.locator}`, `${text(node.source.workTitle.en)} · ${englishLocator(node.source.locator)}`), sourceWorkId(node.source.workId))),
    };
    routeNodes.push(routeNode);
    const steps = [];
    const transitions = [];
    for (const [nodeIndex, node] of branch.nodes.entries()) {
      const readingId = `antiquity:reading:${node.id}`;
      const sourceId = `antiquity:source:${node.id}`;
      const publicId = sourceWorkId(node.source.workId);
      const listed = publicId ? listedById.get(publicId) : null;
      const version = publicId ? versionById.get(publicId) : null;
      const citeLabel = bi(`${node.source.workTitle.ru} · ${node.source.locator}`, `${node.source.workTitle.en} · ${englishLocator(node.source.locator)}`);
      if (!sourceNodes.has(sourceId)) {
        const source = {
          id: sourceId, kind: 'fragment', parentId: publicId ? `antiquity:work:${publicId}` : null,
          title: citeLabel, sourceRefs: [sourceRef(citeLabel, publicId ?? node.source.sourceId)],
          workId: node.source.workId, legacySourceId: node.source.sourceId, locator: node.source.locator,
          sourceAccess: 'metadata-only', sourcePosture: 'bibliographic locator only; no source text or substituted anchor',
          ...(publicId ? {publicDocumentId: publicId, publicVersionId: version.id} : {}),
        };
        sourceNodes.set(sourceId, source);
        if (publicId && version) {
          documentBindings.set(sourceId, {documentId: publicId, versionId: version.id});
          const structural = exactUnit(structureByDocument.get(publicId) ?? [], node.source.locator);
          if (structural) {
            source.sourceAccess = 'public-exact-unit';
            source.sourcePosture = 'exact unit ID from the selected public version structure';
            bindings.set(sourceId, {documentId: publicId, versionId: version.id, unitId: structural.unitId});
          }
        }
      }
      const reading = {
        id: readingId, kind: 'interpretation', parentId: null, demo: true,
        title: node.title, body: node.reading, question: node.question,
        position: [rootPosition[0] + ((nodeIndex % 3) - 1) * 88, rootPosition[1] + (Math.floor(nodeIndex / 3) - 1) * 86, (nodeIndex % 2) * 14],
        cluster: 'readings', sourceAccess: 'proposed-reading',
        sourcePosture: 'editorial reading from the earlier Antiquity tree; proposed, not canonical',
        sourceRefs: [sourceRef(citeLabel, publicId ?? node.source.sourceId)],
        sourceNodeId: sourceId,
      };
      readingNodes.set(readingId, reading);
      if (publicId && version) documentBindings.set(readingId, {documentId: publicId, versionId: version.id});
      const exactBinding = bindings.get(sourceId);
      steps.push({nodeId: readingId, sourceNodeId: sourceId, title: node.title, body: node.reading, question: node.question,
        grounds: [{ref: exactBinding?.unitId ?? sourceId, focus: citeLabel, posture: exactBinding ? 'public-exact-unit' : 'metadata-only'}],
        sourceRefs: reading.sourceRefs});
      edges.push({id: `antiquity:route-source:${node.id}`, from: rootId, to: sourceId, kind: 'contains', label: bi('Источник', 'Source'), body: citeLabel});
      edges.push({id: `antiquity:reading-source:${node.id}`, from: readingId, to: sourceId, kind: 'supports', label: bi('Основано на месте', 'Grounded in this passage'), body: bi('Предлагаемое прочтение возвращается к указанному месту; полный текст доступен только при точной привязке в публичном источнике.', 'This proposed reading points back to the cited passage; full text is available only through an exact binding in the public source.')});
    }
    for (const edge of branch.edges) {
      const from = `antiquity:reading:${edge.from}`;
      const to = `antiquity:reading:${edge.to}`;
      if (!readingNodes.has(from) || !readingNodes.has(to)) continue;
      const id = `antiquity:relation:${branch.id}:${edge.from}:${edge.to}`;
      edges.push({id, from, to, kind: edge.kind, label: edge.label, body: edge.body, posture: 'proposed editorial comparison'});
      transitions.push({id, fromNodeId: from, toNodeId: to, label: edge.label, body: edge.body, kind: edge.kind, posture: 'proposed editorial comparison'});
    }
    edges.push({id: `antiquity:route-entry:${branch.id}`, from: rootId, to: `antiquity:reading:${branch.root}`, kind: 'develops', label: bi('Начать с этого места', 'Begin here'), body: branch.question});
    routeNodes[routeNodes.length - 1].route = {id: branch.id, title: branch.rootTitle, subtitle: branch.summary, question: branch.question, steps, transitions, posture: 'proposed editorial route'};
  }

  const nodes = [...sourceNodes.values(), ...routeNodes, ...readingNodes.values()];
  const rootId = [...sourceNodes.values()][0]?.id;
  const defaultIds = [...routeNodes.map(node => node.id), ...readingNodes.keys(), ...sourceNodes.keys()];
  const clusters = [
    {id: 'routes', title: bi('Маршруты', 'Routes'), subtitle: bi('Вопросы старого Древа Античности', 'Questions from the earlier Antiquity tree'), color: '#d6b37a', center: [0, 0, 0]},
    {id: 'readings', title: bi('Прочтения', 'Readings'), subtitle: bi('Предлагаемые ходы мысли', 'Proposed turns of thought'), color: '#8db8c6', center: [0, 0, 0]},
  ];
  const library = {
    schema: 'tos_constructor_library_v1', rootId, fingerprint: '0'.repeat(64), nodes,
    atlas: {defaultIds, edges, clusters, assemblies: [], extensions: [], routes: routeNodes.map(node => node.route)},
    space: {
      profile: 'antiquity', title: bi('Античность', 'Antiquity'),
      intro: bi('Вопросы из прежнего Древа Античности связывают Менона, «Никомахову этику» и «Апологию Сократа» с более широкими сопоставлениями.', 'Questions from the earlier Antiquity tree connect Meno, the Nicomachean Ethics and the Apology with broader comparisons.'),
      posture: bi('Античные ветви и сопоставления — авторские исследовательские предложения. Связи не утверждают канон, историческое влияние или тождество изданий.', 'Ancient branches and comparisons are authored research proposals. Relations do not claim canon, historical influence, or edition identity.'),
      lenses: [
        {id: 'tree', icon: '✧', title: bi('Древо', 'The tree'), description: bi('Все ветви, источники и переходы этого профиля.', 'All branches, sources and transitions in this profile.')},
        {id: 'sources', icon: '⌘', title: bi('Источники', 'Sources'), description: bi('Публичные произведения и точные библиографические места.', 'Public works and exact bibliographic locations.')},
        {id: 'meaning', icon: '◈', title: bi('Прочтения', 'Readings'), description: bi('Авторские прочтения, вопросы и их основания.', 'Authored readings, questions and their grounds.')},
        {id: 'tensions', icon: '⋈', title: bi('Различия', 'Differences'), description: bi('Возражения, уточнения и аналогии в предложенных маршрутах.', 'Objections, qualifications and analogies in the proposed routes.')},
      ],
      relationKinds: ['contains', 'supports', 'contrasts', 'questions', 'echoes', 'develops'],
      routes: routeNodes.map(node => node.route),
    },
  };
  library.fingerprint = await digest({...library, fingerprint: undefined});
  return {library, bindings, documentBindings};
}

/** Prepare a profile-specific, metadata-bounded Constructor library for the public space. */
export async function createPublicSpace({profile, provider} = {}) {
  if (!['zarathustra', 'antiquity'].includes(profile)) throw new TypeError('An explicit public space profile is required.');
  const sources = await selectedDocuments(provider, profile);
  const structures = new Map();
  for (const [documentId, item] of sources.documents) {
    const version = preferredVersion(item.document, profile === 'zarathustra' ? 'de' : 'en');
    structures.set(documentId, await exactStructure(provider, item.document, version));
  }
  const prepared = profile === 'zarathustra'
    ? await makeZarathustra(sources, structures.get(PUBLIC_PROFILE_IDS.zarathustra) ?? [])
    : await makeAntiquity(sources, structures);
  return {...prepared, profile, provider, sourceRevision: sources.manifest.revision};
}
