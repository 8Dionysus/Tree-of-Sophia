import type { Graph as CosmosGraph, GraphConfig } from "@cosmos.gl/graph";
import Graphology from "graphology";
import forceAtlas2 from "graphology-layout-forceatlas2";
import Sigma from "sigma";
import {
  createPageCommandRegistry,
  isPageCommandCancellation,
  reloadableFocusId,
  requireKnownViewId,
  type PageCommandId,
  type PageCommandInput,
  type PageContextSnapshot,
  type PageSelection,
} from "./page-commands";
import { createToSQueryOperations } from "./query-operations";
import {
  createLocalStoragePersistence,
  createResearchWorkspace,
  type ResearchHypothesis,
  type RouteSnapshot,
} from "./research-workspace";
import { agentSurfaceState, PRODUCT_DEMO_PROMPTS } from "./product-shell";
import { createWebMCPAdapter, type WebMCPDocument } from "./webmcp";
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
  direction?: "outgoing" | "incoming" | "either";
  view_id?: string | null;
  excluded_edge_ids?: string[];
  alternative_limit?: number;
  path_count?: number;
  paths?: Array<{
    path_index: number;
    node_ids: string[];
    edge_ids: string[];
    nodes: GraphNode[];
    edges: GraphEdge[];
  }>;
  source_refs?: string[];
};

type EpistemicPayload = {
  schema?: string;
  item_id?: string;
  view_id?: string | null;
  selection?: AnyItem;
  challenge_relations?: GraphEdge[];
  context_relations?: GraphEdge[];
  neighbor_nodes?: GraphNode[];
  selection_posture?: {
    authority_posture?: string;
    canon_status?: string;
    review_posture?: string;
    confidence?: string;
    priority?: string;
    claim_evidence_closed?: boolean;
  };
  field_posture?: {
    authority_postures?: string[];
    canon_statuses?: string[];
    review_postures?: string[];
    confidence_values?: string[];
  };
  coverage?: {
    posture?: string;
    challenge_state?: string;
    available_challenge_relations?: number;
    returned_challenge_relations?: number;
    missing_surfaces?: string[];
  };
  counts?: Record<string, number>;
  source_refs?: string[];
  authority_note?: string;
  finding?: string;
  finding_ru?: string;
  posture?: string;
  conclusion?: {
    can_conclude?: boolean;
    canon_membership?: boolean;
    claim_evidence_closed?: boolean;
    allowed?: string[];
    allowed_ru?: string[];
    not_allowed?: string[];
    not_allowed_ru?: string[];
  };
  source_anchors?: Array<{
    edge_id?: string;
    anchor_segment_ids?: string[];
    witness_scope?: string;
    relation_ref?: string;
  }>;
  routes?: Array<{
    route_kind?: string;
    ref?: string;
    status?: string;
  }>;
  gaps?: string[];
  gaps_ru?: string[];
  agent_summary?: Record<string, unknown>;
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
  epistemicPacket: EpistemicPayload | null;
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
    "detail.pathAlternatives": "Path alternatives",
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
    "route.rerouteWithoutEdge": "Find alternatives without this edge",
    "route.inspectEpistemic": "Open Evidence Lens",
    "detail.evidenceFinding": "Evidence Lens finding",
    "detail.allowedConclusions": "What this route supports",
    "detail.forbiddenConclusions": "What remains unsupported",
    "detail.sourceAnchors": "Exact source anchors",
    "detail.ownerRoutes": "Owner routes",
    "detail.evidenceGaps": "Open gaps",
    "detail.epistemicPosture": "Epistemic posture",
    "detail.contextPosture": "Surrounding field posture",
    "detail.projectedChallenges": "Projected challenge signals",
    "detail.contextRelations": "Context relations",
    "detail.coverage": "Coverage",
    "detail.notAdjudicated": "These graph relations are leads for review, not adjudicated counterevidence.",
    "workspace.title": "Research workspace",
    "workspace.localOnly": "Local session only · not source · not reviewed · not canon",
    "workspace.hypothesisPlaceholder": "State a working interpretation",
    "workspace.predicatePlaceholder": "Relation label (optional)",
    "workspace.addHypothesis": "Add hypothesis",
    "workspace.excludeEdge": "Exclude selected edge",
    "workspace.saveRoute": "Save route for comparison",
    "workspace.notePlaceholder": "Add a research note",
    "workspace.addNote": "Add note",
    "workspace.undo": "Undo",
    "workspace.redo": "Redo",
    "workspace.export": "Export session",
    "workspace.import": "Import session",
    "workspace.hypotheses": "Session hypotheses",
    "workspace.comparisons": "Route comparisons",
    "workspace.exclusions": "Excluded edges",
    "workspace.notes": "Notes",
    "workspace.journal": "Recent actions",
    "workspace.empty": "The workspace is empty. Select an edge to begin a bounded investigation.",
    "workspace.compareReady": "Comparison ready",
    "agent.title": "Agent surface",
    "agent.connected": "WebMCP connected",
    "agent.connecting": "Connecting WebMCP",
    "agent.unavailable": "WebMCP unavailable",
    "agent.error": "WebMCP registration failed",
    "agent.tools": "tools available",
    "agent.selectionTools": "selection-bound",
    "agent.revision": "context revision",
    "agent.fallback": "The atlas remains fully usable. Connect an external agent through the native MCP server.",
    "agent.nativeMcp": "Native fallback: run tos mcp",
    "agent.demoTitle": "Try the research loop",
    "agent.demoLead": "Select a relation, then give one of these prompts to your agent.",
    "agent.copy": "Copy prompt",
    "agent.copied": "Copied",
    "agent.done": "Got it",
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
    "detail.pathAlternatives": "Альтернативы пути",
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
    "route.rerouteWithoutEdge": "Найти альтернативы без этой связи",
    "route.inspectEpistemic": "Открыть Evidence Lens",
    "detail.evidenceFinding": "Вывод Evidence Lens",
    "detail.allowedConclusions": "Что маршрут позволяет утверждать",
    "detail.forbiddenConclusions": "Что остаётся без основания",
    "detail.sourceAnchors": "Точные привязки к источнику",
    "detail.ownerRoutes": "Маршруты к владельцам истины",
    "detail.evidenceGaps": "Открытые пробелы",
    "detail.epistemicPosture": "Эпистемический статус",
    "detail.contextPosture": "Статус окружающего поля",
    "detail.projectedChallenges": "Проекционные сигналы спора",
    "detail.contextRelations": "Контекстные связи",
    "detail.coverage": "Полнота",
    "detail.notAdjudicated": "Эти графовые связи — маршруты для проверки, а не рассмотренное контрсвидетельство.",
    "workspace.title": "Исследовательская область",
    "workspace.localOnly": "Только локальная сессия · не источник · не review · не канон",
    "workspace.hypothesisPlaceholder": "Сформулируйте рабочую интерпретацию",
    "workspace.predicatePlaceholder": "Название отношения (необязательно)",
    "workspace.addHypothesis": "Добавить гипотезу",
    "workspace.excludeEdge": "Исключить выбранное ребро",
    "workspace.saveRoute": "Сохранить маршрут для сравнения",
    "workspace.notePlaceholder": "Добавить исследовательскую заметку",
    "workspace.addNote": "Добавить заметку",
    "workspace.undo": "Отменить",
    "workspace.redo": "Повторить",
    "workspace.export": "Экспорт сессии",
    "workspace.import": "Импорт сессии",
    "workspace.hypotheses": "Гипотезы сессии",
    "workspace.comparisons": "Сравнение маршрутов",
    "workspace.exclusions": "Исключённые рёбра",
    "workspace.notes": "Заметки",
    "workspace.journal": "Последние действия",
    "workspace.empty": "Область пуста. Выберите ребро, чтобы начать ограниченное исследование.",
    "workspace.compareReady": "Можно сравнивать",
    "agent.title": "Поверхность агента",
    "agent.connected": "WebMCP подключён",
    "agent.connecting": "WebMCP подключается",
    "agent.unavailable": "WebMCP недоступен",
    "agent.error": "Ошибка регистрации WebMCP",
    "agent.tools": "инструментов доступно",
    "agent.selectionTools": "зависят от выбора",
    "agent.revision": "ревизия контекста",
    "agent.fallback": "Атлас остаётся полностью доступным. Внешнего агента можно подключить через родной MCP-сервер.",
    "agent.nativeMcp": "Родной fallback: запустите tos mcp",
    "agent.demoTitle": "Попробуйте исследовательский цикл",
    "agent.demoLead": "Выберите связь и передайте агенту один из этих запросов.",
    "agent.copy": "Копировать запрос",
    "agent.copied": "Скопировано",
    "agent.done": "Понятно",
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
  focusId: string;
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
    focusId: params.get("focus") || "",
  };
}

