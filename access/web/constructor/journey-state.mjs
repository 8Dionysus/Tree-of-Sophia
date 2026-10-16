/** Browser storage custody for Rust-owned prepared route rules. */
import {readingRule} from './reading-rules-rust.mjs';
export function bindResearchRoutes(routes,library){return readingRule('routes',{routes,library});}
export function routeGraphInput(route){return readingRule('graphInput',{route});}
export function routePath(route){return readingRule('path',{route});}

/** One explicit return point; graph edits and interpretive authority are separate. */
export function createJourneyNavigator(routes, {storage, key = 'tos-journey-v1'} = {}) {
  const byId = new Map(routes.map(route => [route.id, route]));
  let active = null, saved = null, error = null;
  const valid = point => readingRule('validPoint',{point,routes});
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
      if (!active || !valid({...active,finished:true})) return false;
      active = {...active, finished: true}; persist(); return true;
    },
    leave() { active = null; },
  };
}
