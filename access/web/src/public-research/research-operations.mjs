// Research operations run on the caller's explicitly selected, versioned data.
// The three formal operators are implemented once by tos-query and bound through WASM.
import {jaccardDistance,spanMiddle} from './space-proximity.mjs';
import {greatCircleKm} from './space-geography.mjs';

let kernel;
const encoder=new TextEncoder(),decoder=new TextDecoder();
export function configureResearchOperations(runtime){
  if(typeof runtime?.research_operation_wasm_v1!=='function')throw new TypeError('Research operation WASM rule is unavailable');
  kernel=runtime.research_operation_wasm_v1;
}
const formal=new Set(['signed_support','reading_distance','commitment_order']);
const unique=items=>[...new Set(items)];
const array=(value,name)=>{if(!Array.isArray(value))throw new TypeError(`${name} must be an array`);return value;};
const required=(value,name)=>{if(value===undefined||value===null||value==='')throw new TypeError(`${name} required`);return value;};

/** A reproducible request is {operation, scope, versions, input}; data and provenance live in input. */
export function runResearchOperation(request){
  required(request?.operation,'operation');required(request.scope,'scope');required(request.versions,'versions');
  const input=required(request.input,'input');
  if(formal.has(request.operation)){
    if(!kernel)throw new Error('Research operation WASM rule is not configured');
    return JSON.parse(decoder.decode(kernel(encoder.encode(JSON.stringify(request)))));
  }
  let result,explanation;
  switch(request.operation){
    case 'distribution': {
      const items=array(input.items,'items'),matches=items.filter(item=>item.matches===true);
      required(input.witness,'witness');required(input.search_kind,'search_kind');
      const groups={};for(const item of matches){const key=item.group??'unknown';groups[key]=(groups[key]??0)+1;}
      result={witness:input.witness,search_kind:input.search_kind,denominator:items.length,count:matches.length,groups,locations:matches.map(item=>item.location),unlocated:matches.filter(item=>!item.location).length};
      explanation='Count of explicitly matched items in the supplied selection; denominator is the number of supplied items.';break;
    }
    case 'directed_path': {
      const edges=array(input.edges,'edges'),allowed=new Set(array(input.allowed_types,'allowed_types'));required(input.from,'from');required(input.to,'to');
      const queue=[input.from],visited=new Set(queue),previous=new Map();
      for(let i=0;i<queue.length&&!visited.has(input.to);i++)for(const edge of edges){
        if(edge.from!==queue[i]||!allowed.has(edge.type)||visited.has(edge.to))continue;
        visited.add(edge.to);previous.set(edge.to,edge);queue.push(edge.to);
      }
      const path=[];if(visited.has(input.to)){let at=input.to;while(at!==input.from){const edge=previous.get(at);path.unshift(edge);at=edge.from;}}
      result={found:visited.has(input.to),visited_nodes:visited.size,transitions:path.length,edges:path};
      explanation='Shortest directed path over supplied edges of the allowed types; each returned edge retains its own provenance.';break;
    }
    case 'common_grounds': {
      const left=unique(array(input.left,'left')),right=unique(array(input.right,'right')),l=new Set(left),r=new Set(right);
      result={common:left.filter(x=>r.has(x)),only_left:left.filter(x=>!r.has(x)),only_right:right.filter(x=>!l.has(x)),jaccard_distance:jaccardDistance(left,right)};
      explanation='Exact identity intersection and differences; Jaccard distance is unknown when either selection is empty.';break;
    }
    case 'interpretation_comparison': {
      const axes=array(input.axes,'axes'),left=required(input.left,'left'),right=required(input.right,'right');
      result={features:axes.map(axis=>{const a=left[axis.id]??null,b=right[axis.id]??null;return {axis,left:a,right:b,status:a===null||b===null?'unknown':a===b?'same':'different'};})};
      explanation='Comparison of declared features; missing values are unknown and do not imply absence.';break;
    }
    case 'argument_structure': {
      const steps=array(input.steps,'steps'),objections=array(input.objections??[],'objections');
      result={steps,objections,unattached_objections:objections.filter(o=>!steps.some(s=>s.id===o.target_step))};
      explanation='Supplied premises, conditional transitions and conclusions with objections addressed to their exact steps; no inference is added.';break;
    }
    case 'dependency_impact': {
      const edges=array(input.dependencies,'dependencies'),start=required(input.changed,'changed'),queue=[start],seen=new Set(queue),paths=[];
      for(let i=0;i<queue.length;i++)for(const edge of edges)if(edge.from===queue[i]){paths.push(edge);if(!seen.has(edge.to)){seen.add(edge.to);queue.push(edge.to);}}
      result={changed:start,affected:queue.slice(1),dependencies:paths};explanation='Reachable dependants along declared dependency direction; this identifies impact, not automatic invalidation.';break;
    }
    case 'time_place': {
      const left=required(input.left,'left'),right=required(input.right,'right'),a=spanMiddle(left.span),b=spanMiddle(right.span);
      const validPoint=p=>Array.isArray(p)&&p.length===2&&p.every(Number.isFinite)&&Math.abs(p[0])<=180&&Math.abs(p[1])<=90;
      result={left,right,time_midpoint_distance_years:a===null||b===null?null:Math.abs(a-b),place_distance_km:validPoint(left.point)&&validPoint(right.point)?greatCircleKm(left.point,right.point):null};
      explanation='Absolute difference between supplied interval midpoints in years and great-circle distance in kilometres; interval bounds, uncertainty and source stay in the inputs.';break;
    }
    default:throw new TypeError(`Unknown research operation: ${request.operation}`);
  }
  return {request,rule_version:'tos-research-operations-web@1',result,explanation};
}