const initialRoute = readInitialRoute();

function createBrowserResearchWorkspace() {
  try {
    return createResearchWorkspace({
      sessionId: "tos-local-research",
      persistence: createLocalStoragePersistence(window.localStorage, "tos-research-workspace-v1"),
    });
  } catch {
    return createResearchWorkspace({ sessionId: "tos-local-research", persistence: false });
  }
}

const researchWorkspace = createBrowserResearchWorkspace();

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
  epistemicPacket: null,
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
let epistemicRevision = 0;
let webMCP: ReturnType<typeof createWebMCPAdapter> | null = null;
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

const queryOperations = createToSQueryOperations(fetchJson);

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
  const edgeId = text(item.edge_id);
  if (edgeId && researchWorkspace.getState().excludedEdgeIds.includes(edgeId)) return false;
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
  if (item.session_hypothesis === true) return "rgba(222,118,255,0.95)";
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
  if (item.session_hypothesis === true) return 3.2;
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
        <div class="reader-header-actions">
          <details id="agent-surface" class="agent-surface">
            <summary>
              <i id="agent-surface-dot" aria-hidden="true"></i>
              <span id="agent-surface-label">${t("agent.connecting")}</span>
            </summary>
            <div id="agent-surface-body" class="agent-surface-body"></div>
          </details>
          <div class="language-toggle" aria-label="${t("language.label")}">
            <button id="language-en" type="button">EN</button>
            <button id="language-ru" type="button">RU</button>
          </div>
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
          <details id="research-workspace-panel" class="research-workspace-panel">
            <summary>
              <span>${t("workspace.title")}</span>
              <small id="research-workspace-summary"></small>
            </summary>
            <div id="research-workspace-body" class="research-workspace-body"></div>
          </details>
          <input id="research-workspace-import" type="file" accept="application/json,.json" hidden />
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

function productOnboardingSeen(): boolean {
  try {
    return window.localStorage.getItem("tos-product-onboarding-v1") === "seen";
  } catch {
    return false;
  }
}

function markProductOnboardingSeen(): void {
  try {
    window.localStorage.setItem("tos-product-onboarding-v1", "seen");
  } catch {
    // The shell remains usable when browser storage is unavailable.
  }
}

function renderAgentSurface(): void {
  const panel = document.getElementById("agent-surface") as HTMLDetailsElement | null;
  const label = document.getElementById("agent-surface-label");
  const body = document.getElementById("agent-surface-body");
  if (!panel || !label || !body || !webMCP) return;
  const model = agentSurfaceState(webMCP.status());
  panel.dataset.webmcpState = model.state;
  label.textContent = t(`agent.${model.state}`);
  if (!productOnboardingSeen() && !panel.dataset.onboardingOpened) {
    panel.open = true;
    panel.dataset.onboardingOpened = "true";
  }
  const statusDetail = model.state === "connected"
    ? `<dl class="agent-surface-metrics">
        <div><dt>${t("agent.tools")}</dt><dd id="agent-tool-count">${model.toolCount}</dd></div>
        <div><dt>${t("agent.selectionTools")}</dt><dd id="agent-selection-tool-count">${model.selectionToolCount}</dd></div>
        <div><dt>${t("agent.revision")}</dt><dd id="agent-context-revision">${model.contextRevision}</dd></div>
      </dl>`
    : `<div class="agent-fallback">
        <p>${escapeHtml(model.error || t("agent.fallback"))}</p>
        <code>${t("agent.nativeMcp")}</code>
      </div>`;
  const prompts = PRODUCT_DEMO_PROMPTS[state.language]
    .map((prompt) => `<article class="agent-prompt-card">
      <strong>${escapeHtml(prompt.title)}</strong>
      <p>${escapeHtml(prompt.prompt)}</p>
      <button type="button" data-agent-prompt="${prompt.id}">${t("agent.copy")}</button>
    </article>`)
    .join("");
  body.innerHTML = `
    <div class="agent-surface-status">
      <span>${t(`agent.${model.state}`)}</span>
      ${statusDetail}
    </div>
    <section class="agent-onboarding">
      <h3>${t("agent.demoTitle")}</h3>
      <p>${t("agent.demoLead")}</p>
      <div class="agent-prompt-list">${prompts}</div>
      <button id="agent-onboarding-done" type="button">${t("agent.done")}</button>
    </section>`;
  body.querySelectorAll<HTMLButtonElement>("[data-agent-prompt]").forEach((button) => {
    button.addEventListener("click", () => {
      const prompt = PRODUCT_DEMO_PROMPTS[state.language].find((item) => item.id === button.dataset.agentPrompt);
      if (!prompt) return;
      void navigator.clipboard.writeText(prompt.prompt).then(() => {
        markProductOnboardingSeen();
        button.textContent = t("agent.copied");
      }).catch(() => {
        button.classList.add("danger");
        button.textContent = t("agent.copy");
      });
    });
  });
  document.getElementById("agent-onboarding-done")?.addEventListener("click", () => {
    markProductOnboardingSeen();
    panel.open = false;
  });
}

function bindShellEvents(): void {
  byId("language-en").addEventListener("click", () => setLanguage("en"));
  byId("language-ru").addEventListener("click", () => setLanguage("ru"));
  byId("mode-philosophy").addEventListener("click", () => invokePageCommandFromUi("tos.page.open-view", { mode: "philosophy" }));
  byId("mode-corpus").addEventListener("click", () => invokePageCommandFromUi("tos.page.open-view", { mode: "corpus" }));
  byId("clusters-button").addEventListener("click", () => {
    state.expandedCluster = null;
    state.graphMode = "clusters";
    renderAll();
    syncPublicRoute();
    pageCommands.notifyStateChange();
  });
  byId("nodes-button").addEventListener("click", () => {
    state.graphMode = "nodes";
    renderAll();
    syncPublicRoute();
    pageCommands.notifyStateChange();
  });
  byId("fit-button").addEventListener("click", () => fitActiveGraph());
  byId("focus-clear-button").addEventListener("click", () => invokePageCommandFromUi("tos.page.clear-focus"));
  byId("inspector-open").addEventListener("click", () => setInspectorOpen(true));
  byId("inspector-close").addEventListener("click", () => setInspectorOpen(false));
  byId("search-button").addEventListener("click", () => invokePageCommandFromUi("tos.page.search", { query: searchInput.value }));
  const searchInput = byId("search") as HTMLInputElement;
  searchInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") invokePageCommandFromUi("tos.page.search", { query: searchInput.value });
  });
  graphContainer?.addEventListener("pointermove", (event) => {
    lastPointer.x = event.clientX;
    lastPointer.y = event.clientY;
    if (hoveredNodeId) positionNodeTooltip();
  });
  graphContainer?.addEventListener("pointerleave", hideNodeTooltip);
  graphContainer?.addEventListener("wheel", hideNodeTooltip, { passive: true });
  (byId("research-workspace-import") as HTMLInputElement).addEventListener("change", (event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    void file.text()
      .then((packet) => pageCommands.invoke("tos.page.workspace-import", { packet }))
      .catch((error: unknown) => showPageCommandError("tos.page.workspace-import", error))
      .finally(() => { input.value = ""; });
  });
}

