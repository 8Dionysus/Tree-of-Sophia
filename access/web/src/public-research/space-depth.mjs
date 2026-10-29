// The lenses that group are read in depth, as the stand's bonsai declares:
// each star lies at the depth of its step of interpretation (a source
// deepest, a reading before it, a comparison of readings before that, your
// own record nearest), and each step that holds stars lies on a plane named
// by the step, which the space shows while it turns. Like a scale, depth is a
// view: positions are never written into the graph.

import {localeValue} from './locale.mjs';

/**
 * Stars laid in depth by their step, and the planes of the steps that hold
 * them, deep to near. `stepOf(node)` names a star's step; a star of a step
 * the bonsai does not declare lies at depth 0 on no plane. One step alone
 * needs no planes.
 */
export function depthLayout(nodes=[],{stepOf=()=>null,depth=null,locale='ru',margin=[46,40]}={}){
  if(!Array.isArray(depth?.steps)||!depth.steps.length)return {nodes,planes:[]};
  const declared=new Map(depth.steps.map(step=>[step.id,step]));
  const held=new Map();
  const laid=nodes.map(node=>{
    const step=stepOf(node),z=declared.get(step)?.z??0;
    if(!held.has(step))held.set(step,[]);held.get(step).push(node.position);
    return {...node,position:[node.position[0],node.position[1],z]};
  });
  // Only a declared step has a plane; a star of another lies at depth 0 without one.
  const named=[...held].filter(([step])=>declared.has(step));
  if(named.length<2)return {nodes:laid,planes:[]};
  const planes=named.map(([step,points])=>{
    const xs=points.map(point=>point[0]),ys=points.map(point=>point[1]);
    return {id:`step:${step}`,z:declared.get(step)?.z??0,x:[Math.min(...xs)-margin[0],Math.max(...xs)+margin[0]],y:[Math.min(...ys)-margin[1],Math.max(...ys)+margin[1]],label:localeValue(declared.get(step)?.label,locale)};
  }).sort((a,b)=>a.z-b.z);
  return {nodes:laid,planes};
}
