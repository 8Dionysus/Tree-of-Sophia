import type { Graph as CosmosGraph, GraphConfig } from "@cosmos.gl/graph";
import Graphology from "graphology";
import forceAtlas2 from "graphology-layout-forceatlas2";
import Sigma from "sigma";
import { createToSWebActions } from "./actions";
import { localizedContentPayload, localizedContentText } from "./content-i18n";
import StarNodeProgram from "./star-node-program";
import "./styles.css";

type Mode = "philosophy" | "corpus";
type GraphMode = "clusters" | "nodes";
type DensityMode = "overview" | "focused" | "dense";
type RendererMode = "cosmos" | "sigma";
type LayoutFamily = "timeline" | "flow" | "evidence" | "semantic" | "infrastructure" | "organic";
type Language = "en" | "ru";

type BootPayload = {
  service: string;
  default_view: string;
  default_philosophy_view: string;
  write_enabled: boolean;
  projection_mode: string;
  neo4j: {
    configured: boolean;
    ready: boolean;
    note: string;
  };
};

type AnyItem = Record<string, unknown>;

type MultilingualLabel = {
  label?: {
    original?: string | null;
    ru?: string | null;
    en?: string | null;
  };
};

type ViewCard = {
  view_id: string;
  title?: string;
  purpose?: string;
  layout_hint?: string;
  graph_layers?: string[];
  entry_surface?: string;
};

type Cluster = AnyItem & {
  cluster_id: string;
  cluster_kind?: string;
  label?: string;
  member_node_ids?: string[];
  source_refs?: string[];
  graph_layers?: string[];
};

type GraphNode = AnyItem & {
  node_id: string;
  label?: string;
  node_type?: string;
  source_ref?: string;
  graph_layers?: string[];
};

type GraphEdge = AnyItem & {
  edge_id: string;
  from_id: string;
  to_id: string;
  predicate_id?: string;
  source_ref?: string;
  graph_layers?: string[];
};

type RelationDirection = "outgoing" | "incoming" | "internal" | "adjacent";

type RelationRow = GraphEdge & {
  direction?: RelationDirection;
  from_label?: string;
  to_label?: string;
  primary_predicate?: string;
  relation_count?: number;
  member_edge_ids?: string[];
  source_refs?: string[];
};

type PhilosophyViewPayload = {
  view: ViewCard;
  nodes?: GraphNode[];
  edges?: GraphEdge[];
  clusters?: Cluster[];
  source_refs?: string[];
  review_packet?: {
    packet?: AnyItem;
  };
};

type CorpusViewPayload = {
  view: ViewCard;
  items?: AnyItem[];
  nodes?: GraphNode[];
  edges?: GraphEdge[];
};

type NeighborhoodPayload = {
  query_backend?: string;
  fallback_reason?: string | null;
  node?: GraphNode;
  neighbors?: GraphNode[];
  edges?: GraphEdge[];
  depth?: number;
  layers?: string[];
  predicates?: string[];
  source_refs?: string[];
};

type PathPayload = {
  query_backend?: string;
  fallback_reason?: string | null;
  from_id?: string;
  to_id?: string;
  found?: boolean;
  nodes?: GraphNode[];
  edges?: GraphEdge[];
  max_depth?: number;
  layers?: string[];
  predicates?: string[];
  source_refs?: string[];
};

type ScaleExportTable = "nodes" | "edges" | "clusters" | "cluster-node-memberships" | "cluster-edge-memberships";

type AppState = {
  language: Language;
  mode: Mode;
  graphMode: GraphMode;
  rendererMode: RendererMode;
  currentViewId: string;
  activeLayers: Set<string>;
  activePredicates: Set<string>;
  densityMode: DensityMode;
  minRelationCount: number;
  status: Record<string, AnyItem>;
  philosophyViews: ViewCard[];
  corpusViews: ViewCard[];
  currentView: PhilosophyViewPayload | CorpusViewPayload | null;
  sourceNotes: GraphNode[];
  sourceNoteEdges: GraphEdge[];
  selected: AnyItem | null;
  selectedGraphId: string | null;
  results: AnyItem[];
  relationItems: AnyItem[];
  expandedCluster: Cluster | null;
  searchQuery: string;
  neighborhood: NeighborhoodPayload | null;
  pathStartNodeId: string | null;
  pathPacket: PathPayload | null;
  inspectorOpen: boolean;
};

declare global {
  interface Window {
    __TOS_GRAPH_BOOT__?: BootPayload;
  }
}

const boot: BootPayload = window.__TOS_GRAPH_BOOT__ || {
  service: "tree-of-sophia-access",
  default_view: "corpus-topology",
  default_philosophy_view: "chronology",
  write_enabled: false,
  projection_mode: "json",
  neo4j: { configured: false, ready: false, note: "Standalone JSON backend" },
};

const palette = {
  default: "#69c8b5",
  blue: "#79bcec",
  gold: "#e2bd68",
  red: "#e47f77",
  violet: "#ad92e7",
  grey: "#94a6a5",
  line: "rgba(23,32,29,0.22)",
};

const scaleExportTables: { table: ScaleExportTable; titleKey: string }[] = [
  { table: "nodes", titleKey: "export.nodes" },
  { table: "edges", titleKey: "export.edges" },
  { table: "clusters", titleKey: "export.clusters" },
  { table: "cluster-node-memberships", titleKey: "export.clusterNodes" },
  { table: "cluster-edge-memberships", titleKey: "export.clusterEdges" },
];

const uiText: Record<Language, Record<string, string>> = {
  en: {
    "brand.title": "Tree of Sophia",
    "brand.note": "A living atlas of philosophy",
    "language.label": "Language",
    "mode.philosophy": "Atlas",
    "mode.corpus": "Library",
    "search.placeholder": "Find a work, witness, tradition, or idea",
    "button.search": "Search",
    "button.sync": "Sync",
    "section.views": "Views",
    "section.layers": "Layers",
    "section.relations": "Relations",
    "section.scaleExport": "Scale Export",
    "chip.mode": "Mode",
    "chip.view": "View",
    "chip.renderer": "Renderer",
    "chip.neo4j": "Neo4j",
    "chip.projection": "Projection",
    "button.clusters": "Regions",
    "button.nodes": "Objects",
    "button.fit": "Show all",
    "button.fullView": "Leave focus",
    "button.reviewPacket": "Review packet",
    "button.unresolved": "Unresolved",
    "button.snapshot": "Snapshot",
    "button.audit": "Audit",
    "button.copy": "Copy",
    "button.copyUrl": "Copy URL",
    "button.contracts": "Contracts",
    "button.manifest": "Manifest",
    "empty.graph": "No graph payload for this view.",
    "empty.noView": "No view loaded.",
    "empty.filters": "No graph payload for current filters.",
    "inspector.selection": "About this place",
    "inspector.nothing": "Nothing selected.",
    "inspector.help": "Select an object or a relation to read its place in the Tree.",
    "inspector.open": "Open reading",
    "inspector.close": "Close reading",
    "detail.overview": "In brief",
    "detail.sourceDisclosure": "About the source",
    "detail.sourceDisclosureNote": "Primary and scholarly witnesses",
    "muted.noLayerCorpus": "No layer contract for corpus view.",
    "muted.noLayers": "No layers for this view.",
    "muted.noRelationCorpus": "No relation contract for corpus view.",
    "muted.scaleCorpus": "Scale export follows the ToS philosophy projection.",
    "relation.of": "of",
    "relation.relations": "relations",
    "relation.min": "min",
    "relation.all": "All",
    "relation.refs": "refs",
    "relation.memberEdges": "member edges",
    "export.allLayers": "all layers",
    "export.noLayers": "no layers selected",
    "export.nodes": "Nodes",
    "export.edges": "Edges",
    "export.clusters": "Clusters",
    "export.clusterNodes": "Cluster nodes",
    "export.clusterEdges": "Cluster edges",
    "detail.results": "Results",
    "detail.relations": "Relations",
    "detail.noDetail": "No detail",
    "detail.noDetailBody": "Use search or click the graph.",
    "detail.sourceRefs": "Source refs",
    "detail.payload": "Payload",
    "detail.relationRoute": "Relation route",
    "detail.from": "From",
    "detail.to": "To",
    "detail.predicate": "Predicate",
    "detail.relationCount": "Relation count",
    "detail.predicateMix": "Predicate mix",
    "detail.graphLayers": "Graph layers",
    "detail.memberEdges": "Member edges",
    "detail.relationReading": "Relation reading",
    "detail.predicatesNearby": "Predicates nearby",
    "detail.selectedRelations": "Selected relations",
    "detail.neighborhood": "Neighborhood",
    "detail.members": "members",
    "detail.neighbors": "Neighbors",
    "detail.neighborCount": "neighbors",
    "detail.neighborhoodRelations": "Neighborhood relations",
    "detail.pathStart": "Path start",
    "detail.path": "Path",
    "detail.pathNodes": "Path nodes",
    "detail.pathRelations": "Path relations",
    "detail.noRoute": "No route found",
    "detail.maxDepth": "max depth",
    "detail.allActiveLayers": "all active layers",
    "detail.allActivePredicates": "all active predicates",
    "detail.backend": "backend",
    "detail.fallback": "fallback",
    "route.neighborhood": "Neighborhood",
    "route.pathStartSet": "Path start set",
    "route.useAsPathStart": "Use as path start",
    "route.pathFrom": "Path from",
    "state.loading": "loading",
    "state.none": "none",
    "caption.view": "View",
    "caption.nodes": "nodes",
    "caption.links": "links",
    "selection.scaleExportUrl": "Scale export URL",
    "selection.search": "Search",
    "selection.reviewPacket": "Review packet",
    "selection.unresolved": "Unresolved",
    "load.failed": "Load failed",
  },
  ru: {
    "brand.title": "Древо Софии",
    "brand.note": "Живой атлас философии",
    "language.label": "Язык",
    "mode.philosophy": "Атлас",
    "mode.corpus": "Библиотека",
    "search.placeholder": "Найдите произведение, свидетельство, традицию или идею",
    "button.search": "Поиск",
    "button.sync": "Синхронизировать",
    "section.views": "Виды",
    "section.layers": "Слои",
    "section.relations": "Связи",
    "section.scaleExport": "Масштабный экспорт",
    "chip.mode": "Режим",
    "chip.view": "Вид",
    "chip.renderer": "Рендерер",
    "chip.neo4j": "Neo4j",
    "chip.projection": "Проекция",
    "button.clusters": "Области",
    "button.nodes": "Объекты",
    "button.fit": "Показать всё",
    "button.fullView": "Выйти из фокуса",
    "button.reviewPacket": "Пакет ревью",
    "button.unresolved": "Нерешенное",
    "button.snapshot": "Снимок",
    "button.audit": "Аудит",
    "button.copy": "Копировать",
    "button.copyUrl": "Копировать URL",
    "button.contracts": "Контракты",
    "button.manifest": "Манифест",
    "empty.graph": "Для этого вида нет графового пакета.",
    "empty.noView": "Вид не загружен.",
    "empty.filters": "Для текущих фильтров нет графового пакета.",
    "inspector.selection": "Об этом месте",
    "inspector.nothing": "Ничего не выбрано.",
    "inspector.help": "Выберите объект или связь, чтобы прочитать их место в Древе.",
    "inspector.open": "Открыть чтение",
    "inspector.close": "Закрыть чтение",
    "detail.overview": "Коротко",
    "detail.sourceDisclosure": "Об источнике",
    "detail.sourceDisclosureNote": "Первичные и научные свидетельства",
    "muted.noLayerCorpus": "У корпусного вида нет контракта слоев.",
    "muted.noLayers": "У этого вида нет слоев.",
    "muted.noRelationCorpus": "У корпусного вида нет контракта связей.",
    "muted.scaleCorpus": "Масштабный экспорт следует философской проекции ToS.",
    "relation.of": "из",
    "relation.relations": "связей",
    "relation.min": "мин.",
    "relation.all": "Все",
    "relation.refs": "ссылок",
    "relation.memberEdges": "вложенных связей",
    "export.allLayers": "все слои",
    "export.noLayers": "слои не выбраны",
    "export.nodes": "Узлы",
    "export.edges": "Связи",
    "export.clusters": "Кластеры",
    "export.clusterNodes": "Узлы кластеров",
    "export.clusterEdges": "Связи кластеров",
    "detail.results": "Результаты",
    "detail.relations": "Связи",
    "detail.noDetail": "Нет деталей",
    "detail.noDetailBody": "Используйте поиск или кликните по графу.",
    "detail.sourceRefs": "Ссылки на источник",
    "detail.payload": "Пакет",
    "detail.relationRoute": "Маршрут связи",
    "detail.from": "От",
    "detail.to": "К",
    "detail.predicate": "Предикат",
    "detail.relationCount": "Число связей",
    "detail.predicateMix": "Состав предикатов",
    "detail.graphLayers": "Слои графа",
    "detail.memberEdges": "Вложенные связи",
    "detail.relationReading": "Чтение связей",
    "detail.predicatesNearby": "Ближайшие предикаты",
    "detail.selectedRelations": "Выбранные связи",
    "detail.neighborhood": "Окрестность",
    "detail.members": "участников",
    "detail.neighbors": "Соседи",
    "detail.neighborCount": "соседей",
    "detail.neighborhoodRelations": "Связи окрестности",
    "detail.pathStart": "Начало пути",
    "detail.path": "Путь",
    "detail.pathNodes": "Узлы пути",
    "detail.pathRelations": "Связи пути",
    "detail.noRoute": "Маршрут не найден",
    "detail.maxDepth": "макс. глубина",
    "detail.allActiveLayers": "все активные слои",
    "detail.allActivePredicates": "все активные предикаты",
    "detail.backend": "движок",
    "detail.fallback": "запасной путь",
    "route.neighborhood": "Окрестность",
    "route.pathStartSet": "Начало пути задано",
    "route.useAsPathStart": "Сделать началом пути",
    "route.pathFrom": "Путь от",
    "state.loading": "загрузка",
    "state.none": "нет",
    "caption.view": "Вид",
    "caption.nodes": "узлов",
    "caption.links": "связей",
    "selection.scaleExportUrl": "URL масштабного экспорта",
    "selection.search": "Поиск",
    "selection.reviewPacket": "Пакет ревью",
    "selection.unresolved": "Нерешенное",
    "load.failed": "Загрузка не удалась",
  },
};

const tokenText: Record<Language, Record<string, string>> = {
  en: {},
  ru: {
    adjacent: "Смежные",
    "authority-layers": "слои авторитетности",
    belongs_to_genre: "принадлежит жанру",
    branches: "ветви",
    candidate: "кандидат",
    "candidate-endpoint": "кандидатная точка",
    "candidate-node": "кандидатный узел",
    "candidate-relation": "кандидатные связи",
    "canon-candidate-status": "статус канона/кандидата",
    "canon-promotion": "продвижение в канон",
    canonized_by: "канонизирован через",
    "canonical-relation": "канонические связи",
    clusters: "кластеры",
    commented_by: "комментируется через",
    concept: "понятие",
    "concept-problem": "понятие или проблема",
    "conceptual-relation": "понятийные связи",
    contains: "содержит",
    contains_work: "содержит произведение",
    contains_row: "содержит строку",
    contains_view: "содержит вид",
    containing_work: "происходит в произведении",
    contested_by: "оспаривается через",
    dense: "плотно",
    deferred: "отложено",
    develops_concept: "развивает понятие",
    "diff-snapshot": "снимок различий",
    edges: "связи",
    "evidence-relation": "свидетельские связи",
    "evidence-status": "статус свидетельств",
    flow: "поток",
    fragments_preserved_by: "фрагменты сохранены через",
    focused: "фокус",
    "graph-view": "графовый вид",
    graph_layers: "слои графа",
    "graph-layers": "слои графа",
    has_node_type_pressure: "давление типа узла",
    has_prepared_dossier: "имеет подготовленное досье",
    has_relation_pressure: "давление типа связи",
    "historical-relation": "исторические связи",
    incoming: "Входящие",
    influences: "влияет",
    institutionalized_in: "институционализировано в",
    internal: "Внутренние",
    manifests: "манифесты",
    "master-table": "мастер-таблица",
    "master-table-row": "строка мастер-таблицы",
    "material-artifact": "материальный свидетель",
    "material-candidate": "кандидатный материал",
    "material-canonical": "канонический материал",
    "material-cluster": "собрание материалов",
    "material-event": "событие",
    "material-evidence": "свидетельство",
    "material-record": "запись источника",
    "material-reference": "ссылка",
    "material-relation": "связь материалов",
    "material-representation": "представление",
    derived_from: "создано из",
    embodied_by: "воплощено в издании",
    exemplified_by: "представлено экземпляром",
    has_expression: "выражено в переводе",
    is_derivative_of: "перерабатывает перевод",
    is_scholarly_reconstruction_of: "реконструирует произведение",
    published_expression: "публикует перевод",
    recorded_witness: "фиксирует свидетельство",
    reported_witness_of: "свидетельствует о",
    resulting_edition: "создаёт издание",
    canonical_event_surface: "раскрывает событие",
    represents: "представляет",
    references: "ссылается на",
    "node-neighborhood": "окрестность узла",
    nodes: "узлы",
    organic: "органика",
    outgoing: "Исходящие",
    overview: "обзор",
    polemicizes_with: "полемизирует с",
    prepared_dossier: "подготовленное досье",
    "prepared-dossier": "подготовленное досье",
    "promotion-flow": "поток продвижения",
    preserved_in: "сохранено в",
    preserves_in: "сохраняет в",
    "provenance-dag": "DAG происхождения",
    preview: "предпросмотр",
    ready: "готово",
    receives_from: "получает от",
    region: "регион",
    relation: "связь",
    relation_edges: "ребра связей",
    "relation-edges": "ребра связей",
    relation_packs: "пакеты связей",
    "relation-packs": "пакеты связей",
    resources: "ресурсы",
    review_packets: "пакеты ревью",
    "review-packets": "пакеты ревью",
    "route-graph": "граф маршрутов",
    "school-institution": "школа/институт",
    semantic: "семантика",
    "source-relation": "источниковые связи",
    "source-witness": "источник-свидетель",
    survives_as: "выживает как",
    timeline: "хронология",
    transforms_concept: "преобразует понятие",
    translated_into: "переведено в",
    "transmission-relation": "связи передачи",
    transmits_to: "передает к",
    uncertain_relation: "неуверенная связь",
    uses_language: "использует язык",
    uses_script: "использует письмо",
    views: "виды",
    "view-section": "раздел вида",
    work: "работа",
  },
};

