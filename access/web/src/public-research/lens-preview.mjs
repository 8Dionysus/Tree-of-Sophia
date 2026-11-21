// Pointing at a lens in the rail lights the stars it would show; the rest of the sky waits in the dark until the
// pointer leaves (sky.preview). Which stars a lens shows is read by the measure its own layout keeps: the scale of
// time its dated stars, the map its stars with a place, proximity the stars it measures, a lens that gathers what it
// gathers and the reader's own records, any other lens the stars its view keeps. The tree shows them all, so it
// lights nothing apart.
const own=node=>node.ownerType==='personal-hypothesis'||String(node.id).startsWith('personal:');

/**
 * The ids of the graph's stars that lens `id` shows, or null for the tree. `gathering` is the lens's gathering
 * where it gathers; `view()` is its lens view otherwise (asked only when needed).
 */
export function lensMembers(id,{graph,timeline=null,geography=null,starIds=[],gathering=null,view=null}){
  if(id==='tree')return null;
  const placed=(map,node)=>!!(node.materialId&&map?.get?.(node.materialId));
  let keep;
  if(id==='time'&&timeline)keep=node=>placed(timeline.positions,node);
  else if(id==='space'&&geography)keep=node=>placed(geography.positions,node);
  else if(id==='proximity'){const stars=new Set(starIds);keep=node=>stars.has(node.materialId)||stars.has(node.id);}
  else if(gathering)keep=node=>placed(gathering.positions,node)||own(node);
  else{const ids=new Set((view?.()?.nodes??[]).map(node=>node.id));keep=node=>ids.has(node.id);}
  return new Set((graph?.nodes??[]).filter(keep).map(node=>node.id));
}