function byId(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (!element) throw new Error(`missing #${id}`);
  return element;
}

function invokePageCommandFromUi(commandId: PageCommandId, input: PageCommandInput = {}): void {
  void pageCommands.invoke(commandId, input).catch((error: unknown) => {
    if (isPageCommandCancellation(error)) return;
    showPageCommandError(commandId, error);
  });
}

function showPageCommandError(commandId: PageCommandId, error: unknown): void {
  console.error(`Tree of Sophia page command failed: ${commandId}`, error);
  const title = document.getElementById("inspector-title");
  const meta = document.getElementById("inspector-meta");
  if (title) title.textContent = t("load.failed");
  if (meta) meta.innerHTML = `<span class="danger">${escapeHtml(text(error))}</span>`;
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
  renderAgentSurface();
  syncPublicRoute();
  pageCommands.notifyStateChange();
}

function syncPublicRoute(): void {
  const url = new URL(window.location.href);
  url.searchParams.set("mode", state.mode);
  if (state.currentViewId) url.searchParams.set("view", state.currentViewId);
  else url.searchParams.delete("view");
  url.searchParams.set("graph", state.graphMode);
  url.searchParams.set("lang", state.language);
  url.searchParams.set("ui", state.language);
  const current = state.currentView as AnyItem | null;
  const reloadableIds = current
    ? [
        ...((current.nodes as AnyItem[] | undefined) || []),
        ...((current.edges as AnyItem[] | undefined) || []),
        ...((current.clusters as AnyItem[] | undefined) || []),
        ...((current.items as AnyItem[] | undefined) || []),
      ].map(itemId)
    : [];
  const routeFocusId = reloadableFocusId(pageSelection(), state.selectedGraphId, reloadableIds);
  if (routeFocusId) url.searchParams.set("focus", routeFocusId);
  else url.searchParams.delete("focus");
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
    button.addEventListener("click", () => invokePageCommandFromUi("tos.page.open-view", {
      mode: state.mode,
      view_id: button.dataset.view || "",
      graph_mode: state.graphMode,
    }));
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
      pageCommands.notifyStateChange();
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
      pageCommands.notifyStateChange();
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
    pageCommands.notifyStateChange();
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
    const epistemicItemId = text(source.node_id || source.edge_id || "");
    const narrative = itemNarrative(source);
    if (source.session_hypothesis === true) {
      cards.push(`<div class="detail-card workspace-hypothesis-card"><span class="detail-title">${t("workspace.hypotheses")}</span><span class="detail-body">${escapeHtml(t("workspace.localOnly"))}</span></div>`);
    }
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
    if (
      state.mode === "philosophy" &&
      state.activeLayers.size > 0 &&
      state.activePredicates.size > 0 &&
      source.from_id &&
      source.to_id &&
      source.edge_id &&
      !isAggregateRelation(source) &&
      source.session_hypothesis !== true
    ) {
      cards.push(`<div class="route-actions"><button id="reroute-without-edge-button" type="button">${t("route.rerouteWithoutEdge")}</button></div>`);
    }
    if (
      (state.mode === "philosophy" || (state.mode === "corpus" && state.currentViewId === "route-graph")) &&
      epistemicItemId &&
      !isAggregateRelation(source) &&
      source.session_hypothesis !== true
    ) {
      cards.push(`<div class="route-actions"><button id="epistemic-button" type="button">${t("route.inspectEpistemic")}</button></div>`);
      cards.push(...epistemicCards(epistemicItemId));
    }
    cards.push(...relationDetailCards(state.selected));
    if (selectedRelationRows.length) {
      cards.push(...relationReadingCards(selectedRelationRows));
      cards.push(relationRowsSection(t("detail.selectedRelations"), selectedRelationRows));
    }
    if (selectedNodeId) {
      cards.push(...neighborhoodCards(selectedNodeId));
      cards.push(...pathCards(selectedNodeId));
    } else if (
      typeof source.from_id === "string" &&
      typeof source.to_id === "string" &&
      state.pathPacket?.from_id === source.from_id &&
      state.pathPacket.to_id === source.to_id
    ) {
      cards.push(...pathCards(source.to_id));
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
  byId("detail-list").querySelectorAll<HTMLButtonElement>("[data-epistemic-edge]").forEach((button) => {
    const item = state.epistemicPacket?.challenge_relations?.[Number(button.dataset.epistemicEdge)];
    button.addEventListener("click", () => {
      if (item && inspectorSelectionAllowed()) selectItem(item);
    });
  });
  document.getElementById("neighborhood-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    invokePageCommandFromUi("tos.page.show-neighborhood", { node_id: selectedNodeId });
  });
  document.getElementById("path-start-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    invokePageCommandFromUi("tos.page.start-path", { node_id: selectedNodeId });
  });
  document.getElementById("path-to-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    invokePageCommandFromUi("tos.page.find-path", { to_id: selectedNodeId });
  });
  document.getElementById("reroute-without-edge-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    invokePageCommandFromUi("tos.page.reroute-without-selection", {
      direction: "outgoing",
      alternative_limit: 3,
      constrain_to_view: true,
    });
  });
  document.getElementById("epistemic-button")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    invokePageCommandFromUi("tos.page.inspect-epistemic", { item_id: pageSelection()?.id });
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

function relationRowsSection(title: string, rows: RelationRow[], source: "selected" | "neighborhood" | "path" | "epistemic" = "selected"): string {
  const grouped = relationRowsByDirection(rows);
  const actionAttr =
    source === "neighborhood"
      ? "data-neighborhood-edge"
      : source === "path"
        ? "data-path-edge"
        : source === "epistemic"
          ? "data-epistemic-edge"
          : "data-selected-relation";
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

  addResearchHypothesisOverlays();

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
      if (data.sessionHypothesis === true) {
        return { ...data, size: Math.max(Number(data.size || 1), 3.2), color: "rgba(229,136,255,0.96)", zIndex: 220 };
      }
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
  const coreEdges = (payload.edges || []).filter((edge) => Boolean(edge.from_id && edge.to_id)).filter(relationAllowed);
  if (coreNodes.length && coreEdges.length) {
    coreNodes.forEach((node, index) => addGraphNode(node.node_id, node, index, node.node_type === "corpus-root" ? 10 : 5));
    coreEdges.forEach((edge, index) => addGraphEdge(edge.edge_id || `corpus-relation:${index}`, edge.from_id, edge.to_id, edge));
    layoutGraph();
    state.results = items;
    state.relationItems = coreEdges;
    return;
  }
  const relations = items.filter((item) => Boolean(item.from_id && item.to_id)).filter(relationAllowed);
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
    sessionHypothesis: item.session_hypothesis === true,
  });
}