const viewTitleText: Record<Language, Record<string, string>> = {
  en: {
    chronology: "Across time",
    transmission: "Paths of transmission",
    "source-evidence": "Traces of sources",
    "concept-lineage": "Kinship of ideas",
    "institution-media": "Schools and milieux",
    "script-decipherment": "Scripts and discoveries",
    "imperial-multilingualism": "Languages of empires",
    "ritual-law": "Ritual and law",
    "epigraphic-network": "World of inscriptions",
    "lost-corpus": "Lost texts",
    "canon-promotion": "How knowledge changes",
    human_atlas: "Materials and witnesses",
    transmission_map: "Material routes",
    evidence_lab: "Evidence and representations",
  },
  ru: {
    chronology: "Во времени",
    transmission: "Пути передачи",
    "source-evidence": "Следы источников",
    "concept-lineage": "Родство идей",
    "institution-media": "Школы и среды",
    "script-decipherment": "Письмена и открытия",
    "imperial-multilingualism": "Языки империй",
    "ritual-law": "Ритуал и закон",
    "epigraphic-network": "Мир надписей",
    "lost-corpus": "Утраченные тексты",
    "canon-promotion": "Как меняется знание",
    "corpus-topology": "Топология корпуса",
    "authority-layers": "Слои авторитетности",
    "route-graph": "Граф маршрутов",
    "node-neighborhood": "Окрестность узла",
    "provenance-dag": "DAG происхождения",
    "promotion-flow": "Поток продвижения",
    "diff-snapshot": "Снимок различий",
    human_atlas: "Материалы и свидетельства",
    transmission_map: "Пути материалов",
    evidence_lab: "События и представления",
  },
};

const viewSubtitleText: Record<Language, Record<string, string>> = {
  en: {
    human_atlas: "works, expressions, editions, items, and witnesses",
    transmission_map: "explicit relations between material records",
    evidence_lab: "events, representations, references, and recorded gaps",
  },
  ru: {
    chronology: "хронологические линии",
    transmission: "направленные коридоры",
    "source-evidence": "DAG свидетельств",
    "concept-lineage": "семантическая родословная",
    "institution-media": "карта инфраструктуры",
    "script-decipherment": "маршрут неопределенности",
    "imperial-multilingualism": "карта параллельных версий",
    "ritual-law": "инфраструктура закона и ритуала",
    "epigraphic-network": "распределенное публичное письмо",
    "lost-corpus": "маршрут отсутствия и свидетельств",
    "canon-promotion": "поток продвижения",
    "corpus-topology": "ветвящееся дерево дома ToS",
    "authority-layers": "переключение видимости по слоям корпуса",
    "route-graph": "маршруты пакетов связей",
    "node-neighborhood": "ограниченное расширение вокруг узла",
    "provenance-dag": "давление источников к кандидату, канону и экспорту",
    "promotion-flow": "проверка кандидатного материала к канону",
    "diff-snapshot": "сравнение снимков корпусного индекса",
    human_atlas: "произведения, выражения, издания, экземпляры и свидетельства",
    transmission_map: "явные связи между материальными записями",
    evidence_lab: "события, представления, ссылки и зафиксированные пробелы",
  },
};

type InitialRoute = {
  mode: Mode;
  viewId: string;
  graphMode: GraphMode;
  language: Language | null;
};

function readInitialRoute(): InitialRoute {
  const params = new URLSearchParams(window.location.search);
  const mode = params.get("mode") === "corpus" ? "corpus" : "philosophy";
  const graphMode = params.get("graph") === "nodes" ? "nodes" : "clusters";
  const requestedLanguage = params.get("ui") || params.get("lang");
  return {
    mode,
    viewId: params.get("view") || "",
    graphMode,
    language: requestedLanguage === "ru" || requestedLanguage === "en" ? requestedLanguage : null,
  };
}

const initialRoute = readInitialRoute();

function initialLanguage(): Language {
  if (initialRoute.language) return initialRoute.language;
  const stored = window.localStorage.getItem("tos-graph-language");
  if (stored === "ru" || stored === "en") return stored;
  return navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
}

const state: AppState = {
  language: initialLanguage(),
  mode: initialRoute.mode,
  graphMode: initialRoute.graphMode,
  rendererMode: "sigma",
  currentViewId: "",
  activeLayers: new Set(),
  activePredicates: new Set(),
  densityMode: "overview",
  minRelationCount: 1,
  status: {},
  philosophyViews: [],
  corpusViews: [],
  currentView: null,
  sourceNotes: [],
  sourceNoteEdges: [],
  selected: null,
  selectedGraphId: null,
  results: [],
  relationItems: [],
  expandedCluster: null,
  searchQuery: "",
  neighborhood: null,
  pathStartNodeId: null,
  pathPacket: null,
  inspectorOpen: false,
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing #app");
const appRoot = app;
document.documentElement.lang = state.language;

let graph = new Graphology({ multi: true, type: "directed" });
let renderer: Sigma | null = null;
let cosmosRenderer: CosmosGraph | null = null;
let cosmosModulePromise: Promise<typeof import("@cosmos.gl/graph")> | null = null;
let graphContainer: HTMLDivElement | null = null;
let nodeTooltip: HTMLDivElement | null = null;
let lastGraphItems = new Map<string, AnyItem>();
let cosmosPointItems: AnyItem[] = [];
let cosmosLinkItems: AnyItem[] = [];
let hoveredNodeId: string | null = null;
let ignoreGraphClicksUntil = 0;
let ignoreInspectorSelectionsUntil = 0;
let graphRenderVersion = 0;
let viewLoadRevision = 0;
let modeLoadRevision = 0;
let searchRevision = 0;
let neighborhoodRevision = 0;
let pathRevision = 0;
const lastPointer = { x: 0, y: 0 };

function text(value: unknown): string {
  return value === undefined || value === null ? "" : String(value);
}

function t(key: string): string {
  return uiText[state.language][key] || uiText.en[key] || key;
}

function tokenLabel(value: unknown): string {
  const original = text(value).trim();
  if (!original) return "";
  const lower = original.toLowerCase();
  const normalized = lower.replaceAll("_", "-");
  return tokenText[state.language][normalized] || tokenText[state.language][lower] || tokenText.en[normalized] || tokenText.en[lower] || original.replaceAll("_", " ").replaceAll("-", " ");
}

function viewDisplayTitle(view?: ViewCard | null): string {
  if (!view) return state.currentViewId || t("caption.view");
  return viewTitleText[state.language][view.view_id] || view.title || view.view_id;
}

function viewDisplaySubtitle(view?: ViewCard | null): string {
  if (!view) return "";
  return viewSubtitleText[state.language][view.view_id] || view.layout_hint || view.purpose || view.entry_surface || "";
}

function isKnownViewCard(item: AnyItem): item is ViewCard {
  const viewId = text(item.view_id);
  if (!viewId) return false;
  return [...state.philosophyViews, ...state.corpusViews].some((view) => view.view_id === viewId && (view === item || text(view.title) === text(item.title)));
}

function short(value: unknown, length = 58): string {
  const raw = text(value);
  return raw.length > length ? `${raw.slice(0, length - 1)}...` : raw;
}

function unwrapItem(item: AnyItem): AnyItem {
  const nested = item.item;
  return nested && typeof nested === "object" && !Array.isArray(nested) ? (nested as AnyItem) : item;
}

function sourceOwnedDisplayLabel(item: AnyItem): string {
  const source = unwrapItem(item);
  const multilingual = source.multilingual;
  if (!multilingual || typeof multilingual !== "object" || Array.isArray(multilingual)) return "";
  const labels = (multilingual as MultilingualLabel).label;
  if (!labels || typeof labels !== "object") return "";
  const preferred = state.language === "ru" ? labels.ru : labels.en;
  const fallback = labels.original || labels.ru || labels.en;
  return text(preferred || fallback).trim();
}

function escapeHtml(value: unknown): string {
  return text(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function itemId(item: AnyItem): string {
  const source = unwrapItem(item);
  return text(
    source.node_id ||
      source.edge_id ||
      source.cluster_id ||
      source.packet_id ||
      source.pack_id ||
      source.id ||
      source.path ||
      source.view_id ||
      item.collection ||
      "item",
  );
}

function itemTitle(item: AnyItem): string {
  const source = unwrapItem(item);
  if (source.from_id && source.to_id) {
    return relationRouteText(source);
  }
  return text(source.label || source.title || source.name || source.view_id || itemId(source));
}

function itemSubtitle(item: AnyItem): string {
  const source = unwrapItem(item);
  return text(
    source.node_type ||
      source.cluster_kind ||
      source.predicate_id ||
      source.source_ref ||
      source.source_path ||
      source.path ||
      source.layout_hint ||
      source.purpose ||
      item.collection ||
      "",
  );
}

function humanKind(value: unknown): string {
  return tokenLabel(value);
}

function cleanPublicTitle(value: unknown): string {
  return text(value)
    .replace(/^Внутритекстовое событие:\s*/i, "")
    .replace(/^Text-internal event:\s*/i, "")
    .replace(/\s+/g, " ")
    .trim();
}

function relatedPublicTitle(item: AnyItem): string {
  if (!isPhilosophyView(state.currentView)) return "";
  const source = unwrapItem(item);
  const nodeById = new Map((state.currentView.nodes || []).map((node) => [node.node_id, node]));
  const memberIds = stringList(source.member_node_ids);
  if (source.cluster_kind === "material-event" && memberIds.length > 0) {
    const member = memberIds
      .map((nodeId) => nodeById.get(nodeId))
      .find((node) => node?.node_type === "material-event" && isPublicAtlasItem(node));
    if (member) return cleanPublicTitle(sourceOwnedDisplayLabel(member) || itemTitle(member));
  }
  if (source.node_type === "canon_node") {
    const surfaceEdge = (state.currentView.edges || []).find(
      (edge) => edge.to_id === source.node_id && predicateId(edge) === "canonical_event_surface",
    );
    const surface = surfaceEdge ? nodeById.get(surfaceEdge.from_id) : undefined;
    if (surface && isPublicAtlasItem(surface)) return cleanPublicTitle(sourceOwnedDisplayLabel(surface) || itemTitle(surface));
  }
  return "";
}

function displayTitle(item: AnyItem): string {
  const source = unwrapItem(item);
  if (source.from_id && source.to_id) return relationRouteText(source);
  if (isKnownViewCard(source)) return viewDisplayTitle(source);
  const raw = cleanPublicTitle(relatedPublicTitle(source) || sourceOwnedDisplayLabel(item) || itemTitle(item));
  const canon = raw.match(/^(?:Canon Or Candidate Status|Статус канона или кандидата):\s*(.+)$/i);
  if (canon) return `${state.language === "ru" ? "Статус" : "Status"} ${canon[1].trim()}`;
  const concept = raw.match(/^(?:Concept Or Problem|Концепт или проблема):\s*(.+)$/i);
  if (concept) return concept[1].trim();
  const corpus = raw.match(/^(?:Corpus Or Prepared Source Document|Корпус или подготовленный исходный документ):\s*(.+)$/i);
  const title = corpus ? corpus[1].trim() : raw;
  return localizedContentText(
    title
      .replace(/^ToS Deep Research[_\s:—-]*/i, "")
      .replace(/^A\d{2}\s*[—–:-]\s*/i, "")
      .replace(/\.docx$/i, "")
      .replace(/\s+/g, " ")
      .trim() || raw,
    state.language,
  );
}

function displaySubtitle(item: AnyItem): string {
  if (item.relation_count) {
    return `${humanKind(item.primary_predicate || item.predicate_id || "relation")} · ${text(item.relation_count)} ${t("relation.relations")}`;
  }
  const rawKind = text(item.cluster_kind || item.node_type || item.predicate_id);
  const publicKinds: Record<string, Record<Language, string>> = {
    "material-candidate": { ru: "Материал", en: "Material" },
    "material-canonical": { ru: "Материал", en: "Material" },
    "material-record": { ru: "Материал", en: "Material" },
    "material-cluster": { ru: "Собрание", en: "Collection" },
    "material-artifact": { ru: "Материальный свидетель", en: "Material witness" },
    "material-evidence": { ru: "Свидетельство", en: "Evidence" },
    "material-event": { ru: "Событие", en: "Event" },
    "material-expression": { ru: "Перевод или версия", en: "Expression" },
    "material-work": { ru: "Произведение", en: "Work" },
    "material-composite": { ru: "Составное свидетельство", en: "Composite witness" },
    "material-representation": { ru: "Представление", en: "Representation" },
    "material-reference": { ru: "Ссылка", en: "Reference" },
    "material-relation": { ru: "Связь", en: "Relation" },
    corpus: { ru: "Тематическая область", en: "Thematic region" },
    corpus_record: { ru: "Источник", en: "Source" },
    artifact_witness: { ru: "Материальный свидетель", en: "Material witness" },
    scholarly_composite: { ru: "Научная реконструкция", en: "Scholarly reconstruction" },
    source_planting: { ru: "Источник", en: "Source" },
    canon_node: { ru: "Узел Древа", en: "Tree node" },
    "candidate-node": { ru: "Материал Древа", en: "Tree material" },
  };
  const originalNodeType = propertyText(item, "original_node_type");
  const originalKinds: Record<string, Record<Language, string>> = {
    text_corpus: { ru: "Произведение или корпус", en: "Work or corpus" },
    institution: { ru: "Школа или учреждение", en: "School or institution" },
    school_tradition: { ru: "Школа или традиция", en: "School or tradition" },
    concept: { ru: "Понятие", en: "Concept" },
    language_script: { ru: "Язык или письменность", en: "Language or script" },
    medium: { ru: "Носитель", en: "Medium" },
    controversy: { ru: "Открытый вопрос", en: "Open question" },
    preservation_state: { ru: "Состояние сохранности", en: "State of preservation" },
    method: { ru: "Способ прочтения", en: "Method of reading" },
    genre: { ru: "Жанр", en: "Genre" },
    transmission_channel: { ru: "Путь передачи", en: "Transmission path" },
    civilization_literary_complex: { ru: "Культурный мир", en: "Cultural world" },
    frontier_case: { ru: "Пограничный случай", en: "Frontier case" },
    figure_anchor: { ru: "Человек", en: "Person" },
  };
  const kind = originalKinds[originalNodeType]?.[state.language] || publicKinds[rawKind]?.[state.language] || humanKind(rawKind);
  const subtitle = localizedContentText(itemSubtitle(item), state.language);
  const subtitleIsKind =
    subtitle === rawKind ||
    subtitle === kind ||
    subtitle.replaceAll("-", " ").toLowerCase() === rawKind.replaceAll("-", " ").toLowerCase() ||
    subtitle.replaceAll("-", " ").toLowerCase() === kind.toLowerCase();
  const pieces = [kind, subtitleIsKind ? "" : humanKind(subtitle)].filter(Boolean);
  return [...new Set(pieces)].join(" · ");
}

function searchCandidateKey(item: AnyItem): string {
  return `${item.collection || ""}:${itemId(item)}:${displayTitle(item)}`;
}

function appendSearchCandidate(items: AnyItem[], seen: Set<string>, item: AnyItem | undefined | null): void {
  if (!item || typeof item !== "object" || Array.isArray(item)) return;
  if (!isPublicAtlasItem(item)) return;
  const key = searchCandidateKey(item);
  if (seen.has(key)) return;
  seen.add(key);
  items.push(item);
}

function localizedSearchCandidates(): AnyItem[] {
  const items: AnyItem[] = [];
  const seen = new Set<string>();
  for (const item of state.results) appendSearchCandidate(items, seen, item);
  for (const view of state.mode === "philosophy" ? state.philosophyViews : state.corpusViews) appendSearchCandidate(items, seen, view);
  const current = state.currentView;
  if (current) {
    appendSearchCandidate(items, seen, current.view);
    if (isPhilosophyView(current)) {
      for (const item of current.clusters || []) appendSearchCandidate(items, seen, item);
      for (const item of current.nodes || []) appendSearchCandidate(items, seen, item);
      for (const item of current.edges || []) appendSearchCandidate(items, seen, item);
      appendSearchCandidate(items, seen, current.review_packet?.packet);
    } else {
      for (const item of current.items || []) appendSearchCandidate(items, seen, item);
    }
  }
  return items;
}

function localizedSearchHaystack(item: AnyItem): string {
  return [
    displayTitle(item),
    displaySubtitle(item),
    JSON.stringify(localizedContentPayload(item, state.language)),
  ]
    .join("\n")
    .toLowerCase();
}

function mergeLocalizedSearchResults(query: string, remoteResults: AnyItem[], limit: number): AnyItem[] {
  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery) return remoteResults.slice(0, limit);
  const merged: AnyItem[] = [];
  const seen = new Set<string>();
  for (const item of remoteResults) appendSearchCandidate(merged, seen, item);
  for (const item of localizedSearchCandidates()) {
    if (merged.length >= limit) break;
    if (seen.has(searchCandidateKey(item))) continue;
    if (localizedSearchHaystack(item).includes(normalizedQuery)) appendSearchCandidate(merged, seen, item);
  }
  return merged.slice(0, limit);
}

function compactGraphLabel(item: AnyItem): string {
  const kind = text(item.cluster_kind || item.node_type || item.predicate_id);
  if (text(item.node_id)) return short(displayTitle(item), 30);
  if (kind.startsWith("material-")) return short(displayTitle(item), 30);
  if (kind === "corpus") return short(displayTitle(item), 28);
  if (kind === "candidate-node") return short(displayTitle(item), 30);
  if (kind === "concept-problem") return short(displayTitle(item), 18);
  if (kind) return short(humanKind(kind), 18);
  return short(displayTitle(item), 18);
}

function relationRouteText(item: AnyItem): string {
  let from = localizedContentText(text(item.from_label || endpointLabel(item.from_id)), state.language);
  let to = localizedContentText(text(item.to_label || endpointLabel(item.to_id)), state.language);
  if (from === to && text(item.from_id) !== text(item.to_id)) {
    const items = currentItemsById();
    const fromItem = items.get(text(item.from_id));
    const toItem = items.get(text(item.to_id));
    if (fromItem) from = `${from} (${displaySubtitle(fromItem).toLowerCase()})`;
    if (toItem) to = `${to} (${displaySubtitle(toItem).toLowerCase()})`;
  }
  return `${from} -> ${relationDisplayLabel(item)} -> ${to}`;
}

async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, options);
  if (!response.ok) throw new Error(await response.text());
  return (await response.json()) as T;
}

