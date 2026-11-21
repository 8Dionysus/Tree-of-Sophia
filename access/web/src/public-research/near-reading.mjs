// Reading up close (J1): how much of a chosen star's text stands beside it in the sky at a zoom. Nothing at the zoom
// of the whole map; the sentence the star rests on once the reader comes near; its places nearer still. A route, a
// measured lens and the workbench keep their own reading, so they show none of it.
export const NEAR_ZOOM=Object.freeze([2.6,4.2]);

/** 0, 1 (the sentence) or 2 (the places) for a view: its zoom and what it shows. */
export function nearLevel({zoom=1,selected=null,route=false,measured=false,bench=false}={}){
  if(!selected||route||measured||bench||!Number.isFinite(zoom))return 0;
  return zoom>=NEAR_ZOOM[1]?2:zoom>=NEAR_ZOOM[0]?1:0;
}
/** How many places a level shows. */
export const nearCount=level=>level===2?3:level===1?1:0;
