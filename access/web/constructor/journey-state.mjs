/** Guided walks remain a reading view over exact prepared nodes and relations. */
import {checkGrounds} from './inquiry-layer.mjs';
const bilingual = value => value && ['ru', 'en'].every(code => typeof value[code] === 'string' && value[code].trim());

export function bindResearchRoutes(routes, library) {
  const materials = new Set(library.nodes.map(node => node.id));
  const edges = new Map(library.atlas.edges.map(edge => [edge.id, edge]));
  const seen = new Set();
  return routes.map(route => {
    if (!route.id || seen.has(route.id) || !bilingual(route.title) || !bilingual(route.question) || !bilingual(route.description) || !bilingual(route.conclusion)) throw new Error('Invalid research route identity or wording');
    seen.add(route.id);
    checkGrounds(route.grounds,route.id+' introduction and conclusion');
    if (route.investigation && !['startingPoint', 'stakes', 'carryForward'].every(key => bilingual(route.investigation[key]))) throw new Error(`Route ${route.id} has incomplete investigation guidance`);
    if (!Array.isArray(route.steps) || route.steps.length < 2 || route.steps.length > 20 || route.transitions?.length !== route.steps.length - 1) throw new Error(`Route ${route.id} has no complete sequence`);
    const steps = route.steps.map(step => {
      if (!materials.has(step.nodeId) || !bilingual(step.title) || !bilingual(step.body)) throw new Error(`Route ${route.id} has an invalid step`);
      checkGrounds(step.grounds,route.id+' '+step.nodeId);
      return {...step, graphNodeId: `material:${step.nodeId}`};
    });
    const transitions = route.transitions.map((transition, index) => {
      const from = steps[index].nodeId, to = steps[index + 1].nodeId, edge = edges.get(transition.edgeId);
      if (transition.from !== from || transition.to !== to || !edge || !bilingual(transition.body)) throw new Error(`Route ${route.id} has an ungrounded transition`);
      const forward = edge.from === from && edge.to === to;
      if (!forward && !(edge.to === from && edge.from === to)) throw new Error(`Route ${route.id} transition does not follow its relation`);
      return {...transition, graphEdgeId: `atlas:${edge.id}`, direction: forward ? 'forward' : 'reverse'};
    });
    return {...route, steps, transitions};
  });
}

export function routeGraphInput(route) {
  return {nodeIds: [...new Set(route.steps.map(step => step.nodeId))], edgeIds: [...new Set(route.transitions.map(step => step.edgeId))]};
}

export function routePath(route) {
  return {nodeIds: [...new Set(route.steps.map(step => step.graphNodeId))], edgeIds: [...new Set(route.transitions.map(step => step.graphEdgeId))], restrictEdges: true};
}

/** One explicit return point; graph edits and interpretive authority are separate. */
export function createJourneyNavigator(routes, {storage, key = 'tos-journey-v1'} = {}) {
  const byId = new Map(routes.map(route => [route.id, route]));
  let active = null, saved = null, error = null;
  const valid = value => value && byId.has(value.routeId) && Number.isInteger(value.index) && value.index >= 0 && value.index < byId.get(value.routeId).steps.length && typeof value.finished === 'boolean' && (!value.finished || value.index === byId.get(value.routeId).steps.length - 1);
  try {
    const raw = storage?.getItem(key);
    if (raw) {
      const value = JSON.parse(raw);
      if (value.version !== 1 || !valid(value)) throw new Error('The saved route belongs to another version');
      saved = {routeId: value.routeId, index: value.index, finished: value.finished};
    }
  } catch (cause) { error = String(cause.message ?? cause); }
  const copy = value => value ? {...value} : null;
  function persist() {
    saved = copy(active);
    try { storage?.setItem(key, JSON.stringify({version: 1, ...saved})); error = null; }
    catch (cause) { error = String(cause.message ?? cause); }
  }
  return {
    getState: () => copy(active),
    savedPoint: () => copy(saved),
    error: () => error,
    currentRoute: () => active ? byId.get(active.routeId) : null,
    start(routeId, index = 0) {
      const candidate = {routeId, index, finished: false};
      if (!valid(candidate)) throw new Error('Unknown route or step');
      active = candidate; persist(); return copy(active);
    },
    go(index) {
      if (!active || !valid({...active, index, finished: false})) return false;
      active = {...active, index, finished: false}; persist(); return true;
    },
    finish() {
      if (!active || active.index !== byId.get(active.routeId).steps.length - 1) return false;
      active = {...active, finished: true}; persist(); return true;
    },
    leave() { active = null; },
  };
}