const webActions = createToSWebActions(fetchJson);

function isPhilosophyView(payload: PhilosophyViewPayload | CorpusViewPayload | null): payload is PhilosophyViewPayload {
  return Boolean(payload && state.mode === "philosophy");
}

function itemLayers(item: AnyItem): string[] {
  const layers = item.graph_layers;
  return Array.isArray(layers) ? layers.map(text) : [];
}

function itemProperties(item: AnyItem): AnyItem {
  const source = unwrapItem(item);
  return source.properties && typeof source.properties === "object" && !Array.isArray(source.properties)
    ? (source.properties as AnyItem)
    : {};
}

const hiddenPublicNodeTypes = new Set([
  "source_planting",
  "prepared-dossier",
  "master-table-row",
  "master-table",
  "atlas-relation-kind",
  "atlas-node-type",
  "candidate-endpoint",
]);

const hiddenPublicClusterKinds = new Set(["canon-candidate-status"]);

const hiddenPublicPredicates = new Set([
  "contains_row",
  "contains_view",
  "has_prepared_dossier",
  "has_node_type_pressure",
  "has_relation_pressure",
]);

function isPublicSourceNote(item: AnyItem): boolean {
  const source = unwrapItem(item);
  const properties = itemProperties(source);
  const nodeType = text(source.node_type);
  if (nodeType === "material-reference" || nodeType === "material-representation") return true;
  return nodeType === "material-event" && text(properties.event_space) === "corpus_provenance";
}

function isPublicAtlasItem(item: AnyItem): boolean {
  const source = unwrapItem(item);
  const properties = itemProperties(source);
  const posture = text(properties.knowledge_posture).toLowerCase();
  const representationLayer = text(properties.representation_layer).toLowerCase();
  if (posture === "candidate") return false;
  if (itemLayers(source).includes("material-candidate")) return false;
  if (properties.projection_placeholder === true) return false;
  if (representationLayer === "view_projection") return false;
  if (isPublicSourceNote(source)) return false;
  if (hiddenPublicNodeTypes.has(text(source.node_type))) return false;
  if (hiddenPublicClusterKinds.has(text(source.cluster_kind))) return false;
  if (hiddenPublicPredicates.has(predicateId(source))) return false;
  return true;
}

function propertyText(item: AnyItem, key: string): string {
  return text(itemProperties(item)[key]);
}

function localizedProperty(item: AnyItem, ruKey: string, enKey: string): string {
  const properties = itemProperties(item);
  const preferred = state.language === "ru" ? properties[ruKey] : properties[enKey];
  const fallback = state.language === "ru" ? properties[enKey] : properties[ruKey];
  return localizedContentText(text(preferred || fallback), state.language).trim();
}

function relationDisplayLabel(item: AnyItem): string {
  const display = itemProperties(item).display;
  if (display && typeof display === "object" && !Array.isArray(display)) {
    const fields = display as AnyItem;
    const preferred = state.language === "ru" ? fields.label_ru : fields.label_en;
    const fallback = state.language === "ru" ? fields.label_en : fields.label_ru;
    const label = localizedContentText(text(preferred || fallback), state.language).trim();
    if (label) return label;
  }
  return humanKind(item.primary_predicate || item.predicate_id || "relation");
}

function itemNarrative(item: AnyItem): string {
  const summary = localizedProperty(item, "public_summary_ru", "public_summary_en");
  if (summary) return summary;
  const display = itemProperties(item).display;
  if (display && typeof display === "object" && !Array.isArray(display)) {
    const fields = display as AnyItem;
    const preferred = state.language === "ru" ? fields.hover_ru : fields.hover_en;
    const fallback = state.language === "ru" ? fields.hover_en : fields.hover_ru;
    return localizedContentText(text(preferred || fallback), state.language).trim();
  }
  return "";
}

function projectPublicPhilosophyPayload(payload: PhilosophyViewPayload): PhilosophyViewPayload {
  const nodes = (payload.nodes || []).filter(isPublicAtlasItem);
  const nodeIds = new Set(nodes.map((node) => node.node_id));
  const edges = (payload.edges || []).filter(
    (edge) => isPublicAtlasItem(edge) && nodeIds.has(edge.from_id) && nodeIds.has(edge.to_id),
  );
  const edgeIds = new Set(edges.map((edge) => edge.edge_id));
  const clusters = (payload.clusters || [])
    .filter(isPublicAtlasItem)
    .map((cluster) => ({
      ...cluster,
      member_node_ids: stringList(cluster.member_node_ids).filter((nodeId) => nodeIds.has(nodeId)),
      member_edge_ids: stringList(cluster.member_edge_ids).filter((edgeId) => edgeIds.has(edgeId)),
    }))
    .filter((cluster) => (cluster.member_node_ids || []).length > 0);
  return { ...payload, nodes, edges, clusters };
}

function stringList(value: unknown): string[] {
  return Array.isArray(value) ? value.map(text).filter(Boolean) : [];
}

function layerAllowed(item: AnyItem): boolean {
  if (state.mode !== "philosophy" || !isPhilosophyView(state.currentView)) return true;
  const layers = itemLayers(item);
  return layers.some((layer) => state.activeLayers.has(layer));
}

function predicateId(item: AnyItem): string {
  return text(item.predicate_id || item.primary_predicate || "relation");
}

function currentPredicates(): string[] {
  if (state.mode !== "philosophy" || !isPhilosophyView(state.currentView)) return [];
  const predicates = new Set<string>();
  (state.currentView.edges || []).forEach((edge) => {
    if (isPublicAtlasItem(edge) && layerAllowed(edge)) predicates.add(predicateId(edge));
  });
  return [...predicates].sort();
}

function predicateAllowed(item: AnyItem): boolean {
  if (state.mode !== "philosophy" || !isPhilosophyView(state.currentView) || (state.currentView.edges || []).length === 0) return true;
  return state.activePredicates.has(predicateId(item));
}

function currentItemsById(): Map<string, AnyItem> {
  const items = new Map<string, AnyItem>();
  if (!state.currentView) return items;
  (state.currentView.nodes || []).forEach((node) => items.set(node.node_id, node));
  if (state.mode === "philosophy" && isPhilosophyView(state.currentView)) {
    (state.currentView.clusters || []).filter(isPublicAtlasItem).forEach((cluster) => items.set(cluster.cluster_id, cluster));
  }
  return items;
}

function endpointLabel(id: unknown): string {
  const key = text(id);
  const item = currentItemsById().get(key);
  return item ? displayTitle(item) : key;
}

function relationAllowed(item: AnyItem): boolean {
  if (state.mode === "corpus") return true;
  return isPublicAtlasItem(item) && layerAllowed(item) && predicateAllowed(item);
}

function relationLimit(): number {
  if (state.rendererMode === "cosmos") {
    if (state.densityMode === "dense") return 12000;
    if (state.densityMode === "focused") return 6000;
    return 2200;
  }
  if (state.densityMode === "dense") return 520;
  if (state.densityMode === "focused") return 280;
  return 150;
}

function edgeLimit(): number {
  if (state.rendererMode === "cosmos") {
    if (state.densityMode === "dense") return 60000;
    if (state.densityMode === "focused") return 30000;
    return 12000;
  }
  if (state.densityMode === "dense") return 3200;
  if (state.densityMode === "focused") return 1800;
  return 900;
}

function nodeLimit(): number {
  if (state.rendererMode === "cosmos") {
    if (state.densityMode === "dense") return 50000;
    if (state.densityMode === "focused") return 22000;
    return 9000;
  }
  return 1200;
}

function clusterLimit(): number {
  if (state.rendererMode === "cosmos") return 8000;
  return 360;
}

function corpusItemLimit(): number {
  if (state.rendererMode === "cosmos") return 12000;
  return 700;
}

function relationCountAllowed(item: AnyItem): boolean {
  const count = Number(item.relation_count || 1);
  return !Number.isFinite(count) || count >= state.minRelationCount;
}

function inspectorSelectionAllowed(): boolean {
  return Date.now() >= ignoreInspectorSelectionsUntil;
}

function countBy<T>(items: T[], key: (item: T) => string): Map<string, number> {
  const counts = new Map<string, number>();
  items.forEach((item) => {
    const value = key(item);
    if (!value) return;
    counts.set(value, (counts.get(value) || 0) + 1);
  });
  return counts;
}

function colorFor(item: AnyItem, index: number): string {
  const kind = text(item.cluster_kind || item.node_type || item.predicate_id);
  if (kind.includes("source") || kind.includes("corpus")) return palette.blue;
  if (kind.includes("canon") || kind.includes("candidate")) return palette.gold;
  if (kind.includes("evidence") || kind.includes("unresolved")) return palette.red;
  if (kind.includes("concept") || kind.includes("lineage")) return palette.violet;
  return [palette.default, palette.blue, palette.gold, palette.violet, palette.grey][index % 5];
}

function edgeColorFor(item: AnyItem): string {
  const predicate = text(item.predicate_id || item.primary_predicate || "");
  const layers = itemLayers(item).join(" ");
  const signal = `${predicate} ${layers}`;
  if (signal.includes("source") || signal.includes("prepared") || signal.includes("contains")) return "rgba(66,111,163,0.72)";
  if (signal.includes("canon") || signal.includes("candidate")) return "rgba(177,116,47,0.74)";
  if (signal.includes("evidence") || signal.includes("unresolved")) return "rgba(163,72,63,0.7)";
  if (signal.includes("concept") || signal.includes("lineage")) return "rgba(118,95,162,0.72)";
  return "rgba(36,120,101,0.5)";
}

function relationWeight(item: AnyItem): number {
  const count = Number(item.relation_count || item.member_count || item.count || 1);
  if (!Number.isFinite(count) || count <= 1) return 1.05;
  return Math.min(5.2, 1.05 + Math.log2(count + 1) * 0.52);
}

function renderShell(): void {
  appRoot.innerHTML = `
    <div class="app-shell reader-site ${state.inspectorOpen ? "inspector-open" : ""}">
      <header class="reader-header">
        <a class="reader-brand" href="/" aria-label="${t("brand.title")}">
          <span class="brand-mark" aria-hidden="true"><i></i><i></i><i></i></span>
          <span><strong class="brand-title">${t("brand.title")}</strong><small class="brand-note">${t("brand.note")}</small></span>
        </a>
        <div class="reader-search">
          <input id="search" type="search" placeholder="${t("search.placeholder")}" />
          <button id="search-button" type="button" aria-label="${t("button.search")}">↵</button>
        </div>
        <div class="language-toggle" aria-label="${t("language.label")}">
          <button id="language-en" type="button">EN</button>
          <button id="language-ru" type="button">RU</button>
        </div>
      </header>
      <nav class="reader-nav" aria-label="${t("section.views")}">
        <div class="mode-switch">
          <button id="mode-philosophy" class="mode-button" type="button">${t("mode.philosophy")}</button>
          <button id="mode-corpus" class="mode-button" type="button">${t("mode.corpus")}</button>
        </div>
        <div id="view-list" class="view-list"></div>
      </nav>
      <main class="main-stage">
        <section class="graph-wrap sky-stage" aria-label="${t("brand.note")}">
          <div class="sky-atmosphere" aria-hidden="true"></div>
          <div id="graph"></div>
          <div class="sky-intro">
            <span class="sky-kicker">${t("brand.note")}</span>
            <h1 id="current-view-title"></h1>
            <p id="current-view-subtitle"></p>
          </div>
          <div id="metrics" class="sky-metrics"></div>
          <div class="graph-tools">
            <div class="graph-view-toggle">
              <button id="clusters-button" type="button">${t("button.clusters")}</button>
              <button id="nodes-button" type="button">${t("button.nodes")}</button>
            </div>
            <button id="fit-button" type="button">${t("button.fit")}</button>
            <button id="focus-clear-button" type="button">${t("button.fullView")}</button>
            <details class="map-settings">
              <summary aria-label="${t("section.layers")}">⋯</summary>
              <div class="map-settings-panel">
                <div class="section-title">${t("section.layers")}</div>
                <div id="layer-list" class="stack"></div>
                <div class="section-title">${t("section.relations")}</div>
                <div id="relation-controls" class="relation-controls"></div>
                <div class="section-title">${t("section.scaleExport")}</div>
                <div id="scale-export-controls" class="scale-export-controls"></div>
              </div>
            </details>
          </div>
          <button id="inspector-open" class="inspector-open-button" type="button">${t("inspector.open")} <span>↗</span></button>
          <div id="graph-empty" class="graph-empty" hidden>${t("empty.graph")}</div>
          <div id="graph-caption" class="graph-caption"></div>
          <div id="node-tooltip" class="node-tooltip" hidden></div>
        </section>
      </main>
      <aside class="right-rail reader-inspector" aria-label="${t("inspector.selection")}">
        <div class="inspector-head">
          <span class="inspector-kicker">${t("inspector.selection")}</span>
          <button id="inspector-close" type="button" aria-label="${t("inspector.close")}">×</button>
          <h2 id="inspector-title" class="inspector-title">${t("inspector.selection")}</h2>
          <div id="inspector-meta" class="muted">${t("inspector.nothing")}</div>
        </div>
        <div class="inspector-scroll">
          <div id="detail-list" class="detail-grid"></div>
        </div>
      </aside>
    </div>
  `;

  graphContainer = document.querySelector<HTMLDivElement>("#graph");
  nodeTooltip = document.querySelector<HTMLDivElement>("#node-tooltip");
  bindShellEvents();
}

