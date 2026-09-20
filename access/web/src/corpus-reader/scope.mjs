// Loaded source rows, visible folded objects and global corpus size are separate.
// This description never guesses a total, hierarchy or semantic grouping.
export function explorationScope(state) {
  const view = state?.view;
  if (!view) return {available:false};
  return {
    available:true,
    loadedNodes:view.nodes.length,
    loadedRelations:view.relations.length,
    shownNodes:state.model?.vertices?.length ?? view.nodes.length,
    shownRelations:state.model?.edges?.length ?? view.relations.length,
    hasMore:Boolean(view.continuation?.next_cursor),
    continuationState:view.continuation?.state ?? null,
    mode:state.mode ?? null,
  };
}
export function scopeText(scope, language = 'ru') {
  if (!scope.available) return language === 'en' ? 'Open an area to explore.' : 'Откройте область для исследования.';
  if (language === 'en') return `Loaded: ${scope.loadedNodes} objects, ${scope.loadedRelations} relations. Visible: ${scope.shownNodes} objects, ${scope.shownRelations} relations.${scope.hasMore ? ' More material is available.' : ''}`;
  return `Загружено: ${scope.loadedNodes} объектов, ${scope.loadedRelations} связей. Показано: ${scope.shownNodes} объектов, ${scope.shownRelations} связей.${scope.hasMore ? ' Есть продолжение.' : ''}`;
}
export function mountExplorationScope(root, controller, locale = () => 'ru') {
  const container = root.querySelector('.footer') ?? root;
  const details = document.createElement('details');
  details.className = 'corpus-scope';
  const title = document.createElement('summary'), body = document.createElement('p');
  details.append(title,body);container.append(details);
  const refresh = () => {
    title.textContent = locale() === 'en' ? 'Exploration scope' : 'Область исследования';
    body.textContent = scopeText(explorationScope(controller.state()),locale());
  };
  const observer = new MutationObserver(refresh);
  observer.observe(root,{attributes:true,attributeFilter:['data-visible-nodes','data-visible-edges','data-selection','data-loading','data-source-revision','data-snapshot-revision']});
  root.addEventListener('click',refresh);refresh();
  return () => {observer.disconnect();root.removeEventListener('click',refresh);details.remove();};
}