function addResearchHypothesisOverlays(): void {
  researchWorkspace.getState().hypotheses.forEach((hypothesis) => {
    if (!hypothesis.fromId || !hypothesis.toId) return;
    const payload: AnyItem = {
      edge_id: hypothesis.id,
      from_id: hypothesis.fromId,
      to_id: hypothesis.toId,
      predicate_id: hypothesis.predicateLabel || "session_hypothesis",
      label: hypothesis.title,
      title: hypothesis.title,
      summary: hypothesis.body,
      session_hypothesis: true,
      source: false,
      reviewed: false,
      canon: false,
      authority_layer: "session-hypothesis",
      graph_layers: ["session-hypothesis"],
    };
    addGraphEdge(hypothesis.id, hypothesis.fromId, hypothesis.toId, payload);
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

function setSelectedItemState(item: AnyItem): void {
  epistemicRevision += 1;
  state.epistemicPacket = null;
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
}

function applySelectedItem(item: AnyItem): void {
  setSelectedItemState(item);
  renderChips();
  renderGraphPreservingInspectorLists();
  renderInspector();
  syncPublicRoute();
  scrollInspectorTop();
}

function selectableItem(itemIdValue: string): AnyItem | null {
  const current = state.currentView as AnyItem | null;
  const collections = [
    state.results,
    state.relationItems,
    state.neighborhood?.neighbors || [],
    state.neighborhood?.edges || [],
    state.pathPacket?.nodes || [],
    state.pathPacket?.edges || [],
    state.epistemicPacket?.challenge_relations || [],
    state.epistemicPacket?.context_relations || [],
    state.epistemicPacket?.neighbor_nodes || [],
    ...(
      current
        ? [
            (current.nodes as AnyItem[] | undefined) || [],
            (current.edges as AnyItem[] | undefined) || [],
            (current.clusters as AnyItem[] | undefined) || [],
            (current.items as AnyItem[] | undefined) || [],
          ]
        : []
    ),
    [...lastGraphItems.values()],
  ];
  for (const collection of collections) {
    const match = collection.find((item) => itemId(item) === itemIdValue);
    if (match) return match;
  }
  return null;
}

function viewItem(view: PhilosophyViewPayload | CorpusViewPayload, itemIdValue: string): AnyItem | null {
  const current = view as AnyItem;
  if (!current) return null;
  const collections = [
    (current.nodes as AnyItem[] | undefined) || [],
    (current.edges as AnyItem[] | undefined) || [],
    (current.clusters as AnyItem[] | undefined) || [],
    (current.items as AnyItem[] | undefined) || [],
  ];
  for (const collection of collections) {
    const match = collection.find((item) => itemId(item) === itemIdValue);
    if (match) return match;
  }
  return null;
}

function isAggregateRelation(item: AnyItem): boolean {
  return text(item.edge_id).startsWith("cluster-relation:") || stringList(item.member_edge_ids).length > 0;
}

function selectItem(item: AnyItem): void {
  invokePageCommandFromUi("tos.page.select", { item_id: itemId(item) });
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

function epistemicCards(itemIdValue: string): string[] {
  const packet = state.epistemicPacket;
  if (!packet || packet.item_id !== itemIdValue) return [];
  const posture = packet.selection_posture || {};
  const fieldPosture = packet.field_posture || {};
  const coverage = packet.coverage || {};
  const postureLines = [
    posture.authority_posture ? `authority: ${humanKind(posture.authority_posture)}` : "",
    posture.canon_status ? `canon: ${humanKind(posture.canon_status)}` : "",
    posture.review_posture ? `review: ${humanKind(posture.review_posture)}` : "",
    posture.confidence ? `confidence: ${humanKind(posture.confidence)}` : "",
    posture.priority ? `priority: ${humanKind(posture.priority)}` : "",
    `claim/evidence closed: ${posture.claim_evidence_closed ? "yes" : "no"}`,
  ].filter(Boolean);
  const fieldPostureLines = [
    ...(fieldPosture.authority_postures || []).map((value) => `authority: ${humanKind(value)}`),
    ...(fieldPosture.canon_statuses || []).map((value) => `canon: ${humanKind(value)}`),
    ...(fieldPosture.review_postures || []).map((value) => `review: ${humanKind(value)}`),
    ...(fieldPosture.confidence_values || []).map((value) => `confidence: ${humanKind(value)}`),
  ];
  const coverageLines = [
    coverage.posture ? `posture: ${humanKind(coverage.posture)}` : "",
    coverage.challenge_state ? `challenges: ${humanKind(coverage.challenge_state)}` : "",
    coverage.available_challenge_relations !== undefined
      ? `challenge relations: ${coverage.returned_challenge_relations || 0}/${coverage.available_challenge_relations}`
      : "",
    ...(coverage.missing_surfaces || []).map((value) => `missing: ${value}`),
  ].filter(Boolean);
  const challengeRows = (packet.challenge_relations || []).map((edge) => {
    const direction: RelationDirection = edge.from_id === itemIdValue
      ? "outgoing"
      : edge.to_id === itemIdValue
        ? "incoming"
        : "adjacent";
    return relationRowFromEdge(edge, direction);
  });
  const contextRows = (packet.context_relations || []).map((edge) => {
    const direction: RelationDirection = edge.from_id === itemIdValue
      ? "outgoing"
      : edge.to_id === itemIdValue
        ? "incoming"
        : "adjacent";
    return relationRowFromEdge(edge, direction);
  });
  const finding = state.language === "ru" ? packet.finding_ru || packet.finding : packet.finding;
  const conclusion = packet.conclusion || {};
  const allowed = state.language === "ru" ? conclusion.allowed_ru || conclusion.allowed : conclusion.allowed;
  const notAllowed = state.language === "ru" ? conclusion.not_allowed_ru || conclusion.not_allowed : conclusion.not_allowed;
  const gaps = state.language === "ru" ? packet.gaps_ru || packet.gaps : packet.gaps;
  const cards = [
    ...(finding
      ? [detailCard(
          t("detail.evidenceFinding"),
          `${finding}\nposture: ${humanKind(packet.posture || "unknown")}\nconclusive in stated scope: ${conclusion.can_conclude ? "yes" : "no"}`,
        )]
      : []),
    ...(allowed?.length
      ? [detailCard(t("detail.allowedConclusions"), allowed.map((value) => `✓ ${value}`).join("\n"))]
      : []),
    ...(notAllowed?.length
      ? [detailCard(t("detail.forbiddenConclusions"), notAllowed.map((value) => `— ${value}`).join("\n"))]
      : []),
    ...(packet.source_anchors?.length
      ? [detailCard(
          t("detail.sourceAnchors"),
          packet.source_anchors.map((anchor) => `${anchor.edge_id}: ${(anchor.anchor_segment_ids || []).join(", ")} [${anchor.witness_scope || ""}]`).join("\n"),
        )]
      : []),
    ...(packet.routes?.length
      ? [detailCard(
          t("detail.ownerRoutes"),
          packet.routes.map((route) => `${humanKind(route.route_kind || "route")}: ${route.status || ""}\n${route.ref || ""}`).join("\n\n"),
        )]
      : []),
    ...(gaps?.length
      ? [detailCard(t("detail.evidenceGaps"), gaps.map((gap) => `• ${gap}`).join("\n"))]
      : []),
    detailCard(t("detail.epistemicPosture"), postureLines.join("\n")),
    detailCard(t("detail.contextPosture"), fieldPostureLines.join("\n")),
    detailCard(t("detail.coverage"), coverageLines.join("\n")),
  ];
  if (challengeRows.length) {
    cards.push(detailCard(t("detail.projectedChallenges"), t("detail.notAdjudicated")));
    cards.push(
      ...challengeRows.slice(0, 8).map((row) =>
        detailCard(
          relationRouteText(row),
          itemNarrative(row) || humanKind(predicateId(row)),
        ),
      ),
    );
    cards.push(relationRowsSection(t("detail.projectedChallenges"), challengeRows, "epistemic"));
  }
  if (contextRows.length) {
    cards.push(`<div class="section-title">${t("detail.contextRelations")}</div>`);
    cards.push(
      ...contextRows.slice(0, 12).map((row) =>
        detailCard(relationDisplayLabel(row), relationRouteText(row)),
      ),
    );
  }
  if (packet.source_refs?.length) cards.push(sourceReferenceList(packet.source_refs));
  if (packet.authority_note) cards.push(detailCard(t("detail.coverage"), packet.authority_note));
  return cards;
}

async function showEpistemic(
  itemIdValue: string,
  limit = 80,
  signal?: AbortSignal,
): Promise<EpistemicPayload> {
  if (!itemIdValue) throw new Error("select a graph node or edge");
  const evidenceModeAvailable = state.mode === "philosophy"
    || (state.mode === "corpus" && state.currentViewId === "route-graph");
  if (!evidenceModeAvailable) throw new Error("Evidence Lens requires a philosophy view or the corpus route graph");
  const selected = pageSelection();
  if (!selected || selected.id !== itemIdValue || !["node", "edge"].includes(selected.kind)) {
    throw new Error("epistemic inspection requires the current projection node or edge selection");
  }
  if (selected.reroutable === false) {
    throw new Error("aggregate cluster relations do not have a direct projection evidence field");
  }
  const requestRevision = ++epistemicRevision;
  const requestMode = state.mode;
  const requestActiveViewId = state.currentViewId;
  const selectedItem = state.selected && itemId(state.selected) === itemIdValue
    ? state.selected
    : selectableItem(itemIdValue);
  const selectedProjectionItem = selectedItem ? unwrapItem(selectedItem) : null;
  const declaredViewIds = Array.isArray(selectedProjectionItem?.view_ids)
    ? stringList(selectedProjectionItem?.view_ids)
    : null;
  const selectionBelongsToActiveView = declaredViewIds
    ? declaredViewIds.includes(requestActiveViewId)
    : Boolean(state.currentView && viewItem(state.currentView, itemIdValue));
  const requestConstraintViewId = selectionBelongsToActiveView ? requestActiveViewId : "";
  const packet = (await queryOperations.invoke("tos.epistemic.inspect", {
    mode: state.mode,
    item_id: itemIdValue,
    view_id: requestConstraintViewId,
    limit,
  }, { signal })) as EpistemicPayload;
  signal?.throwIfAborted();
  if (
    requestRevision !== epistemicRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestActiveViewId ||
    pageSelection()?.id !== itemIdValue
  ) throw new DOMException("superseded epistemic request", "AbortError");
  state.epistemicPacket = packet;
  state.inspectorOpen = true;
  renderInspector();
  scrollInspectorTop();
  return packet;
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
  if ((state.pathPacket.paths?.length || 0) > 1) {
    cards.push(`<div class="section-title">${t("detail.pathAlternatives")}</div>`);
    cards.push(
      ...(state.pathPacket.paths || []).map((path, index) =>
        detailCard(
          `${t("detail.path")} ${index + 1}`,
          path.node_ids.map((pathNodeId) => endpointLabel(pathNodeId) || pathNodeId).join(" → "),
        ),
      ),
    );
  }
  return cards;
}

async function showNeighborhood(
  nodeId: string,
  depth = 1,
  signal?: AbortSignal,
): Promise<NeighborhoodPayload> {
  if (!nodeId) throw new Error("node_id is required");
  assertPhilosophyRouteAvailable();
  const requestRevision = ++neighborhoodRevision;
  const requestMode = state.mode;
  const requestViewId = state.currentViewId;
  const selected = state.selected;
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  const neighborhood = (await queryOperations.invoke("tos.neighborhood", {
    node_id: nodeId,
    depth,
    limit: 160,
    layers: [...state.activeLayers],
    predicates: [...state.activePredicates],
  }, { signal })) as NeighborhoodPayload;
  signal?.throwIfAborted();
  if (
    requestRevision !== neighborhoodRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestViewId
  ) throw new DOMException("superseded neighborhood request", "AbortError");
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
  syncPublicRoute();
  scrollInspectorTop();
  return neighborhood;
}

function setPathStart(nodeId: string): void {
  if (!nodeId) return;
  pathRevision += 1;
  state.pathStartNodeId = nodeId;
  state.pathPacket = null;
  renderInspector();
}

type PathRequest = {
  direction?: "outgoing" | "incoming" | "either";
  maxDepth?: number;
  alternativeLimit?: number;
  excludedEdgeIds?: string[];
  viewId?: string;
  signal?: AbortSignal;
};

async function showPath(fromId: string, toId: string, request: PathRequest = {}): Promise<PathPayload> {
  if (!fromId || !toId || fromId === toId) throw new Error("path endpoints must be different nodes");
  assertPhilosophyRouteAvailable();
  const requestRevision = ++pathRevision;
  const requestMode = state.mode;
  const requestViewId = state.currentViewId;
  const selected = state.selected;
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  const pathPacket = (await queryOperations.invoke("tos.path.find", {
    from_id: fromId,
    to_id: toId,
    max_depth: request.maxDepth || 6,
    direction: request.direction || "outgoing",
    view_id: request.viewId,
    excluded_edge_ids: request.excludedEdgeIds || [],
    alternative_limit: request.alternativeLimit || 1,
    layers: [...state.activeLayers],
    predicates: [...state.activePredicates],
  }, { signal: request.signal })) as PathPayload;
  request.signal?.throwIfAborted();
  if (
    requestRevision !== pathRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestViewId
  ) throw new DOMException("superseded path request", "AbortError");
  state.graphMode = "nodes";
  state.expandedCluster = null;
  state.pathStartNodeId = fromId;
  state.pathPacket = pathPacket;
  state.selected = selected;
  state.selectedGraphId = toId;
  renderChips();
  renderGraph();
  ignoreGraphClicksUntil = Date.now() + 1500;
  ignoreInspectorSelectionsUntil = Date.now() + 1500;
  renderInspector();
  syncPublicRoute();
  scrollInspectorTop();
  return pathPacket;
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

function assertPhilosophyRouteAvailable(): void {
  if (state.mode !== "philosophy") throw new Error("graph route commands require philosophy mode");
  if (state.activeLayers.size === 0 || state.activePredicates.size === 0) {
    throw new Error("graph route commands require at least one active layer and predicate");
  }
}

function researchId(prefix: "hypothesis" | "route" | "note"): string {
  const suffix = typeof crypto.randomUUID === "function"
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2, 12)}`;
  return `${prefix}:${suffix}`;
}

function workspaceHypothesisCard(hypothesis: ResearchHypothesis): string {
  const route = hypothesis.fromId && hypothesis.toId
    ? `<small>${escapeHtml(endpointLabel(hypothesis.fromId) || hypothesis.fromId)} → ${escapeHtml(endpointLabel(hypothesis.toId) || hypothesis.toId)}</small>`
    : "";
  return `<article class="workspace-card workspace-hypothesis">
    <strong>${escapeHtml(hypothesis.title)}</strong>
    ${route}
    <span>${escapeHtml(hypothesis.body)}</span>
    <em>${escapeHtml(t("workspace.localOnly"))}</em>
  </article>`;
}

function workspaceRouteCard(route: RouteSnapshot): string {
  return `<article class="workspace-card">
    <strong>${escapeHtml(route.label)}</strong>
    <span>${escapeHtml(endpointLabel(route.fromId) || route.fromId)} → ${escapeHtml(endpointLabel(route.toId) || route.toId)}</span>
    <small>${route.nodeIds.length} ${t("caption.nodes")} · ${route.edgeIds.length} ${t("caption.links")}</small>
  </article>`;
}

async function exportResearchWorkspace(): Promise<void> {
  const result = await pageCommands.invoke("tos.page.workspace-export");
  const value = result.value as { filename?: unknown; packet?: unknown } | undefined;
  const packet = typeof value?.packet === "string" ? value.packet : "";
  if (!packet) throw new Error("research workspace export is empty");
  const url = URL.createObjectURL(new Blob([`${packet}\n`], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = typeof value?.filename === "string" ? value.filename : "tos-research-workspace.v1.json";
  anchor.click();
  URL.revokeObjectURL(url);
}

function renderResearchWorkspace(): void {
  const root = document.getElementById("research-workspace-body");
  const summaryNode = document.getElementById("research-workspace-summary");
  if (!root || !summaryNode) return;
  const workspace = researchWorkspace.getState();
  const summary = researchWorkspace.summary();
  const selection = pageSelection();
  const selectedEdge = selection?.kind === "edge" && selection.reroutable !== false && selection.from_id && selection.to_id
    ? selection
    : null;
  summaryNode.textContent = `${summary.hypothesis_count}H · ${summary.comparison_count}R · ${summary.note_count}N`;
  const hasContents = summary.hypothesis_count + summary.comparison_count + summary.excluded_edge_count + summary.note_count > 0;

  root.innerHTML = `
    <p class="workspace-posture">${escapeHtml(t("workspace.localOnly"))}</p>
    ${selectedEdge ? `<div class="workspace-context">
      <small>${escapeHtml(selectedEdge.label || selectedEdge.id)}</small>
      <textarea id="workspace-hypothesis-input" maxlength="4000" placeholder="${escapeHtml(t("workspace.hypothesisPlaceholder"))}"></textarea>
      <input id="workspace-predicate-input" type="text" maxlength="512" placeholder="${escapeHtml(t("workspace.predicatePlaceholder"))}" />
      <div class="workspace-actions">
        <button id="workspace-add-hypothesis" type="button">${t("workspace.addHypothesis")}</button>
        <button id="workspace-exclude-edge" type="button">${t("workspace.excludeEdge")}</button>
        <button id="workspace-save-route" type="button">${t("workspace.saveRoute")}</button>
      </div>
    </div>` : ""}
    <div class="workspace-note-entry">
      <textarea id="workspace-note-input" maxlength="4000" placeholder="${escapeHtml(t("workspace.notePlaceholder"))}"></textarea>
      <button id="workspace-add-note" type="button">${t("workspace.addNote")}</button>
    </div>
    <div class="workspace-history-actions">
      <button id="workspace-undo" type="button" ${summary.can_undo ? "" : "disabled"}>${t("workspace.undo")}</button>
      <button id="workspace-redo" type="button" ${summary.can_redo ? "" : "disabled"}>${t("workspace.redo")}</button>
      <button id="workspace-export" type="button">${t("workspace.export")}</button>
      <button id="workspace-import-button" type="button">${t("workspace.import")}</button>
    </div>
    ${researchWorkspace.comparableRoutesReady() ? `<p class="workspace-ready">${t("workspace.compareReady")}</p>` : ""}
    ${workspace.hypotheses.length ? `<section><h3>${t("workspace.hypotheses")}</h3>${workspace.hypotheses.map(workspaceHypothesisCard).join("")}</section>` : ""}
    ${workspace.routeSnapshots.length ? `<section><h3>${t("workspace.comparisons")}</h3>${workspace.routeSnapshots.map(workspaceRouteCard).join("")}</section>` : ""}
    ${workspace.excludedEdgeIds.length ? `<section><h3>${t("workspace.exclusions")}</h3><div class="workspace-tags">${workspace.excludedEdgeIds.map((id) => `<code>${escapeHtml(id)}</code>`).join("")}</div></section>` : ""}
    ${workspace.notes.length ? `<section><h3>${t("workspace.notes")}</h3>${workspace.notes.map((note) => `<article class="workspace-card"><span>${escapeHtml(note.body)}</span>${note.targetId ? `<small>${escapeHtml(note.targetId)}</small>` : ""}</article>`).join("")}</section>` : ""}
    ${workspace.journal.length ? `<section><h3>${t("workspace.journal")}</h3><ol class="workspace-journal">${workspace.journal.slice(-8).reverse().map((entry) => `<li><span>${escapeHtml(humanKind(entry.action))}</span>${entry.targetId ? `<code>${escapeHtml(entry.targetId)}</code>` : ""}</li>`).join("")}</ol></section>` : ""}
    ${hasContents ? "" : `<p class="workspace-empty">${t("workspace.empty")}</p>`}
  `;

  document.getElementById("workspace-add-hypothesis")?.addEventListener("click", () => {
    const statement = (document.getElementById("workspace-hypothesis-input") as HTMLTextAreaElement | null)?.value || "";
    const predicateLabel = (document.getElementById("workspace-predicate-input") as HTMLInputElement | null)?.value || "";
    invokePageCommandFromUi("tos.page.add-session-hypothesis", { statement, predicate_label: predicateLabel });
  });
  document.getElementById("workspace-exclude-edge")?.addEventListener("click", () => invokePageCommandFromUi("tos.page.exclude-selected-edge"));
  document.getElementById("workspace-save-route")?.addEventListener("click", () => invokePageCommandFromUi("tos.page.save-route-comparison"));
  document.getElementById("workspace-add-note")?.addEventListener("click", () => {
    const note = (document.getElementById("workspace-note-input") as HTMLTextAreaElement | null)?.value || "";
    invokePageCommandFromUi("tos.page.add-research-note", { text: note });
  });
  document.getElementById("workspace-undo")?.addEventListener("click", () => invokePageCommandFromUi("tos.page.workspace-undo"));
  document.getElementById("workspace-redo")?.addEventListener("click", () => invokePageCommandFromUi("tos.page.workspace-redo"));
  document.getElementById("workspace-export")?.addEventListener("click", () => void exportResearchWorkspace().catch((error: unknown) => showPageCommandError("tos.page.workspace-export", error)));
  document.getElementById("workspace-import-button")?.addEventListener("click", () => (byId("research-workspace-import") as HTMLInputElement).click());
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
  renderResearchWorkspace();
}

async function copyScaleExportUrl(table?: ScaleExportTable, format?: "csv" | "jsonl"): Promise<void> {
  const url = scaleExportAbsoluteUrl(table, format);
  const expectedRevision = pageCommands.context().revision;
  let copyError = "";
  try {
    await navigator.clipboard.writeText(url);
  } catch (error) {
    copyError = text(error);
  }
  if (pageCommands.context().revision !== expectedRevision) return;
  state.epistemicPacket = null;
  state.selected = {
    title: t("selection.scaleExportUrl"),
    url,
    ...(copyError ? { copy_error: copyError } : {}),
  };
  state.selectedGraphId = null;
  renderInspector();
  syncPublicRoute();
  pageCommands.notifyStateChange();
  scrollInspectorTop();
}

type PreparedView = {
  mode: Mode;
  viewId: string;
  currentView: PhilosophyViewPayload | CorpusViewPayload;
  sourceNotes: GraphNode[];
  sourceNoteEdges: GraphEdge[];
  activeLayers: Set<string>;
  activePredicates: Set<string>;
  graphMode: GraphMode;
  results: AnyItem[];
};

async function prepareView(
  mode: Mode,
  viewId: string,
  requestedGraphMode: GraphMode | undefined,
  signal: AbortSignal | undefined,
): Promise<PreparedView> {
  if (mode === "philosophy") {
    const sourcePayload = (await queryOperations.invoke("tos.view.open", {
      mode,
      view_id: viewId,
      limit: 1000,
    }, { signal })) as PhilosophyViewPayload;
    signal?.throwIfAborted();
    const sourceNotes = (sourcePayload.nodes || []).filter(isPublicSourceNote);
    const sourceNoteIds = new Set(sourceNotes.map((node) => node.node_id));
    const payload = projectPublicPhilosophyPayload(sourcePayload);
    return {
      mode,
      viewId,
      currentView: payload,
      sourceNotes,
      sourceNoteEdges: (sourcePayload.edges || []).filter(
        (edge) => sourceNoteIds.has(edge.from_id) || sourceNoteIds.has(edge.to_id),
      ),
      activeLayers: new Set(payload.view.graph_layers || []),
      activePredicates: new Set((payload.edges || []).filter(isPublicAtlasItem).map(predicateId)),
      graphMode: requestedGraphMode || "clusters",
      results: (payload.clusters || []).filter(isPublicAtlasItem),
    };
  }
  const payload = (await queryOperations.invoke("tos.view.open", {
    mode,
    view_id: viewId,
    limit: 700,
  }, { signal })) as CorpusViewPayload;
  signal?.throwIfAborted();
  return {
    mode,
    viewId,
    currentView: payload,
    sourceNotes: [],
    sourceNoteEdges: [],
    activeLayers: new Set(),
    activePredicates: new Set(),
    graphMode: "nodes",
    results: (payload.items || []).filter(isPublicAtlasItem),
  };
}

function commitPreparedView(prepared: PreparedView, requestedFocusId: string): void {
  searchRevision += 1;
  neighborhoodRevision += 1;
  pathRevision += 1;
  epistemicRevision += 1;
  state.mode = prepared.mode;
  state.currentViewId = prepared.viewId;
  state.currentView = prepared.currentView;
  state.selected = null;
  state.selectedGraphId = null;
  state.results = prepared.results;
  state.relationItems = [];
  state.expandedCluster = null;
  state.neighborhood = null;
  state.pathStartNodeId = null;
  state.pathPacket = null;
  state.epistemicPacket = null;
  state.inspectorOpen = false;
  state.sourceNotes = prepared.sourceNotes;
  state.sourceNoteEdges = prepared.sourceNoteEdges;
  state.activeLayers = prepared.activeLayers;
  state.activePredicates = prepared.activePredicates;
  state.graphMode = prepared.graphMode;
  if (prepared.mode === "philosophy") {
    state.densityMode = "overview";
    state.minRelationCount = 1;
  }
  if (requestedFocusId) {
    const focusItem = viewItem(prepared.currentView, requestedFocusId);
    if (focusItem) setSelectedItemState(focusItem);
  }
  renderAll();
  syncPublicRoute();
}

async function loadMode(
  mode: Mode,
  requestedViewId = "",
  requestedGraphMode?: GraphMode,
  requestedFocusId = "",
  signal?: AbortSignal,
): Promise<void> {
  const loadRevision = ++modeLoadRevision;
  viewLoadRevision += 1;
  if (mode === "philosophy") {
    const status = await fetchJson<AnyItem>("/api/philosophy/status", { signal });
    const views = await fetchJson<{ views: ViewCard[] }>("/api/philosophy/views", { signal });
    const philosophyViews = views.views || [];
    const viewId = philosophyViews.some((view) => view.view_id === requestedViewId)
      ? requestedViewId
      : boot.default_philosophy_view || philosophyViews[0]?.view_id || "";
    const prepared = await prepareView(mode, viewId, requestedGraphMode, signal);
    signal?.throwIfAborted();
    if (loadRevision !== modeLoadRevision) throw new DOMException("superseded mode request", "AbortError");
    state.status.philosophy = status;
    state.philosophyViews = philosophyViews;
    commitPreparedView(prepared, requestedFocusId);
  } else {
    const status = await fetchJson<AnyItem>("/api/corpus/status", { signal });
    const summary = await fetchJson<{ graph_views?: ViewCard[]; counts?: AnyItem }>("/api/corpus/summary", { signal });
    const corpusViews = summary.graph_views || [];
    const viewId = corpusViews.some((view) => view.view_id === requestedViewId)
      ? requestedViewId
      : boot.default_view || corpusViews[0]?.view_id || "";
    const prepared = await prepareView(mode, viewId, "nodes", signal);
    signal?.throwIfAborted();
    if (loadRevision !== modeLoadRevision) throw new DOMException("superseded mode request", "AbortError");
    state.status.corpus = { ...status, counts: summary.counts || status.counts };
    state.corpusViews = corpusViews;
    commitPreparedView(prepared, requestedFocusId);
  }
}

async function loadView(
  viewId: string,
  requestedGraphMode?: GraphMode,
  requestedFocusId = "",
  signal?: AbortSignal,
): Promise<void> {
  if (!viewId) return;
  const knownViews = state.mode === "philosophy" ? state.philosophyViews : state.corpusViews;
  requireKnownViewId(viewId, knownViews.map((view) => view.view_id));
  const loadRevision = ++viewLoadRevision;
  const loadMode = state.mode;
  const prepared = await prepareView(loadMode, viewId, requestedGraphMode, signal);
  signal?.throwIfAborted();
  if (loadRevision !== viewLoadRevision || state.mode !== loadMode) {
    throw new DOMException("superseded view request", "AbortError");
  }
  commitPreparedView(prepared, requestedFocusId);
}

async function search(requestedQuery?: string, signal?: AbortSignal): Promise<{ result_count: number }> {
  const requestRevision = ++searchRevision;
  const requestMode = state.mode;
  const requestViewId = state.currentViewId;
  const input = byId("search") as HTMLInputElement;
  const query = (requestedQuery === undefined ? input.value : requestedQuery).trim();
  input.value = query;
  state.searchQuery = query;
  state.epistemicPacket = null;
  const payload = (await queryOperations.invoke("tos.search", {
    mode: state.mode,
    query,
    limit: 80,
  }, { signal })) as { results?: AnyItem[] };
  signal?.throwIfAborted();
  if (
    requestRevision !== searchRevision ||
    state.mode !== requestMode ||
    state.currentViewId !== requestViewId
  ) throw new DOMException("superseded search request", "AbortError");
  state.results = mergeLocalizedSearchResults(query, (payload.results || []).filter(isPublicAtlasItem), 80);
  state.selected = { title: query ? `${t("selection.search")}: ${query}` : t("selection.search"), results: state.results.length };
  state.inspectorOpen = true;
  state.selectedGraphId = null;
  renderInspector();
  syncPublicRoute();
  scrollInspectorTop();
  return { result_count: state.results.length };
}

function pageSelection(): PageSelection | null {
  if (!state.selected) return null;
  const item = unwrapItem(state.selected);
  const id = itemId(item);
  if (!id || id === "item") return null;
  const kind: PageSelection["kind"] = item.edge_id || (item.from_id && item.to_id)
    ? "edge"
    : item.node_id
      ? "node"
      : item.cluster_id
        ? "cluster"
        : "item";
  return {
    id,
    kind,
    label: displayTitle(item),
    ...(item.from_id ? { from_id: text(item.from_id) } : {}),
    ...(item.to_id ? { to_id: text(item.to_id) } : {}),
    ...(kind === "edge" ? { reroutable: !isAggregateRelation(item) && item.session_hypothesis !== true } : {}),
  };
}

function pageContextSnapshot(): PageContextSnapshot {
  return {
    mode: state.mode,
    view_id: state.currentViewId,
    graph_mode: state.graphMode,
    selected: pageSelection(),
    path_start_node_id: state.pathStartNodeId,
    active_layers: [...state.activeLayers].sort(),
    active_predicates: [...state.activePredicates].sort(),
    deep_link: window.location.href,
    research_workspace: researchWorkspace.summary(),
  };
}

function commandString(input: Record<string, unknown>, key: string, fallback = ""): string {
  return text(input[key] ?? fallback).trim();
}

function commandInteger(input: Record<string, unknown>, key: string, fallback: number, low: number, high: number): number {
  const parsed = Number(input[key]);
  return Number.isFinite(parsed) ? Math.max(low, Math.min(high, Math.trunc(parsed))) : fallback;
}

function commandDirection(input: Record<string, unknown>): "outgoing" | "incoming" | "either" {
  const direction = commandString(input, "direction", "outgoing");
  if (direction === "outgoing" || direction === "incoming" || direction === "either") return direction;
  throw new Error("direction must be outgoing, incoming, or either");
}

function selectedNodeId(): string {
  return state.selected ? selectedNodeIdFor(state.selected) : "";
}

function workspaceExcludedEdgeIds(extra: string[] = []): string[] {
  return [...new Set([...researchWorkspace.getState().excludedEdgeIds, ...extra].filter(Boolean))];
}

function refreshResearchSurfaces(): void {
  renderGraph();
  renderInspector();
  renderResearchWorkspace();
}

function currentRouteSnapshot(label: string): RouteSnapshot {
  const selected = pageSelection();
  const path = state.pathPacket?.found
    ? state.pathPacket.paths?.[0] || {
        node_ids: stringList(state.pathPacket.nodes?.map((node) => node.node_id)),
        edge_ids: stringList(state.pathPacket.edges?.map((edge) => edge.edge_id)),
      }
    : null;
  const fromId = text(state.pathPacket?.from_id || selected?.from_id);
  const toId = text(state.pathPacket?.to_id || selected?.to_id);
  if (!fromId || !toId) throw new Error("select an edge or show a path before saving a route comparison");
  const nodeIds = path ? stringList(path.node_ids) : [fromId, toId];
  const edgeIds = path ? stringList(path.edge_ids) : selected?.kind === "edge" ? [selected.id] : [];
  if (!nodeIds.length) throw new Error("the visible route has no nodes to save");
  return {
    id: researchId("route"),
    label,
    fromId,
    toId,
    nodeIds,
    edgeIds,
  };
}

const pageCommands = createPageCommandRegistry(pageContextSnapshot, {
  "tos.page.open-view": async (input, execution) => {
    const mode = commandString(input, "mode", state.mode) === "corpus" ? "corpus" : "philosophy";
    const viewId = commandString(input, "view_id");
    const graphMode = commandString(input, "graph_mode") === "nodes" ? "nodes" : "clusters";
    const focusId = commandString(input, "focus_id");
    const knownViews = mode === "philosophy" ? state.philosophyViews : state.corpusViews;
    if (mode !== state.mode || !viewId || knownViews.length === 0) {
      await loadMode(mode, viewId, graphMode, focusId, execution.signal);
    } else {
      await loadView(viewId, graphMode, focusId, execution.signal);
    }
    return { mode: state.mode, view_id: state.currentViewId, focus_id: state.selectedGraphId };
  },
  "tos.page.search": async (input, execution) => search(commandString(input, "query"), execution.signal),
  "tos.page.select": (input) => {
    const itemIdValue = commandString(input, "item_id");
    if (!itemIdValue) throw new Error("item_id is required");
    const item = selectableItem(itemIdValue);
    if (!item) throw new Error(`item is not present in the active view: ${itemIdValue}`);
    applySelectedItem(item);
    return { selected_id: itemIdValue };
  },
  "tos.page.show-neighborhood": async (input, execution) => {
    const nodeId = commandString(input, "node_id", selectedNodeId());
    return showNeighborhood(nodeId, commandInteger(input, "depth", 1, 1, 3), execution.signal);
  },
  "tos.page.start-path": (input) => {
    assertPhilosophyRouteAvailable();
    const nodeId = commandString(input, "node_id", selectedNodeId());
    if (!nodeId) throw new Error("select a node or provide node_id");
    setPathStart(nodeId);
    return { path_start_node_id: nodeId };
  },
  "tos.page.find-path": async (input, execution) => {
    const fromId = commandString(input, "from_id", state.pathStartNodeId || "");
    const toId = commandString(input, "to_id", selectedNodeId());
    const constrainToView = input.constrain_to_view !== false;
    return showPath(fromId, toId, {
      direction: commandDirection(input),
      maxDepth: commandInteger(input, "max_depth", 6, 1, 8),
      alternativeLimit: commandInteger(input, "alternative_limit", 1, 1, 5),
      excludedEdgeIds: workspaceExcludedEdgeIds(stringList(input.excluded_edge_ids)),
      viewId: constrainToView && state.mode === "philosophy" ? state.currentViewId : undefined,
      signal: execution.signal,
    });
  },
  "tos.page.reroute-without-selection": async (input, execution) => {
    const selected = pageSelection();
    if (
      !selected ||
      selected.kind !== "edge" ||
      selected.reroutable === false ||
      !selected.from_id ||
      !selected.to_id
    ) {
      throw new Error("select a projection edge before requesting a reroute");
    }
    const constrainToView = input.constrain_to_view !== false;
    return showPath(selected.from_id, selected.to_id, {
      direction: commandDirection(input),
      maxDepth: commandInteger(input, "max_depth", 6, 1, 8),
      alternativeLimit: commandInteger(input, "alternative_limit", 3, 1, 5),
      excludedEdgeIds: workspaceExcludedEdgeIds([selected.id]),
      viewId: constrainToView && state.mode === "philosophy" ? state.currentViewId : undefined,
      signal: execution.signal,
    });
  },
  "tos.page.inspect-epistemic": async (input, execution) => {
    const itemIdValue = commandString(input, "item_id", pageSelection()?.id || "");
    const result = await showEpistemic(
      itemIdValue,
      commandInteger(input, "limit", 80, 1, 200),
      execution.signal,
    );
    const selected = pageSelection();
    if (selected && researchWorkspace.getState().selectedLens?.id !== selected.id) {
      researchWorkspace.selectLens({ id: selected.id, kind: selected.kind, ...(selected.label ? { label: selected.label } : {}) });
    }
    renderResearchWorkspace();
    return result;
  },
  "tos.page.research-workspace": () => ({
    packet: JSON.parse(researchWorkspace.exportPacket()) as Record<string, unknown>,
    local_only: true,
    authority: { source: false, reviewed: false, canon: false },
  }),
  "tos.page.add-research-note": (input) => {
    const body = commandString(input, "text");
    if (!body) throw new Error("text is required");
    const selected = pageSelection();
    researchWorkspace.addNote({
      id: researchId("note"),
      body,
      ...(selected ? { targetId: selected.id } : {}),
    });
    renderResearchWorkspace();
    return { added: true, summary: researchWorkspace.summary() };
  },
  "tos.page.add-session-hypothesis": (input) => {
    const selected = pageSelection();
    if (!selected || selected.kind !== "edge" || selected.reroutable === false || !selected.from_id || !selected.to_id) {
      throw new Error("select a non-aggregate source edge before adding a session hypothesis");
    }
    const statement = commandString(input, "statement");
    if (!statement) throw new Error("statement is required");
    const predicateLabel = commandString(input, "predicate_label", "session hypothesis");
    const hypothesis = researchWorkspace.addHypothesis({
      id: researchId("hypothesis"),
      title: predicateLabel || "session hypothesis",
      body: statement,
      targetId: selected.id,
      fromId: selected.from_id,
      toId: selected.to_id,
      ...(predicateLabel ? { predicateLabel } : {}),
    });
    refreshResearchSurfaces();
    return {
      hypothesis,
      summary: researchWorkspace.summary(),
      authority: { session_hypothesis: true, source: false, reviewed: false, canon: false },
    };
  },
  "tos.page.exclude-selected-edge": () => {
    const selected = pageSelection();
    if (!selected || selected.kind !== "edge" || selected.reroutable === false) {
      throw new Error("select a non-aggregate source edge before excluding it");
    }
    researchWorkspace.excludeEdge(selected.id);
    refreshResearchSurfaces();
    return { excluded_edge_id: selected.id, summary: researchWorkspace.summary() };
  },
  "tos.page.save-route-comparison": (input) => {
    const nextNumber = researchWorkspace.getState().routeSnapshots.length + 1;
    const label = commandString(input, "label", `Route ${nextNumber}`);
    const route = currentRouteSnapshot(label);
    researchWorkspace.saveRouteSnapshot(route);
    renderResearchWorkspace();
    return { route, comparison_ready: researchWorkspace.comparableRoutesReady(), summary: researchWorkspace.summary() };
  },
  "tos.page.workspace-undo": () => {
    const changed = researchWorkspace.undo();
    refreshResearchSurfaces();
    return { changed, summary: researchWorkspace.summary() };
  },
  "tos.page.workspace-redo": () => {
    const changed = researchWorkspace.redo();
    refreshResearchSurfaces();
    return { changed, summary: researchWorkspace.summary() };
  },
  "tos.page.workspace-export": () => ({
    filename: "tos-research-workspace.v1.json",
    packet: researchWorkspace.exportPacket(),
    local_only: true,
  }),
  "tos.page.workspace-import": (input) => {
    const packet = commandString(input, "packet");
    if (!packet) throw new Error("packet is required");
    researchWorkspace.importPacket(packet);
    refreshResearchSurfaces();
    return { imported: true, summary: researchWorkspace.summary() };
  },
  "tos.page.clear-focus": () => {
    clearFocus();
    return { cleared: true };
  },
});

webMCP = createWebMCPAdapter(pageCommands, document as WebMCPDocument, {
  prepareWordAnalysis: (input, options) => queryOperations.invoke(
    "tos.zarathustra.word-analysis.prepare",
    input,
    options,
  ),
});

renderShell();
webMCP.subscribeStatus(() => renderAgentSurface());
void webMCP.start();
window.addEventListener("beforeunload", () => webMCP.stop(), { once: true });
void pageCommands.invoke("tos.page.open-view", {
  mode: initialRoute.mode,
  view_id: initialRoute.viewId,
  graph_mode: initialRoute.graphMode,
  focus_id: initialRoute.focusId,
}).catch((error: unknown) => {
  byId("inspector-title").textContent = t("load.failed");
  byId("inspector-meta").innerHTML = `<span class="danger">${text(error)}</span>`;
});