function bindShellEvents(): void {
  byId("language-en").addEventListener("click", () => setLanguage("en"));
  byId("language-ru").addEventListener("click", () => setLanguage("ru"));
  byId("mode-philosophy").addEventListener("click", () => void loadMode("philosophy"));
  byId("mode-corpus").addEventListener("click", () => void loadMode("corpus"));
  byId("clusters-button").addEventListener("click", () => {
    state.expandedCluster = null;
    state.graphMode = "clusters";
    renderAll();
    syncPublicRoute();
  });
  byId("nodes-button").addEventListener("click", () => {
    state.graphMode = "nodes";
    renderAll();
    syncPublicRoute();
  });
  byId("fit-button").addEventListener("click", () => fitActiveGraph());
  byId("focus-clear-button").addEventListener("click", clearFocus);
  byId("inspector-open").addEventListener("click", () => setInspectorOpen(true));
  byId("inspector-close").addEventListener("click", () => setInspectorOpen(false));
  byId("search-button").addEventListener("click", () => void search());
  const searchInput = byId("search") as HTMLInputElement;
  searchInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") void search();
  });
  graphContainer?.addEventListener("pointermove", (event) => {
    lastPointer.x = event.clientX;
    lastPointer.y = event.clientY;
    if (hoveredNodeId) positionNodeTooltip();
  });
  graphContainer?.addEventListener("pointerleave", hideNodeTooltip);
  graphContainer?.addEventListener("wheel", hideNodeTooltip, { passive: true });
}

function byId(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (!element) throw new Error(`missing #${id}`);
  return element;
}

function setActive(id: string, active: boolean): void {
  byId(id).classList.toggle("active", active);
}

function setLanguage(language: Language): void {
  if (state.language === language) return;
  state.language = language;
  window.localStorage.setItem("tos-graph-language", language);
  document.documentElement.lang = language;
  renderShell();
  renderAll();
  syncPublicRoute();
}

function syncPublicRoute(): void {
  const url = new URL(window.location.href);
  url.searchParams.set("mode", state.mode);
  if (state.currentViewId) url.searchParams.set("view", state.currentViewId);
  else url.searchParams.delete("view");
  url.searchParams.set("graph", state.graphMode);
  url.searchParams.set("lang", state.language);
  url.searchParams.set("ui", state.language);
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

function renderChips(): void {
  setActive("language-en", state.language === "en");
  setActive("language-ru", state.language === "ru");
  setActive("mode-philosophy", state.mode === "philosophy");
  setActive("mode-corpus", state.mode === "corpus");
  setActive("clusters-button", state.graphMode === "clusters");
  setActive("nodes-button", state.graphMode === "nodes");
  const focusButton = byId("focus-clear-button") as HTMLButtonElement;
  focusButton.disabled = !state.neighborhood && !state.pathPacket && !state.expandedCluster;
  focusButton.classList.toggle("active", Boolean(state.neighborhood || state.pathPacket || state.expandedCluster));
  document.querySelector(".reader-site")?.classList.toggle("inspector-open", state.inspectorOpen);
}

function renderMetrics(): void {
  const current = state.currentView;
  const entries: [number, string][] = isPhilosophyView(current)
    ? [
        [(current.nodes || []).filter(isPublicAtlasItem).length, state.language === "ru" ? "объектов" : "objects"],
        [(current.edges || []).filter(isPublicAtlasItem).length, state.language === "ru" ? "связей" : "relations"],
        [(current.clusters || []).filter(isPublicAtlasItem).length, state.language === "ru" ? "областей" : "regions"],
      ]
    : [[current?.items?.length || 0, state.language === "ru" ? "материалов" : "materials"]];
  byId("metrics").innerHTML = entries.map(([value, label]) => `<span><strong>${value}</strong> ${label}</span>`).join("");
}

function renderViews(): void {
  const views = state.mode === "philosophy" ? state.philosophyViews : state.corpusViews;
  byId("view-list").innerHTML = views
    .map(
      (view) => `
        <button class="view-card ${view.view_id === state.currentViewId ? "active" : ""}" data-view="${view.view_id}" type="button" title="${escapeHtml(viewDisplaySubtitle(view))}">
          <span class="view-title">${short(viewDisplayTitle(view), 72)}</span>
        </button>
      `,
    )
    .join("");
  byId("view-list").querySelectorAll<HTMLButtonElement>("[data-view]").forEach((button) => {
    button.addEventListener("click", () => void loadView(button.dataset.view || ""));
  });
  const current = state.currentView?.view || views.find((view) => view.view_id === state.currentViewId);
  byId("current-view-title").textContent = viewDisplayTitle(current);
  byId("current-view-subtitle").textContent = viewDisplaySubtitle(current);
}

function renderLayers(): void {
  if (state.mode !== "philosophy" || !isPhilosophyView(state.currentView)) {
    byId("layer-list").innerHTML = `<div class="muted">${t("muted.noLayerCorpus")}</div>`;
    return;
  }
  const layers = state.currentView.view.graph_layers || [];
  if (layers.length === 0) {
    byId("layer-list").innerHTML = `<div class="muted">${t("muted.noLayers")}</div>`;
    return;
  }
  byId("layer-list").innerHTML = layers
    .map(
      (layer) => `
        <label class="layer-toggle ${state.activeLayers.has(layer) ? "active" : ""}">
          <input data-layer="${layer}" type="checkbox" ${state.activeLayers.has(layer) ? "checked" : ""} />
          <span>${escapeHtml(humanKind(layer))}</span>
        </label>
      `,
    )
    .join("");
  byId("layer-list").querySelectorAll<HTMLInputElement>("[data-layer]").forEach((input) => {
    input.addEventListener("change", () => {
      const layer = input.dataset.layer || "";
      if (input.checked) state.activeLayers.add(layer);
      else state.activeLayers.delete(layer);
      invalidateFocusedPackets();
      renderAll();
    });
  });
}

function renderRelationControls(): void {
  const root = byId("relation-controls");
  if (state.mode !== "philosophy" || !isPhilosophyView(state.currentView)) {
    root.innerHTML = `<div class="muted">${t("muted.noRelationCorpus")}</div>`;
    return;
  }
  const layerEdges = (state.currentView.edges || []).filter(layerAllowed);
  const activeEdges = layerEdges.filter(predicateAllowed);
  const predicateCounts = countBy(layerEdges, predicateId);
  const predicates = [...predicateCounts.entries()].sort((left, right) => right[1] - left[1]);
  root.innerHTML = `
    <div class="relation-summary">
      <strong>${activeEdges.length}</strong>
      <span>${t("relation.of")} ${layerEdges.length} ${t("relation.relations")}</span>
    </div>
    <div class="density-row">
      ${(["overview", "focused", "dense"] as DensityMode[])
        .map(
          (mode) => `
            <button class="density-button ${state.densityMode === mode ? "active" : ""}" data-density="${mode}" type="button">
              ${humanKind(mode)}
            </button>
          `,
        )
        .join("")}
    </div>
    <div class="threshold-row">
      <button id="relation-min-dec" type="button">-</button>
      <span>${t("relation.min")} ${state.minRelationCount}</span>
      <button id="relation-min-inc" type="button">+</button>
      <button id="predicate-reset" type="button">${t("relation.all")}</button>
    </div>
    <div class="predicate-list">
      ${predicates
        .map(
          ([predicate, count]) => `
            <label class="predicate-toggle ${state.activePredicates.has(predicate) ? "active" : ""}">
              <input data-predicate="${escapeHtml(predicate)}" type="checkbox" ${
                state.activePredicates.has(predicate) ? "checked" : ""
              } />
              <span>${escapeHtml(humanKind(predicate))}</span>
              <small>${count}</small>
            </label>
          `,
        )
        .join("")}
    </div>
  `;

  root.querySelectorAll<HTMLButtonElement>("[data-density]").forEach((button) => {
    button.addEventListener("click", () => {
      state.densityMode = (button.dataset.density || "overview") as DensityMode;
      renderAll();
    });
  });
  root.querySelectorAll<HTMLInputElement>("[data-predicate]").forEach((input) => {
    input.addEventListener("change", () => {
      const predicate = input.dataset.predicate || "";
      if (input.checked) state.activePredicates.add(predicate);
      else state.activePredicates.delete(predicate);
      invalidateFocusedPackets();
      renderAll();
    });
  });
  root.querySelector<HTMLButtonElement>("#relation-min-dec")?.addEventListener("click", () => {
    state.minRelationCount = Math.max(1, state.minRelationCount - 1);
    renderAll();
  });
  root.querySelector<HTMLButtonElement>("#relation-min-inc")?.addEventListener("click", () => {
    state.minRelationCount = Math.min(50, state.minRelationCount + 1);
    renderAll();
  });
  root.querySelector<HTMLButtonElement>("#predicate-reset")?.addEventListener("click", () => {
    state.activePredicates = new Set(currentPredicates());
    invalidateFocusedPackets();
    renderAll();
  });
}

function scaleExportQuery(): string {
  const params = new URLSearchParams();
  if (state.currentViewId) params.set("view_id", state.currentViewId);
  const layers = [...state.activeLayers].filter(Boolean);
  params.set("layers", layers.length ? layers.join(",") : "__tos_none__");
  const query = params.toString();
  return query ? `?${query}` : "";
}

function scaleExportPath(table?: ScaleExportTable, format?: "csv" | "jsonl"): string {
  const suffix = table && format ? `/${table}.${format}` : "/manifest";
  return `/api/philosophy/scale-export${suffix}${scaleExportQuery()}`;
}

function scaleExportAbsoluteUrl(table?: ScaleExportTable, format?: "csv" | "jsonl"): string {
  return new URL(scaleExportPath(table, format), window.location.origin).toString();
}

function renderScaleExportControls(): void {
  const root = byId("scale-export-controls");
  if (state.mode !== "philosophy" || !isPhilosophyView(state.currentView)) {
    root.innerHTML = `<div class="muted">${t("muted.scaleCorpus")}</div>`;
    return;
  }
  const layers = [...state.activeLayers].filter(Boolean);
  root.innerHTML = `
    <div class="export-summary">
      <strong>${escapeHtml(state.currentViewId || "view")}</strong>
      <span>${escapeHtml(layers.length ? layers.map(humanKind).join(", ") : t("export.noLayers"))}</span>
    </div>
    <div class="export-actions">
      <a class="export-link" data-export-link="contracts" href="/api/philosophy/contracts" target="_blank" rel="noreferrer">${t("button.contracts")}</a>
      <a class="export-link" data-export-link="manifest" href="${escapeHtml(scaleExportPath())}" target="_blank" rel="noreferrer">${t("button.manifest")}</a>
      <button data-copy-export="manifest" type="button">${t("button.copyUrl")}</button>
    </div>
    <div class="export-table-list">
      ${scaleExportTables
        .map(
          ({ table, titleKey }) => `
            <div class="export-row">
              <span>${escapeHtml(t(titleKey))}</span>
              <a class="export-link" data-export-link="${table}-csv" href="${escapeHtml(scaleExportPath(table, "csv"))}" target="_blank" rel="noreferrer">CSV</a>
              <a class="export-link" data-export-link="${table}-jsonl" href="${escapeHtml(scaleExportPath(table, "jsonl"))}" target="_blank" rel="noreferrer">JSONL</a>
              <button data-copy-export="${table}" type="button">${t("button.copy")}</button>
            </div>
          `,
        )
        .join("")}
    </div>
  `;
  root.querySelectorAll<HTMLButtonElement>("[data-copy-export]").forEach((button) => {
    button.addEventListener("click", () => {
      const table = button.dataset.copyExport || "";
      if (table === "manifest") void copyScaleExportUrl();
      else void copyScaleExportUrl(table as ScaleExportTable, "jsonl");
    });
  });
}

function renderInspector(): void {
  const title = state.selected ? displayTitle(state.selected) : viewDisplayTitle(state.currentView?.view) || state.currentViewId || t("inspector.selection");
  byId("inspector-title").textContent = title;
  byId("inspector-meta").textContent = state.selected
    ? displaySubtitle(state.selected)
    : t("inspector.help");

  const cards: string[] = [];
  const selectedRelationRows = state.selected ? relationRowsForSelection(state.selected) : [];
  const selectedNodeId = state.selected ? selectedNodeIdFor(state.selected) : "";
  if (state.selected) {
    const source = unwrapItem(state.selected);
    const narrative = itemNarrative(source);
    if (narrative && !(source.from_id && source.to_id)) {
      cards.push(`<div class="detail-card lead-card"><span class="detail-title">${t("detail.overview")}</span><span class="detail-body">${escapeHtml(narrative)}</span></div>`);
    }
    if (
      selectedNodeId &&
      state.mode === "philosophy" &&
      state.activeLayers.size > 0 &&
      state.activePredicates.size > 0
    ) {
      cards.push(nodeRouteActions(selectedNodeId));
    }
    cards.push(...relationDetailCards(state.selected));
    if (selectedRelationRows.length) {
      cards.push(...relationReadingCards(selectedRelationRows));
      cards.push(relationRowsSection(t("detail.selectedRelations"), selectedRelationRows));
    }
    if (selectedNodeId) {
      cards.push(...neighborhoodCards(selectedNodeId));
      cards.push(...pathCards(selectedNodeId));
    }
    const refs = collectRefs(state.selected);
    const noteList = sourceNoteList(state.selected);
    const sourceDetails = [
      noteList,
      sourceReferenceList(refs),
    ].filter(Boolean).join("");
    if (sourceDetails) {
      cards.push(`<details class="source-disclosure"><summary><span>${t("detail.sourceDisclosure")}</span><small>${t("detail.sourceDisclosureNote")}</small></summary><div class="source-disclosure-body">${sourceDetails}</div></details>`);
    }
  }
  if (state.results.length) {
    cards.push(`<div class="section-title">${t("detail.results")}</div>`);
    cards.push(
      ...state.results.slice(0, 48).map(
        (item, index) => `
          <button class="result-card" data-result="${index}" type="button">
            <span class="result-title">${escapeHtml(short(displayTitle(item), 82))}</span>
            <span class="result-subtitle">${escapeHtml(short(displaySubtitle(item), 98))}</span>
          </button>
        `,
      ),
    );
  }
  if (state.relationItems.length) {
    cards.push(`<div class="section-title">${t("detail.relations")}</div>`);
    cards.push(
      ...state.relationItems.slice(0, 48).map(
        (item, index) => `
          <button class="result-card relation-card" data-relation="${index}" type="button">
            <span class="result-title">${escapeHtml(short(displayTitle(item), 86))}</span>
            <span class="result-subtitle">${escapeHtml(short(displaySubtitle(item), 102))}</span>
          </button>
        `,
      ),
    );
  }
  byId("detail-list").innerHTML = cards.join("") || detailCard(t("detail.noDetail"), t("detail.noDetailBody"));
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-result]").forEach((button) => {
    button.addEventListener("click", () => {
      if (inspectorSelectionAllowed()) selectItem(state.results[Number(button.dataset.result)]);
    });
  });
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-relation]").forEach((button) => {
    button.addEventListener("click", () => {
      if (inspectorSelectionAllowed()) selectItem(state.relationItems[Number(button.dataset.relation)]);
    });
  });
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-selected-relation]").forEach((button) => {
    button.addEventListener("click", () => {
      if (inspectorSelectionAllowed()) selectItem(selectedRelationRows[Number(button.dataset.selectedRelation)]);
    });
  });
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-neighbor]").forEach((button) => {
    const item = state.neighborhood?.neighbors?.[Number(button.dataset.neighbor)];
    button.addEventListener("click", () => {
      if (item && inspectorSelectionAllowed()) selectItem(item);
    });
  });
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-path-node]").forEach((button) => {
    const item = state.pathPacket?.nodes?.[Number(button.dataset.pathNode)];
    button.addEventListener("click", () => {
      if (item && inspectorSelectionAllowed()) selectItem(item);
    });
  });
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-path-edge]").forEach((button) => {
    const item = state.pathPacket?.edges?.[Number(button.dataset.pathEdge)];
    button.addEventListener("click", () => {
      if (item && inspectorSelectionAllowed()) selectItem(item);
    });
  });
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-neighborhood-edge]").forEach((button) => {
    const item = state.neighborhood?.edges?.[Number(button.dataset.neighborhoodEdge)];
    button.addEventListener("click", () => {
      if (item && inspectorSelectionAllowed()) selectItem(item);
    });
  });
  document.getElementById("neighborhood-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    void showNeighborhood(selectedNodeId);
  });
  document.getElementById("path-start-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setPathStart(selectedNodeId);
  });
  document.getElementById("path-to-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    void showPathTo(selectedNodeId);
  });
}

function setInspectorOpen(open: boolean): void {
  state.inspectorOpen = open;
  renderChips();
  if (open) requestAnimationFrame(scrollInspectorTop);
}

function scrollInspectorTop(): void {
  document.querySelector<HTMLDivElement>(".inspector-scroll")?.scrollTo({ top: 0 });
}

function detailCard(title: string, body: string, pre = false): string {
  const safeTitle = escapeHtml(title);
  const safeBody = escapeHtml(body);
  return `
    <div class="detail-card">
      <span class="detail-title">${safeTitle}</span>
      ${pre ? `<pre>${safeBody}</pre>` : `<span class="detail-body">${safeBody}</span>`}
    </div>
  `;
}

function relationDetailCards(item: AnyItem): string[] {
  const source = unwrapItem(item);
  if (!source.from_id || !source.to_id) return [];
  const predicates = item.predicates && typeof item.predicates === "object" ? (item.predicates as Record<string, unknown>) : {};
  const predicateText = Object.entries(predicates)
    .sort((left, right) => Number(right[1]) - Number(left[1]))
    .slice(0, 8)
    .map(([predicate, count]) => `${humanKind(predicate)}: ${count}`)
    .join("\n");
  const from = localizedContentText(text(source.from_label || endpointLabel(source.from_id)), state.language);
  const to = localizedContentText(text(source.to_label || endpointLabel(source.to_id)), state.language);
  const cards = [
    detailCard(t("detail.relationRoute"), `${from}\n-> ${relationDisplayLabel(source)}\n-> ${to}`),
    detailCard(t("detail.from"), from),
    detailCard(t("detail.to"), to),
    detailCard(t("detail.predicate"), relationDisplayLabel(source)),
  ];
  const narrative = itemNarrative(source);
  if (narrative) cards.splice(1, 0, detailCard(state.language === "ru" ? "Что означает связь" : "What the relation means", narrative));
  if (source.relation_count) cards.push(detailCard(t("detail.relationCount"), text(source.relation_count)));
  if (predicateText) cards.push(detailCard(t("detail.predicateMix"), predicateText));
  return cards;
}

function selectedNodeIdFor(item: AnyItem): string {
  const source = unwrapItem(item);
  return text(source.node_id);
}

function collectRefs(item: AnyItem): string[] {
  const source = unwrapItem(item);
  const refs = new Set<string>();
  for (const key of ["source_ref", "source_path", "path"]) {
    if (source[key]) refs.add(text(source[key]));
  }
  const sourceRefs = source.source_refs;
  if (Array.isArray(sourceRefs)) sourceRefs.forEach((ref) => refs.add(text(ref)));
  return [...refs].filter((ref) => {
    if (/^https?:\/\//i.test(ref)) return true;
    if (!ref || ref.startsWith("/") || ref.includes("\\") || /[\u0000-\u001f]/.test(ref)) return false;
    const segments = ref.split("/");
    return segments.length > 1 && segments.every((segment) => segment && segment !== "." && segment !== "..");
  });
}

function sourceReferenceList(refs: string[]): string {
  if (!refs.length) return "";
  const rows = refs.slice(0, 8).map((ref) => {
    if (!/^https?:\/\//i.test(ref)) {
      return `<span class="source-reference-text"><span>${escapeHtml(ref)}</span></span>`;
    }
    let label = ref;
    try {
      const url = new URL(ref);
      label = `${url.hostname}${url.pathname === "/" ? "" : url.pathname}`;
    } catch {}
    return `<a href="${escapeHtml(ref)}" target="_blank" rel="noreferrer"><span>${escapeHtml(label)}</span><span aria-hidden="true">↗</span></a>`;
  }).join("");
  return `<div class="source-reference-list"><small>${t("detail.sourceRefs")}</small>${rows}</div>`;
}

function sourceNotesFor(item: AnyItem): AnyItem[] {
  const source = unwrapItem(item);
  const selectedIds = new Set([text(source.node_id), ...stringList(source.member_node_ids)].filter(Boolean));
  if (!selectedIds.size) return [];
  const noteIds = new Set(state.sourceNotes.map((note) => note.node_id));
  const connected = new Set<string>();
  state.sourceNoteEdges.forEach((edge) => {
    if (selectedIds.has(edge.from_id) && noteIds.has(edge.to_id)) connected.add(edge.to_id);
    if (selectedIds.has(edge.to_id) && noteIds.has(edge.from_id)) connected.add(edge.from_id);
  });
  return state.sourceNotes.filter((note) => connected.has(note.node_id));
}

function sourceNoteList(item: AnyItem): string {
  const notes = sourceNotesFor(item).slice(0, 8);
  if (!notes.length) return "";
  const rows = notes.map((note) => {
    const properties = itemProperties(note);
    const access = properties.access && typeof properties.access === "object" && !Array.isArray(properties.access)
      ? properties.access as AnyItem
      : {};
    const bibliography = properties.bibliographic && typeof properties.bibliographic === "object" && !Array.isArray(properties.bibliographic)
      ? properties.bibliographic as AnyItem
      : {};
    const description = properties.description && typeof properties.description === "object" && !Array.isArray(properties.description)
      ? properties.description as AnyItem
      : {};
    const noteText = text(
      bibliography.citation ||
      (state.language === "ru" ? description.ru : description.en) ||
      (state.language === "ru" ? description.en : description.ru) ||
      humanKind(note.node_type),
    );
    const label = escapeHtml(displayTitle(note));
    const body = escapeHtml(short(noteText, 170));
    const url = text(access.url);
    const title = url
      ? `<a href="${escapeHtml(url)}" target="_blank" rel="noreferrer">${label}<span aria-hidden="true">↗</span></a>`
      : `<span>${label}</span>`;
    return `<div class="source-note">${title}${body ? `<small>${body}</small>` : ""}</div>`;
  }).join("");
  return `<div class="source-note-list">${rows}</div>`;
}

function relationReadingCards(rows: RelationRow[]): string[] {
  const counts = countBy(rows, (row) => row.direction || "adjacent");
  const predicates = countBy(rows, (row) => predicateId(row));
  const predicateText = [...predicates.entries()]
    .sort((left, right) => right[1] - left[1])
    .slice(0, 8)
    .map(([predicate, count]) => `${humanKind(predicate)}: ${count}`)
    .join("\n");
  const summary = [
    `${humanKind("outgoing")}: ${counts.get("outgoing") || 0}`,
    `${humanKind("incoming")}: ${counts.get("incoming") || 0}`,
    `${humanKind("internal")}: ${counts.get("internal") || 0}`,
    `${humanKind("adjacent")}: ${counts.get("adjacent") || 0}`,
  ].join("\n");
  return [detailCard(t("detail.relationReading"), summary), predicateText ? detailCard(t("detail.predicatesNearby"), predicateText) : ""].filter(Boolean);
}

function relationRowsSection(title: string, rows: RelationRow[], source: "selected" | "neighborhood" | "path" = "selected"): string {
  const grouped = relationRowsByDirection(rows);
  const actionAttr =
    source === "neighborhood" ? "data-neighborhood-edge" : source === "path" ? "data-path-edge" : "data-selected-relation";
  return `
    <div class="section-title">${escapeHtml(title)}</div>
    <div class="relation-table">
      ${grouped
        .map(
          (group) => `
            <div class="relation-group-label">${escapeHtml(group.title)}</div>
            ${group.rows
              .map(({ row, index }) => {
          const meta = [
            humanKind(predicateId(row)),
            row.relation_count ? `${text(row.relation_count)} ${t("relation.relations")}` : "",
          ]
            .filter(Boolean)
            .join(" · ");
          return `
            <button class="relation-row" ${actionAttr}="${index}" type="button">
              <span class="relation-direction ${row.direction || "adjacent"}">${escapeHtml(humanKind(row.direction || "adjacent"))}</span>
              <span class="relation-route">${escapeHtml(short(relationRouteText(row), 116))}</span>
              <span class="relation-meta">${escapeHtml(short(meta, 128))}</span>
            </button>
          `;
        })
        .join("")}
          `,
        )
        .join("")}
    </div>
  `;
}

function relationRowsByDirection(rows: RelationRow[]): { title: string; rows: { row: RelationRow; index: number }[] }[] {
  const labels: Record<RelationDirection, string> = {
    outgoing: humanKind("outgoing"),
    incoming: humanKind("incoming"),
    internal: humanKind("internal"),
    adjacent: humanKind("adjacent"),
  };
  const order: RelationDirection[] = ["outgoing", "incoming", "internal", "adjacent"];
  const indexed = rows.slice(0, 100).map((row, index) => ({ row, index }));
  return order
    .map((direction) => ({
      title: labels[direction],
      rows: indexed.filter((item) => (item.row.direction || "adjacent") === direction),
    }))
    .filter((group) => group.rows.length > 0);
}

function relationRowsForSelection(item: AnyItem): RelationRow[] {
  const source = unwrapItem(item);
  if (!state.currentView) return [];
  const edges = (state.currentView.edges || []).filter(relationAllowed);
  const byEdgeId = new Map(edges.map((edge) => [edge.edge_id, edge]));
  if (source.from_id && source.to_id) {
    const memberRows = stringList(source.member_edge_ids)
      .map((edgeId) => byEdgeId.get(edgeId))
      .filter((edge): edge is GraphEdge => Boolean(edge))
      .map((edge) => relationRowFromEdge(edge, "adjacent"));
    if (memberRows.length) return memberRows;
    return [relationRowFromEdge(source as GraphEdge, "adjacent")];
  }

  const selectedIds = new Set<string>();
  if (source.node_id) selectedIds.add(text(source.node_id));
  if (source.cluster_id) selectedIds.add(text(source.cluster_id));
  stringList(source.member_node_ids).forEach((nodeId) => selectedIds.add(nodeId));
  if (selectedIds.size === 0) return [];

  return edges
    .filter((edge) => selectedIds.has(edge.from_id) || selectedIds.has(edge.to_id))
    .map((edge) => {
      const fromSelected = selectedIds.has(edge.from_id);
      const toSelected = selectedIds.has(edge.to_id);
      const direction: RelationDirection = fromSelected && toSelected ? "internal" : fromSelected ? "outgoing" : toSelected ? "incoming" : "adjacent";
      return relationRowFromEdge(edge, direction);
    })
    .sort((left, right) => {
      const order: Record<RelationDirection, number> = { outgoing: 0, incoming: 1, internal: 2, adjacent: 3 };
      return order[left.direction || "adjacent"] - order[right.direction || "adjacent"] || predicateId(left).localeCompare(predicateId(right));
    })
    .slice(0, 160);
}

function relationRowFromEdge(edge: GraphEdge, direction: RelationDirection): RelationRow {
  return {
    ...edge,
    direction,
    from_label: text(edge.from_label || endpointLabel(edge.from_id)),
    to_label: text(edge.to_label || endpointLabel(edge.to_id)),
    source_refs: collectRefs(edge),
  };
}

function showNodeTooltip(nodeId: string): void {
  const item = lastGraphItems.get(nodeId);
  if (!nodeTooltip || !item) return;
  hoveredNodeId = nodeId;
  const memberIds = item.member_node_ids;
  const members = Array.isArray(memberIds) ? `${memberIds.length} ${t("detail.members")}` : "";
  const meta = [displaySubtitle(item), members].filter(Boolean).join(" · ");
  const narrative = itemNarrative(item);
  nodeTooltip.innerHTML = `
    <div class="node-tooltip-title">${escapeHtml(displayTitle(item))}</div>
    ${meta ? `<div class="node-tooltip-meta">${escapeHtml(meta)}</div>` : ""}
    ${narrative ? `<div class="node-tooltip-summary">${escapeHtml(narrative)}</div>` : ""}
  `;
  nodeTooltip.hidden = false;
  positionNodeTooltip();
}

function hideNodeTooltip(): void {
  hoveredNodeId = null;
  if (nodeTooltip) nodeTooltip.hidden = true;
}

function positionNodeTooltip(): void {
  if (!nodeTooltip || nodeTooltip.hidden) return;
  const rect = nodeTooltip.parentElement?.getBoundingClientRect();
  if (!rect) return;
  const margin = 10;
  const width = nodeTooltip.offsetWidth || 320;
  const height = nodeTooltip.offsetHeight || 140;
  const preferredLeft = lastPointer.x - rect.left + 16;
  const preferredTop = lastPointer.y - rect.top + 16;
  const left = Math.max(margin, Math.min(preferredLeft, rect.width - width - margin));
  const top = Math.max(margin, Math.min(preferredTop, rect.height - height - margin));
  nodeTooltip.style.left = `${left}px`;
  nodeTooltip.style.top = `${top}px`;
}

function graphFocus(): { nodes: Set<string>; edges: Set<string> } | null {
  const selected = state.selectedGraphId;
  if (state.pathPacket?.found) {
    const pathNodes = new Set(stringList(state.pathPacket.nodes?.map((node) => node.node_id)));
    const pathEdges = new Set(stringList(state.pathPacket.edges?.map((edge) => edge.edge_id)));
    if (pathNodes.size || pathEdges.size) return { nodes: pathNodes, edges: pathEdges };
  }
  if (state.neighborhood?.node) {
    const nodes = new Set<string>([
      state.neighborhood.node.node_id,
      ...stringList(state.neighborhood.neighbors?.map((node) => node.node_id)),
    ]);
    const edges = new Set<string>(stringList(state.neighborhood.edges?.map((edge) => edge.edge_id)));
    if (nodes.size || edges.size) return { nodes, edges };
  }
  if (!selected) return null;
  const nodes = new Set<string>();
  const edges = new Set<string>();
  if (graph.hasEdge(selected)) {
    const [source, target] = graph.extremities(selected);
    nodes.add(source);
    nodes.add(target);
    edges.add(selected);
    return { nodes, edges };
  }
  if (!graph.hasNode(selected)) return null;
  nodes.add(selected);
  graph.forEachEdge((edge, _attributes, source, target) => {
    if (source === selected || target === selected) {
      nodes.add(source);
      nodes.add(target);
      edges.add(edge);
    }
  });
  return { nodes, edges };
}

function renderGraph(): void {
  if (!graphContainer) return;
  hideNodeTooltip();
  destroyGraphRenderers();
  graph.clear();
  lastGraphItems = new Map();
  state.relationItems = [];

  if (!state.currentView) {
    setGraphEmpty(true, t("empty.noView"));
    return;
  }

  if (state.mode === "corpus") buildCorpusGraph();
  else if (state.graphMode === "clusters") buildClusterGraph();
  else buildNodeGraph();

  if (graph.order === 0) {
    setGraphEmpty(true, t("empty.filters"));
    return;
  }

  setGraphEmpty(false);
  const renderVersion = graphRenderVersion;
  if (state.rendererMode === "cosmos") void renderCosmosGraph(renderVersion);
  else renderSigmaGraph();
}

function destroyGraphRenderers(): void {
  graphRenderVersion += 1;
  renderer?.kill();
  renderer = null;
  cosmosRenderer?.destroy();
  cosmosRenderer = null;
  cosmosPointItems = [];
  cosmosLinkItems = [];
  if (graphContainer) graphContainer.innerHTML = "";
}

function fitActiveGraph(): void {
  if (state.rendererMode === "cosmos") {
    cosmosRenderer?.fitView(260, 0.18, false);
    return;
  }
  renderer?.getCamera().animatedReset({ duration: 260 });
}

function loadCosmosModule(): Promise<typeof import("@cosmos.gl/graph")> {
  cosmosModulePromise ||= import("@cosmos.gl/graph");
  return cosmosModulePromise;
}

function renderSigmaGraph(): void {
  if (!graphContainer) return;
  const focus = graphFocus();
  const clusterOverview = state.graphMode === "clusters";
  const graphSize = graph.order;
  const completeClusterLabels = clusterOverview && graphSize <= 24;
  renderer = new Sigma(graph, graphContainer, {
    allowInvalidContainer: true,
    defaultNodeColor: palette.default,
    defaultEdgeColor: "rgba(151, 185, 197, 0.28)",
    enableEdgeEvents: true,
    labelColor: { color: "#f2eee4" },
    labelFont: 'Georgia, "Times New Roman", serif',
    labelSize: clusterOverview ? 15 : graphSize > 180 ? 12 : 14,
    labelWeight: "400",
    labelDensity: completeClusterLabels ? 0.18 : clusterOverview ? 0.055 : graphSize > 180 ? 0.01 : 0.035,
    labelGridCellSize: completeClusterLabels ? 128 : clusterOverview ? 170 : graphSize > 180 ? 190 : 150,
    labelRenderedSizeThreshold: clusterOverview ? 5 : 4.5,
    minEdgeThickness: graphSize > 180 ? 0.35 : 0.7,
    minCameraRatio: 0.08,
    maxCameraRatio: 8,
    defaultNodeType: "star",
    nodeProgramClasses: { star: StarNodeProgram },
    renderEdgeLabels: false,
    nodeReducer: (node, data) => {
      if (!focus) return completeClusterLabels ? { ...data, forceLabel: true } : data;
      if (focus.nodes.has(node)) {
        return { ...data, forceLabel: true, highlighted: true, zIndex: Number(data.zIndex || 0) + 100 };
      }
      return { ...data, label: "", color: "rgba(146, 164, 169, 0.3)", zIndex: 0 };
    },
    edgeReducer: (edge, data) => {
      if (!focus) {
        if (graphSize > 600) {
          return { ...data, size: Math.min(Number(data.size || 1), 0.42), color: "rgba(130, 166, 177, 0.075)", zIndex: 0 };
        }
        if (graphSize > 180) {
          return { ...data, size: Math.min(Number(data.size || 1), 0.58), color: "rgba(137, 174, 184, 0.12)", zIndex: 0 };
        }
        return data;
      }
      if (focus.edges.has(edge)) {
        return { ...data, size: Number(data.size || 1) * 2.5, color: "rgba(200, 218, 224, 0.82)", zIndex: 100 };
      }
      return { ...data, size: 0.3, color: "rgba(126, 151, 160, 0.16)", zIndex: 0 };
    },
    defaultDrawNodeHover: (context, data) => {
      const radius = data.size + 7;
      const inner = Math.max(3, radius * 0.24);
      context.beginPath();
      for (let index = 0; index < 16; index += 1) {
        const angle = -Math.PI / 2 + (index * Math.PI) / 8;
        const pointRadius = index % 4 === 0 ? radius : index % 2 === 0 ? inner * 1.35 : inner;
        const x = data.x + Math.cos(angle) * pointRadius;
        const y = data.y + Math.sin(angle) * pointRadius;
        if (index === 0) context.moveTo(x, y);
        else context.lineTo(x, y);
      }
      context.closePath();
      context.fillStyle = "rgba(255, 250, 232, 0.94)";
      context.fill();
      context.lineWidth = 1;
      context.strokeStyle = "rgba(255, 255, 255, 0.72)";
      context.stroke();
    },
    zIndex: true,
  });
  renderer.on("enterNode", ({ node }) => showNodeTooltip(node));
  renderer.on("leaveNode", hideNodeTooltip);
  renderer.on("enterEdge", ({ edge }) => showNodeTooltip(edge));
  renderer.on("leaveEdge", hideNodeTooltip);
  renderer.on("clickNode", ({ node }) => {
    if (Date.now() < ignoreGraphClicksUntil) return;
    const payload = lastGraphItems.get(node);
    if (payload) selectItem(payload);
  });
  renderer.on("clickEdge", ({ edge }) => {
    if (Date.now() < ignoreGraphClicksUntil) return;
    const payload = lastGraphItems.get(edge);
    if (payload) selectItem(payload);
  });
  renderer.getCamera().animatedReset({ duration: 220 });
}

type CosmosPayload = {
  nodeIds: string[];
  edgeIds: string[];
  pointPositions: Float32Array;
  pointColors: Float32Array;
  pointSizes: Float32Array;
  links: Float32Array;
  linkColors: Float32Array;
  linkWidths: Float32Array;
  linkArrows: boolean[];
  focusedPointIndex?: number;
  focusedLinkIndex?: number;
  highlightedPointIndices?: number[];
  highlightedLinkIndices?: number[];
  outlinedPointIndices?: number[];
};

async function renderCosmosGraph(renderVersion: number): Promise<void> {
  if (!graphContainer) return;
  const payload = cosmosPayloadFromGraph();
  const { Graph: CosmosGraphClass } = await loadCosmosModule();
  if (renderVersion !== graphRenderVersion || state.rendererMode !== "cosmos" || !graphContainer) return;
  const family = layoutFamily();
  const simulationEnabled = cosmosSimulationEnabled(family);
  const config: GraphConfig = {
    backgroundColor: [1, 1, 1, 0],
    spaceSize: 4096,
    randomSeed: 41,
    rescalePositions: true,
    fitViewOnInit: true,
    fitViewDelay: 140,
    fitViewDuration: 260,
    fitViewPadding: family === "timeline" || family === "flow" ? 0.06 : 0.1,
    pointDefaultColor: palette.blue,
    pointDefaultSize: 5,
    pointSizeScale: state.graphMode === "clusters" ? 0.86 : 1.22,
    pointGreyoutOpacity: 0.32,
    renderHoveredPointRing: true,
    hoveredPointRingColor: [1, 1, 1, 0.96],
    focusedPointRingColor: [0.09, 0.13, 0.12, 0.9],
    outlinedPointRingColor: [0.09, 0.13, 0.12, 0.72],
    focusedPointIndex: payload.focusedPointIndex,
    highlightedPointIndices: payload.highlightedPointIndices,
    outlinedPointIndices: payload.outlinedPointIndices,
    renderLinks: payload.links.length > 0,
    linkDefaultColor: [0.09, 0.13, 0.12, 0.22],
    linkOpacity: linkOpacityForLayout(family),
    linkGreyoutOpacity: 0.08,
    linkDefaultWidth: 0.7,
    linkWidthScale: linkWidthScaleForLayout(family),
    linkVisibilityDistanceRange: [8000, 12000],
    linkVisibilityMinTransparency: state.densityMode === "dense" ? 0.32 : 0.86,
    scaleLinksOnZoom: true,
    linkBlending: state.densityMode !== "dense",
    curvedLinks: family === "semantic" || family === "infrastructure",
    linkDefaultArrows: family === "flow" || family === "evidence",
    focusedLinkIndex: payload.focusedLinkIndex,
    highlightedLinkIndices: payload.highlightedLinkIndices,
    hoveredLinkWidthIncrease: 2,
    focusedLinkWidthIncrease: 3,
    enableDrag: true,
    enableSimulation: simulationEnabled,
    simulationGravity: family === "organic" ? 0.12 : 0.04,
    simulationCenter: simulationEnabled ? 0.05 : 0,
    simulationRepulsion: simulationEnabled ? (state.densityMode === "dense" ? 0.36 : 0.62) : 0,
    simulationLinkSpring: simulationEnabled ? 0.58 : 0,
    simulationLinkDistance: state.graphMode === "clusters" ? 26 : 18,
    simulationFriction: 0.84,
    transitionDuration: 420,
    hoveredPointCursor: "pointer",
    hoveredLinkCursor: "pointer",
    onPointClick: (index, _pointPosition, _event) => {
      if (Date.now() < ignoreGraphClicksUntil) return;
      const item = cosmosPointItems[index];
      if (item) selectItem(item);
    },
    onLinkClick: (index, _event) => {
      if (Date.now() < ignoreGraphClicksUntil) return;
      const item = cosmosLinkItems[index];
      if (item) selectItem(item);
    },
    onMouseMove: (_index, _pointPosition, event) => {
      lastPointer.x = event.clientX;
      lastPointer.y = event.clientY;
      if (hoveredNodeId) positionNodeTooltip();
    },
    onPointMouseOver: (index, _pointPosition, event) => {
      if (event && "clientX" in event) {
        lastPointer.x = event.clientX;
        lastPointer.y = event.clientY;
      }
      const nodeId = payload.nodeIds[index];
      if (nodeId) showNodeTooltip(nodeId);
    },
    onPointMouseOut: (_event) => hideNodeTooltip(),
    onLinkMouseOver: (index) => {
      const edgeId = payload.edgeIds[index];
      if (edgeId) showNodeTooltip(edgeId);
    },
    onLinkMouseOut: (_event) => hideNodeTooltip(),
  };
  cosmosRenderer = new CosmosGraphClass(graphContainer, config);
  cosmosPointItems = payload.nodeIds.map((nodeId) => lastGraphItems.get(nodeId) || { node_id: nodeId, label: nodeId });
  cosmosLinkItems = payload.edgeIds.map((edgeId) => lastGraphItems.get(edgeId) || { edge_id: edgeId, label: edgeId });
  cosmosRenderer.setPointPositions(payload.pointPositions);
  cosmosRenderer.setPointColors(payload.pointColors);
  cosmosRenderer.setPointSizes(payload.pointSizes);
  cosmosRenderer.setLinks(payload.links);
  cosmosRenderer.setLinkColors(payload.linkColors);
  cosmosRenderer.setLinkWidths(payload.linkWidths);
  cosmosRenderer.setLinkArrows(payload.linkArrows);
  cosmosRenderer.render(simulationEnabled ? undefined : 0);
}

function cosmosSimulationEnabled(family: LayoutFamily): boolean {
  if (state.graphMode === "nodes" && state.densityMode !== "overview") return true;
  return family === "semantic" || family === "organic";
}

function linkOpacityForLayout(family: LayoutFamily): number {
  if (state.densityMode === "dense") return 0.28;
  if (family === "timeline" || family === "flow") return 0.42;
  if (family === "evidence") return 0.5;
  return 0.62;
}

function linkWidthScaleForLayout(family: LayoutFamily): number {
  if (state.densityMode === "dense") return 0.72;
  if (family === "timeline" || family === "flow") return 0.82;
  if (family === "evidence") return 0.94;
  return 1.22;
}

function cosmosPayloadFromGraph(): CosmosPayload {
  const family = layoutFamily();
  const nodeIds = graph.nodes();
  const nodeIndex = new Map(nodeIds.map((nodeId, index) => [nodeId, index]));
  const pointPositions = new Float32Array(nodeIds.length * 2);
  const pointColors = new Float32Array(nodeIds.length * 4);
  const pointSizes = new Float32Array(nodeIds.length);
  nodeIds.forEach((nodeId, index) => {
    const x = numberAttr(graph.getNodeAttribute(nodeId, "x"));
    const y = numberAttr(graph.getNodeAttribute(nodeId, "y"));
    pointPositions[index * 2] = x;
    pointPositions[index * 2 + 1] = y;
    writeRgba(pointColors, index, colorToRgba(text(graph.getNodeAttribute(nodeId, "color") || palette.default)));
    const sizeMultiplier = state.graphMode === "clusters" ? 0.94 : 1.42;
    const maxSize = state.graphMode === "clusters" ? 28 : 38;
    pointSizes[index] = Math.max(4.5, Math.min(maxSize, numberAttr(graph.getNodeAttribute(nodeId, "size"), 7) * sizeMultiplier));
  });

  const edgeIds: string[] = [];
  const links: number[] = [];
  const linkColors: number[] = [];
  const linkWidths: number[] = [];
  const linkArrows: boolean[] = [];
  graph.edges().forEach((edgeId) => {
    const [source, target] = graph.extremities(edgeId);
    const sourceIndex = nodeIndex.get(source);
    const targetIndex = nodeIndex.get(target);
    if (sourceIndex === undefined || targetIndex === undefined) return;
    edgeIds.push(edgeId);
    links.push(sourceIndex, targetIndex);
    linkColors.push(...colorToRgba(text(graph.getEdgeAttribute(edgeId, "color") || palette.line)));
    const minWidth = state.densityMode === "dense" ? 0.9 : family === "timeline" || family === "flow" ? 1.15 : 1.6;
    linkWidths.push(Math.max(minWidth, Math.min(9, numberAttr(graph.getEdgeAttribute(edgeId, "size"), 1))));
    linkArrows.push(state.mode === "philosophy" && state.densityMode !== "dense");
  });

  const focus = graphFocus();
  const focusPointIndices = focus
    ? nodeIds.map((nodeId, index) => (focus.nodes.has(nodeId) ? index : -1)).filter((index) => index >= 0)
    : undefined;
  const focusLinkIndices = focus
    ? edgeIds.map((edgeId, index) => (focus.edges.has(edgeId) ? index : -1)).filter((index) => index >= 0)
    : undefined;
  const hardHighlight = Boolean(state.pathPacket?.found);
  const focusedPointIndex =
    state.selectedGraphId && nodeIndex.has(state.selectedGraphId) ? nodeIndex.get(state.selectedGraphId) : undefined;
  const focusedLinkIndex = state.selectedGraphId ? edgeIds.indexOf(state.selectedGraphId) : -1;
  return {
    nodeIds,
    edgeIds,
    pointPositions,
    pointColors,
    pointSizes,
    links: new Float32Array(links),
    linkColors: new Float32Array(linkColors),
    linkWidths: new Float32Array(linkWidths),
    linkArrows,
    focusedPointIndex,
    focusedLinkIndex: focusedLinkIndex >= 0 ? focusedLinkIndex : undefined,
    highlightedPointIndices: hardHighlight ? focusPointIndices : undefined,
    highlightedLinkIndices: hardHighlight ? focusLinkIndices : undefined,
    outlinedPointIndices: focusPointIndices,
  };
}

function numberAttr(value: unknown, fallback = 0): number {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : fallback;
}

function colorToRgba(value: string): [number, number, number, number] {
  const color = value.trim();
  if (color.startsWith("#")) {
    const hex = color.slice(1);
    if (hex.length === 6) {
      const red = parseInt(hex.slice(0, 2), 16) / 255;
      const green = parseInt(hex.slice(2, 4), 16) / 255;
      const blue = parseInt(hex.slice(4, 6), 16) / 255;
      return [red, green, blue, 1];
    }
  }
  const rgba = color.match(/rgba?\\(([^)]+)\\)/i);
  if (rgba) {
    const parts = rgba[1].split(",").map((part) => Number(part.trim()));
    const [red = 36, green = 120, blue = 101, alpha = 1] = parts;
    return [red / 255, green / 255, blue / 255, alpha > 1 ? alpha / 255 : alpha];
  }
  return [0.14, 0.47, 0.4, 1];
}

function writeRgba(target: Float32Array, index: number, rgba: [number, number, number, number]): void {
  target[index * 4] = rgba[0];
  target[index * 4 + 1] = rgba[1];
  target[index * 4 + 2] = rgba[2];
  target[index * 4 + 3] = rgba[3];
}

function setGraphEmpty(empty: boolean, message = ""): void {
  const emptyNode = byId("graph-empty");
  emptyNode.hidden = !empty;
  emptyNode.textContent = message || t("empty.graph");
  const caption = byId("graph-caption");
  const view = state.currentView?.view;
  const graphSummary = graph.order > 0 ? ` · ${graph.order} ${t("caption.nodes")} · ${graph.size} ${t("caption.links")}` : "";
  const subtitle = viewDisplaySubtitle(view);
  caption.textContent = `${viewDisplayTitle(view) || state.currentViewId || t("caption.view")}${subtitle ? ` · ${subtitle}` : ""}${graphSummary}`;
}

function buildClusterGraph(): void {
  if (!isPhilosophyView(state.currentView)) return;
  const clusters = (state.currentView.clusters || []).filter(isPublicAtlasItem).filter(layerAllowed).slice(0, clusterLimit());
  const nodes = (state.currentView.nodes || []).filter(isPublicAtlasItem).filter(layerAllowed);
  const edges = (state.currentView.edges || []).filter(relationAllowed);
  const nodeToCluster = new Map<string, string[]>();
  clusters.forEach((cluster) => {
    stringList(cluster.member_node_ids).forEach((nodeId) => {
      const ids = nodeToCluster.get(nodeId) || [];
      ids.push(cluster.cluster_id);
      nodeToCluster.set(nodeId, ids);
    });
  });

  clusters.forEach((cluster, index) => {
    addGraphNode(cluster.cluster_id, cluster, index, Math.max(5.5, Math.min(12, 5.5 + (cluster.member_node_ids?.length || 0) * 0.04)));
  });
  const relationBuild = addClusterRelationEdges(edges, nodeToCluster, clusters);
  const relationPairs = relationBuild.relationPairs;
  const linked = new Set<string>();
  if (state.densityMode !== "overview") {
    nodes.slice(0, state.densityMode === "dense" ? 1200 : 700).forEach((node) => {
      const ids = nodeToCluster.get(node.node_id) || [];
      for (let i = 0; i < ids.length; i += 1) {
        for (let j = i + 1; j < ids.length; j += 1) {
          const key = [ids[i], ids[j]].sort().join("::");
          if (linked.has(key)) continue;
          linked.add(key);
          if (relationPairs.has(key)) continue;
          graph.addDirectedEdgeWithKey(`cluster-edge:${key}`, ids[i], ids[j], {
            size: 0.5,
            color: "rgba(23,32,29,0.1)",
          });
        }
      }
    });
  }
  layoutGraph();
  state.results = clusters;
  state.relationItems = relationBuild.relationItems;
}

function addClusterRelationEdges(
  edges: GraphEdge[],
  nodeToCluster: Map<string, string[]>,
  clusters: Cluster[],
): { relationPairs: Set<string>; relationItems: AnyItem[] } {
  const clusterById = new Map(clusters.map((cluster) => [cluster.cluster_id, cluster]));
  const aggregates = new Map<
    string,
    {
      from_id: string;
      to_id: string;
      relation_count: number;
      predicates: Map<string, number>;
      graph_layers: Set<string>;
      source_refs: Set<string>;
      member_edge_ids: string[];
    }
  >();
  const relationPairs = new Set<string>();
  const relationItems: AnyItem[] = [];

  edges.forEach((edge) => {
    const fromClusterIds = nodeToCluster.get(edge.from_id) || [];
    const toClusterIds = nodeToCluster.get(edge.to_id) || [];
    fromClusterIds.forEach((fromId) => {
      toClusterIds.forEach((toId) => {
        if (fromId === toId) return;
        const key = `${fromId}->${toId}`;
        const existing =
          aggregates.get(key) ||
          {
            from_id: fromId,
            to_id: toId,
            relation_count: 0,
            predicates: new Map<string, number>(),
            graph_layers: new Set<string>(),
            source_refs: new Set<string>(),
            member_edge_ids: [],
          };
        existing.relation_count += 1;
        const predicate = edge.predicate_id || "relation";
        existing.predicates.set(predicate, (existing.predicates.get(predicate) || 0) + 1);
        itemLayers(edge).forEach((layer) => existing.graph_layers.add(layer));
        collectRefs(edge).forEach((ref) => existing.source_refs.add(ref));
        if (edge.edge_id) existing.member_edge_ids.push(edge.edge_id);
        aggregates.set(key, existing);
        relationPairs.add([fromId, toId].sort().join("::"));
      });
    });
  });

  [...aggregates.values()]
    .filter(relationCountAllowed)
    .sort((left, right) => right.relation_count - left.relation_count)
    .slice(0, relationLimit())
    .forEach((aggregate, index) => {
      const predicates = Object.fromEntries(aggregate.predicates.entries());
      const primaryPredicate =
        [...aggregate.predicates.entries()].sort((left, right) => right[1] - left[1])[0]?.[0] || "relation";
      const fromCluster = clusterById.get(aggregate.from_id);
      const toCluster = clusterById.get(aggregate.to_id);
      const payload: AnyItem = {
        edge_id: `cluster-relation:${index}`,
        from_id: aggregate.from_id,
        to_id: aggregate.to_id,
        predicate_id: primaryPredicate,
        primary_predicate: primaryPredicate,
        relation_count: aggregate.relation_count,
        predicates,
        graph_layers: [...aggregate.graph_layers],
        source_refs: [...aggregate.source_refs],
        member_edge_ids: aggregate.member_edge_ids.slice(0, 80),
        from_label: displayTitle(fromCluster || { label: aggregate.from_id }),
        to_label: displayTitle(toCluster || { label: aggregate.to_id }),
        label: `${displayTitle(fromCluster || { label: aggregate.from_id })} -> ${displayTitle(toCluster || { label: aggregate.to_id })}`,
      };
      addGraphEdge(payload.edge_id as string, aggregate.from_id, aggregate.to_id, payload);
      relationItems.push(payload);
    });

  return { relationPairs, relationItems };
}

function buildNodeGraph(): void {
  if (!isPhilosophyView(state.currentView)) return;
  const focusPacket = focusedNodePacket();
  const expandedIds = new Set(state.expandedCluster?.member_node_ids || []);
  const sourceNodes = focusPacket?.nodes || state.currentView.nodes || [];
  const sourceEdges = focusPacket?.edges || state.currentView.edges || [];
  const nodes = sourceNodes
    .filter(isPublicAtlasItem)
    .filter(layerAllowed)
    .filter((node) => focusPacket || expandedIds.size === 0 || expandedIds.has(node.node_id))
    .slice(0, nodeLimit());
  const visible = new Set(nodes.map((node) => node.node_id));
  nodes.forEach((node, index) => addGraphNode(node.node_id, node, index, 5));
  const visibleEdges = sourceEdges
    .filter(relationAllowed)
    .filter((edge) => visible.has(edge.from_id) && visible.has(edge.to_id))
    .slice(0, edgeLimit());
  visibleEdges.forEach((edge, index) => addGraphEdge(edge.edge_id || `edge:${index}`, edge.from_id, edge.to_id, edge));
  layoutGraph();
  state.results = nodes;
  state.relationItems = visibleEdges.slice(0, 180);
}

function focusedNodePacket(): { nodes: GraphNode[]; edges: GraphEdge[] } | null {
  if (state.pathPacket?.found && state.pathPacket.nodes?.length) {
    return {
      nodes: (state.pathPacket.nodes || []).filter(isPublicAtlasItem),
      edges: (state.pathPacket.edges || []).filter(isPublicAtlasItem),
    };
  }
  if (state.neighborhood?.node) {
    return {
      nodes: [state.neighborhood.node, ...(state.neighborhood.neighbors || [])].filter(isPublicAtlasItem),
      edges: (state.neighborhood.edges || []).filter(isPublicAtlasItem),
    };
  }
  return null;
}

function buildCorpusGraph(): void {
  const payload = state.currentView as CorpusViewPayload;
  const items = (payload.items || []).filter(isPublicAtlasItem).slice(0, corpusItemLimit());
  const coreNodes = (payload.nodes || []).filter((node) => Boolean(node.node_id));
  const coreEdges = (payload.edges || []).filter((edge) => Boolean(edge.from_id && edge.to_id));
  if (coreNodes.length && coreEdges.length) {
    coreNodes.forEach((node, index) => addGraphNode(node.node_id, node, index, node.node_type === "corpus-root" ? 10 : 5));
    coreEdges.forEach((edge, index) => addGraphEdge(edge.edge_id || `corpus-relation:${index}`, edge.from_id, edge.to_id, edge));
    layoutGraph();
    state.results = items;
    state.relationItems = coreEdges;
    return;
  }
  const relations = items.filter((item) => Boolean(item.from_id && item.to_id));
  const resources = items.filter((item) => !(item.from_id && item.to_id));
  const projectedNodes = new Map(
    (payload.nodes || []).filter((node) => node.node_id).map((node) => [node.node_id, node]),
  );
  const rootId = `view:${state.currentViewId}`;
  if (resources.length || relations.length === 0) addGraphNode(rootId, payload.view, 0, 10);
  resources.forEach((item, index) => {
    const id = itemId(item);
    addGraphNode(id, item, index + 1, 5);
    addGraphEdge(`corpus-edge:${rootId}:${id}`, rootId, id, { edge_id: `corpus-edge:${id}`, predicate_id: "contains", ...item });
  });
  const endpointIds = new Set<string>();
  relations.forEach((item, index) => {
    const fromId = text(item.from_id);
    const toId = text(item.to_id);
    for (const [id, label] of [
      [fromId, text(item.from_label || endpointLabel(fromId) || fromId)],
      [toId, text(item.to_label || endpointLabel(toId) || toId)],
    ]) {
      if (endpointIds.has(id)) continue;
      endpointIds.add(id);
      addGraphNode(
        id,
        projectedNodes.get(id) || { node_id: id, label, owner_branch: item.owner_branch },
        resources.length + endpointIds.size,
        5,
      );
    }
    addGraphEdge(text(item.edge_id || `corpus-relation:${index}`), fromId, toId, item);
  });
  layoutGraph();
  state.results = items;
  state.relationItems = relations;
}

function renderGraphPreservingInspectorLists(): void {
  const results = state.results;
  const relationItems = state.relationItems;
  renderGraph();
  state.results = results;
  state.relationItems = relationItems;
}

function addGraphNode(id: string, item: AnyItem, index: number, size: number): void {
  if (graph.hasNode(id)) return;
  lastGraphItems.set(id, item);
  graph.addNode(id, {
    label: compactGraphLabel(item),
    size,
    color: colorFor(item, index),
    type: "star",
    x: Math.cos(index) * (1 + index / 40),
    y: Math.sin(index) * (1 + index / 40),
    zIndex: size,
  });
}

function addGraphEdge(id: string, from: string, to: string, item: AnyItem): void {
  if (!graph.hasNode(from) || !graph.hasNode(to) || graph.hasEdge(id)) return;
  lastGraphItems.set(id, item);
  graph.addDirectedEdgeWithKey(id, from, to, {
    size: relationWeight(item),
    color: edgeColorFor(item),
    label: humanKind(predicateId(item)),
  });
}

function hashNumber(value: string): number {
  return value.split("").reduce((acc, char) => (acc * 31 + char.charCodeAt(0)) % 100000, 17);
}

function layoutFamily(): LayoutFamily {
  const viewId = state.currentViewId;
  const hint = text(state.currentView?.view?.layout_hint).toLowerCase();
  if (viewId === "chronology" || hint.includes("timeline") || hint.includes("lane")) return "timeline";
  if (viewId === "transmission" || viewId === "transmission_map" || viewId === "canon-promotion" || hint.includes("directed") || hint.includes("flow") || hint.includes("corridor") || hint.includes("promotion")) return "flow";
  if (
    viewId === "source-evidence" ||
    viewId === "evidence_lab" ||
    viewId === "script-decipherment" ||
    viewId === "lost-corpus" ||
    hint.includes("dag") ||
    hint.includes("evidence") ||
    hint.includes("uncertainty") ||
    hint.includes("absence")
  ) {
    return "evidence";
  }
  if (viewId === "concept-lineage" || hint.includes("semantic") || hint.includes("lineage")) return "semantic";
  if (
    viewId === "institution-media" ||
    viewId === "human_atlas" ||
    viewId === "imperial-multilingualism" ||
    viewId === "ritual-law" ||
    viewId === "epigraphic-network" ||
    hint.includes("infrastructure") ||
    hint.includes("parallel") ||
    hint.includes("ritual") ||
    hint.includes("law") ||
    hint.includes("distributed")
  ) {
    return "infrastructure";
  }
  return "organic";
}

function dossierOrdinal(item: AnyItem, fallback: number): number {
  const source = unwrapItem(item);
  const raw = [
    propertyText(source, "dossier_id"),
    propertyText(source, "atlas_row_id"),
    propertyText(source, "candidate_id"),
    propertyText(source, "original_node_id"),
    text(source.node_id || source.cluster_id || source.edge_id || source.label),
  ].join(" ");
  const tableThree = raw.match(/T3[-_ ]?(\d+)/i);
  if (tableThree) return 3000 + Number(tableThree[1]);
  const tableTwo = raw.match(/T2[-_ ]?(\d+)/i);
  if (tableTwo) return 2000 + Number(tableTwo[1]);
  const dossier = raw.match(/\bA(\d{1,3})\b/i);
  if (dossier) return Number(dossier[1]);
  return fallback;
}

function normalized(value: number, min: number, max: number): number {
  if (!Number.isFinite(value) || max <= min) return 0;
  return ((value - min) / (max - min)) * 2 - 1;
}

function laneForItem(item: AnyItem, fallback: number): number {
  const signal = text(item.cluster_kind || item.node_type || item.primary_predicate || item.predicate_id || item.label || item.title);
  if (signal.includes("canon") || signal.includes("candidate")) return 0;
  if (signal.includes("source") || signal.includes("corpus")) return 1;
  if (signal.includes("concept") || signal.includes("lineage")) return 2;
  if (signal.includes("evidence") || signal.includes("unresolved")) return 3;
  return fallback % 5;
}

function flowLaneForItem(item: AnyItem, fallback: number): number {
  const signal = `${text(item.cluster_kind || item.node_type || item.primary_predicate || item.predicate_id || item.label || item.title)} ${itemLayers(item).join(" ")}`.toLowerCase();
  if (signal.includes("source") || signal.includes("witness") || signal.includes("evidence")) return 0;
  if (signal.includes("corpus") || signal.includes("dossier") || signal.includes("prepared")) return 1;
  if (signal.includes("candidate") || signal.includes("concept")) return 2;
  if (signal.includes("transmission") || signal.includes("preserved") || signal.includes("survives") || signal.includes("medium")) return 3;
  if (signal.includes("canon") || signal.includes("status") || signal.includes("promotion")) return 4;
  return fallback % 5;
}

function evidenceLaneForItem(item: AnyItem, fallback: number): number {
  const signal = `${text(item.cluster_kind || item.node_type || item.primary_predicate || item.predicate_id || item.label || item.title)} ${propertyText(item, "original_node_type")} ${itemLayers(item).join(" ")}`.toLowerCase();
  if (signal.includes("source") || signal.includes("document") || signal.includes("witness")) return 0;
  if (signal.includes("preservation") || signal.includes("survives") || signal.includes("preserved")) return 1;
  if (signal.includes("evidence") || signal.includes("dossier") || signal.includes("corpus")) return 2;
  if (signal.includes("contested") || signal.includes("controversy") || signal.includes("uncertainty")) return 3;
  if (signal.includes("lost") || signal.includes("absence") || signal.includes("fragment")) return 4;
  return fallback % 5;
}

function semanticLaneForItem(item: AnyItem, fallback: number): number {
  const signal = `${text(item.cluster_kind || item.node_type || item.primary_predicate || item.predicate_id || item.label || item.title)} ${propertyText(item, "original_node_type")} ${itemLayers(item).join(" ")}`.toLowerCase();
  if (signal.includes("concept") || signal.includes("problem")) return 0;
  if (signal.includes("method") || signal.includes("genre")) return 1;
  if (signal.includes("candidate")) return 2;
  if (signal.includes("canon") || signal.includes("canonical")) return 3;
  if (signal.includes("source") || signal.includes("evidence")) return 4;
  return fallback % 5;
}

function branchRegionLane(item: AnyItem, fallback: number): number {
  const branch = propertyText(item, "branch_path").toLowerCase();
  if (branch.includes("west-asia")) return 0;
  if (branch.includes("north-africa")) return 1;
  if (branch.includes("east-asia")) return 2;
  if (branch.includes("south-asia")) return 3;
  if (branch.includes("southeast-asia")) return 4;
  if (branch.includes("mediterranean")) return 5;
  return fallback % 6;
}

function layoutForceIterations(family: LayoutFamily): number {
  if (state.graphMode === "clusters") return 0;
  if (graph.order > 160) return 0;
  if (state.rendererMode === "cosmos" && family !== "organic" && family !== "semantic") return 0;
  if (family === "timeline" || family === "flow" || family === "evidence") return 10;
  if (family === "semantic") return 20;
  return 26;
}

function layoutGraph(): void {
  const count = Math.max(graph.order, 1);
  const family = layoutFamily();
  const nodes = graph.nodes();
  const ordinals = nodes.map((node, index) => dossierOrdinal(lastGraphItems.get(node) || {}, index));
  const minOrdinal = Math.min(...ordinals);
  const maxOrdinal = Math.max(...ordinals);
  const span = 1 + count / 90;
  nodes.forEach((node, index) => {
    const item = lastGraphItems.get(node) || {};
    const hash = hashNumber(node);
    const lane = laneForItem(item, hash);
    const orderX = normalized(ordinals[index], minOrdinal, maxOrdinal);
    const jitter = ((hash % 29) - 14) / 120;
    let x = 0;
    let y = 0;
    if (state.graphMode === "clusters" && count <= 24 && family !== "timeline") {
      const columns = Math.max(2, Math.ceil(Math.sqrt(count * 1.5)));
      const row = Math.floor(index / columns);
      const column = index % columns;
      const rows = Math.ceil(count / columns);
      const rowLength = Math.min(columns, count - row * columns);
      x = (column - (rowLength - 1) / 2) * 1.72 + (row % 2 === 0 ? -0.08 : 0.08);
      y = (row - (rows - 1) / 2) * 1.05 + Math.sin((column + 1) * (row + 1)) * 0.06;
    } else if (family === "timeline") {
      if (state.graphMode === "clusters" && count <= 24) {
        x = (orderX - 0.5) * Math.max(3.2, count * 0.9);
        y = (index % 2 === 0 ? -0.22 : 0.22) + ((hash % 7) - 3) / 90;
      } else {
        x = orderX * (2.7 + count / 75);
        y = (lane - 2) * 0.58 + jitter;
      }
    } else if (family === "flow") {
      const flowLane = flowLaneForItem(item, hash);
      x = (flowLane - 2) * 1.08 + jitter;
      y = orderX * (2.2 + count / 92) + ((hash % 17) - 8) / 180;
    } else if (family === "evidence") {
      const evidenceLane = evidenceLaneForItem(item, hash);
      x = (evidenceLane - 2) * 1.0 + jitter;
      y = orderX * (2.3 + count / 105) + ((hash % 23) - 11) / 180;
    } else if (family === "semantic") {
      const semanticLane = semanticLaneForItem(item, hash);
      const radius = 0.45 + semanticLane * 0.42 + count / 520;
      const angle = Math.PI * 2 * ((hash % 997) / 997);
      x = Math.cos(angle) * radius + orderX * 0.12;
      y = Math.sin(angle) * radius + (semanticLane - 2) * 0.08;
    } else if (family === "infrastructure") {
      const regionLane = branchRegionLane(item, hash);
      const mediaLane = evidenceLaneForItem(item, hash);
      x = (regionLane - 2.5) * 0.82 + jitter;
      y = (mediaLane - 2) * 0.66 + orderX * 0.72 + ((hash % 19) - 9) / 160;
    } else {
      const angle = (Math.PI * 2 * (hash % Math.max(count, 2))) / Math.max(count, 2);
      x = Math.cos(angle) * span + jitter;
      y = Math.sin(angle) * span + jitter;
    }
    graph.setNodeAttribute(node, "x", x);
    graph.setNodeAttribute(node, "y", y);
  });
  const iterations = layoutForceIterations(family);
  if (graph.order > 1 && iterations > 0) {
    forceAtlas2.assign(graph, {
      iterations,
      settings: {
        ...forceAtlas2.inferSettings(graph),
        gravity: family === "timeline" ? 0.16 : family === "flow" || family === "evidence" ? 0.12 : 0.055,
        scalingRatio: graph.order > 500 ? 7 : family === "semantic" ? 8 : 11,
      },
    });
  }
}

function selectItem(item: AnyItem): void {
  state.selected = item;
  state.inspectorOpen = true;
  state.selectedGraphId = text(item.node_id || item.cluster_id || item.edge_id || "") || null;
  const nodeId = selectedNodeIdFor(item);
  if (!nodeId || state.neighborhood?.node?.node_id !== nodeId) state.neighborhood = null;
  const pathNodeIds = new Set(stringList(state.pathPacket?.nodes?.map((node) => node.node_id)));
  if (!nodeId || (state.pathPacket && !pathNodeIds.has(nodeId))) state.pathPacket = null;
  const cluster = item as Cluster;
  if (cluster.cluster_id && cluster.member_node_ids?.length) {
    state.expandedCluster = cluster;
  }
  renderChips();
  renderGraphPreservingInspectorLists();
  renderInspector();
  scrollInspectorTop();
}

function nodeRouteActions(nodeId: string): string {
  const pathStartLabel = state.pathStartNodeId ? endpointLabel(state.pathStartNodeId) || state.pathStartNodeId : "";
  const canPathTo = Boolean(state.pathStartNodeId && state.pathStartNodeId !== nodeId);
  return `
    <div class="route-actions">
      <button id="neighborhood-button" type="button">${t("route.neighborhood")}</button>
      <button id="path-start-button" type="button">${state.pathStartNodeId === nodeId ? t("route.pathStartSet") : t("route.useAsPathStart")}</button>
      ${canPathTo ? `<button id="path-to-button" type="button">${t("route.pathFrom")} ${escapeHtml(short(pathStartLabel, 24))}</button>` : ""}
    </div>
  `;
}

function neighborhoodCards(nodeId: string): string[] {
  if (!state.neighborhood || state.neighborhood.node?.node_id !== nodeId) return [];
  const neighbors = state.neighborhood.neighbors || [];
  const edges = state.neighborhood.edges || [];
  const cards = [
    detailCard(
      t("detail.neighborhood"),
      [
        `${neighbors.length} ${t("detail.neighborCount")}`,
        `${edges.length} ${t("relation.relations")}`,
        state.neighborhood.predicates?.length ? state.neighborhood.predicates.map(humanKind).join(", ") : t("detail.allActivePredicates"),
      ]
        .filter(Boolean)
        .join("\n"),
    ),
  ];
  if (neighbors.length) {
    cards.push(`<div class="section-title">${t("detail.neighbors")}</div>`);
    cards.push(
      ...neighbors.slice(0, 24).map(
        (item, index) => `
          <button class="result-card" data-neighbor="${index}" type="button">
            <span class="result-title">${escapeHtml(short(displayTitle(item), 82))}</span>
            <span class="result-subtitle">${escapeHtml(short(displaySubtitle(item), 98))}</span>
          </button>
        `,
      ),
    );
  }
  if (edges.length) {
    cards.push(relationRowsSection(t("detail.neighborhoodRelations"), edges.map((edge) => relationRowFromEdge(edge, "adjacent")), "neighborhood"));
  }
  return cards;
}

function pathCards(nodeId: string): string[] {
  const cards: string[] = [];
  if (state.pathStartNodeId) {
    cards.push(detailCard(t("detail.pathStart"), endpointLabel(state.pathStartNodeId) || state.pathStartNodeId));
  }
  if (!state.pathPacket || (state.pathPacket.from_id !== nodeId && state.pathPacket.to_id !== nodeId)) return cards;
  const nodes = state.pathPacket.nodes || [];
  const edges = state.pathPacket.edges || [];
  cards.push(
    detailCard(
      t("detail.path"),
      state.pathPacket.found
        ? [
            `${nodes.length} ${t("caption.nodes")}`,
            `${edges.length} ${t("relation.relations")}`,
            `${t("detail.maxDepth")} ${state.pathPacket.max_depth || 6}`,
            state.pathPacket.predicates?.length ? state.pathPacket.predicates.map(humanKind).join(", ") : t("detail.allActivePredicates"),
          ]
            .filter(Boolean)
            .join("\n")
        : [
            t("detail.noRoute"),
            `${t("detail.maxDepth")} ${state.pathPacket.max_depth || 6}`,
          ]
            .filter(Boolean)
            .join("\n"),
    ),
  );
  if (nodes.length) {
    cards.push(`<div class="section-title">${t("detail.pathNodes")}</div>`);
    cards.push(
      ...nodes.map(
        (item, index) => `
          <button class="result-card" data-path-node="${index}" type="button">
            <span class="result-title">${escapeHtml(short(displayTitle(item), 82))}</span>
            <span class="result-subtitle">${escapeHtml(short(displaySubtitle(item), 98))}</span>
          </button>
        `,
      ),
    );
  }
  if (edges.length) {
    cards.push(relationRowsSection(t("detail.pathRelations"), edges.map((edge) => relationRowFromEdge(edge, "adjacent")), "path"));
  }
  return cards;
}

async function showNeighborhood(nodeId: string): Promise<void> {
  if (
    !nodeId ||
    state.mode !== "philosophy" ||
    state.activeLayers.size === 0 ||
    state.activePredicates.size === 0
  ) return;
  const requestRevision = ++neighborhoodRevision;
  const requestMode = state.mode;
  const requestViewId = state.currentViewId;
  const selected = state.selected;
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  const neighborhood = (await webActions.invoke("tos.neighborhood", {
    node_id: nodeId,
    depth: 1,
    limit: 160,
    layers: [...state.activeLayers],
    predicates: [...state.activePredicates],
  })) as NeighborhoodPayload;
  if (
    requestRevision !== neighborhoodRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestViewId
  ) return;
  state.graphMode = "nodes";
  state.expandedCluster = null;
  state.neighborhood = neighborhood;
  state.selected = selected;
  state.selectedGraphId = nodeId;
  renderChips();
  renderGraph();
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  renderInspector();
  scrollInspectorTop();
}

function setPathStart(nodeId: string): void {
  if (!nodeId) return;
  pathRevision += 1;
  state.pathStartNodeId = nodeId;
  state.pathPacket = null;
  renderInspector();
}

async function showPathTo(nodeId: string): Promise<void> {
  if (
    !nodeId ||
    !state.pathStartNodeId ||
    state.pathStartNodeId === nodeId ||
    state.mode !== "philosophy" ||
    state.activeLayers.size === 0 ||
    state.activePredicates.size === 0
  ) return;
  const requestRevision = ++pathRevision;
  const requestMode = state.mode;
  const requestViewId = state.currentViewId;
  const fromId = state.pathStartNodeId;
  const selected = state.selected;
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  const pathPacket = (await webActions.invoke("tos.path.find", {
    from_id: fromId,
    to_id: nodeId,
    max_depth: 6,
    layers: [...state.activeLayers],
    predicates: [...state.activePredicates],
  })) as PathPayload;
  if (
    requestRevision !== pathRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestViewId ||
    state.pathStartNodeId !== fromId
  ) return;
  state.graphMode = "nodes";
  state.expandedCluster = null;
  state.pathPacket = pathPacket;
  state.selected = selected;
  state.selectedGraphId = nodeId;
  renderChips();
  renderGraph();
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  renderInspector();
  scrollInspectorTop();
}

function clearFocus(): void {
  invalidateFocusedPackets();
  state.graphMode = state.mode === "philosophy" ? "clusters" : "nodes";
  renderAll();
  syncPublicRoute();
}

function invalidateFocusedPackets(): void {
  neighborhoodRevision += 1;
  pathRevision += 1;
  state.neighborhood = null;
  state.pathPacket = null;
  state.expandedCluster = null;
}

function renderAll(): void {
  renderChips();
  renderMetrics();
  renderViews();
  renderLayers();
  renderRelationControls();
  renderScaleExportControls();
  renderGraph();
  renderInspector();
}

async function copyScaleExportUrl(table?: ScaleExportTable, format?: "csv" | "jsonl"): Promise<void> {
  const url = scaleExportAbsoluteUrl(table, format);
  try {
    await navigator.clipboard.writeText(url);
    state.selected = { title: t("selection.scaleExportUrl"), url };
  } catch (error) {
    state.selected = { title: t("selection.scaleExportUrl"), url, copy_error: text(error) };
  }
  state.selectedGraphId = null;
  renderInspector();
  scrollInspectorTop();
}

async function loadMode(mode: Mode, requestedViewId = "", requestedGraphMode?: GraphMode): Promise<void> {
  const loadRevision = ++modeLoadRevision;
  searchRevision += 1;
  neighborhoodRevision += 1;
  pathRevision += 1;
  state.mode = mode;
  state.currentView = null;
  state.selected = null;
  state.selectedGraphId = null;
  state.results = [];
  state.relationItems = [];
  state.expandedCluster = null;
  state.neighborhood = null;
  state.pathStartNodeId = null;
  state.pathPacket = null;
  state.inspectorOpen = false;
  if (mode === "philosophy") {
    state.status.philosophy = await fetchJson<AnyItem>("/api/philosophy/status");
    if (loadRevision !== modeLoadRevision || state.mode !== mode) return;
    const views = await fetchJson<{ views: ViewCard[] }>("/api/philosophy/views");
    if (loadRevision !== modeLoadRevision || state.mode !== mode) return;
    state.philosophyViews = views.views || [];
    const viewId = state.philosophyViews.some((view) => view.view_id === requestedViewId)
      ? requestedViewId
      : boot.default_philosophy_view || state.philosophyViews[0]?.view_id || "";
    await loadView(viewId, requestedGraphMode);
  } else {
    state.status.corpus = await fetchJson<AnyItem>("/api/corpus/status");
    if (loadRevision !== modeLoadRevision || state.mode !== mode) return;
    const summary = await fetchJson<{ graph_views?: ViewCard[]; counts?: AnyItem }>("/api/corpus/summary");
    if (loadRevision !== modeLoadRevision || state.mode !== mode) return;
    state.status.corpus = { ...state.status.corpus, counts: summary.counts || state.status.corpus.counts };
    state.corpusViews = summary.graph_views || [];
    const viewId = state.corpusViews.some((view) => view.view_id === requestedViewId)
      ? requestedViewId
      : boot.default_view || state.corpusViews[0]?.view_id || "";
    await loadView(viewId, "nodes");
  }
}

async function loadView(viewId: string, requestedGraphMode?: GraphMode): Promise<void> {
  if (!viewId) return;
  const loadRevision = ++viewLoadRevision;
  searchRevision += 1;
  neighborhoodRevision += 1;
  pathRevision += 1;
  const loadMode = state.mode;
  state.currentViewId = viewId;
  state.currentView = null;
  state.selected = null;
  state.selectedGraphId = null;
  state.results = [];
  state.relationItems = [];
  state.expandedCluster = null;
  state.neighborhood = null;
  state.pathStartNodeId = null;
  state.pathPacket = null;
  state.inspectorOpen = false;
  if (state.mode === "philosophy") {
    const sourcePayload = (await webActions.invoke("tos.view.open", {
      mode: "philosophy",
      view_id: viewId,
      limit: 1000,
    })) as PhilosophyViewPayload;
    if (loadRevision !== viewLoadRevision || state.mode !== loadMode || state.currentViewId !== viewId) return;
    state.sourceNotes = (sourcePayload.nodes || []).filter(isPublicSourceNote);
    const sourceNoteIds = new Set(state.sourceNotes.map((node) => node.node_id));
    state.sourceNoteEdges = (sourcePayload.edges || []).filter(
      (edge) => sourceNoteIds.has(edge.from_id) || sourceNoteIds.has(edge.to_id),
    );
    const payload = projectPublicPhilosophyPayload(sourcePayload);
    state.currentView = payload;
    state.activeLayers = new Set(payload.view.graph_layers || []);
    state.activePredicates = new Set((payload.edges || []).filter(isPublicAtlasItem).map(predicateId));
    state.densityMode = "overview";
    state.minRelationCount = 1;
    state.graphMode = requestedGraphMode || "clusters";
    state.results = (payload.clusters || []).filter(isPublicAtlasItem);
  } else {
    state.sourceNotes = [];
    state.sourceNoteEdges = [];
    const payload = (await webActions.invoke("tos.view.open", {
      mode: "corpus",
      view_id: viewId,
      limit: 700,
    })) as CorpusViewPayload;
    if (loadRevision !== viewLoadRevision || state.mode !== loadMode || state.currentViewId !== viewId) return;
    state.currentView = payload;
    state.activeLayers = new Set();
    state.activePredicates = new Set();
    state.graphMode = "nodes";
    state.results = (payload.items || []).filter(isPublicAtlasItem);
  }
  renderAll();
  syncPublicRoute();
}

async function search(): Promise<void> {
  const requestRevision = ++searchRevision;
  const requestMode = state.mode;
  const requestViewId = state.currentViewId;
  const query = (byId("search") as HTMLInputElement).value.trim();
  state.searchQuery = query;
  const payload = (await webActions.invoke("tos.search", {
    mode: state.mode,
    query,
    limit: 80,
  })) as { results?: AnyItem[] };
  if (
    requestRevision !== searchRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestViewId
  ) return;
  state.results = mergeLocalizedSearchResults(query, (payload.results || []).filter(isPublicAtlasItem), 80);
  state.selected = { title: query ? `${t("selection.search")}: ${query}` : t("selection.search"), results: state.results.length };
  state.inspectorOpen = true;
  state.selectedGraphId = null;
  renderInspector();
  scrollInspectorTop();
}

renderShell();
void loadMode(initialRoute.mode, initialRoute.viewId, initialRoute.graphMode).catch((error: unknown) => {
  byId("inspector-title").textContent = t("load.failed");
  byId("inspector-meta").innerHTML = `<span class="danger">${text(error)}</span>`;
});
