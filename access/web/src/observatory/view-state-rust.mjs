import {t} from './ui-i18n.mjs';

let normalize;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const packet=value=>JSON.stringify(value,(_key,item)=>typeof item==='number'&&!Number.isFinite(item)?'__nonfinite_json_number__':item);
const id=value=>typeof value==='string'&&value.length>1024?value.slice(0,1025):value;
const vector=value=>Array.isArray(value)?value.slice(0,4):value;
const vertex=value=>value&&typeof value==='object'?{id:id(value.id),slot:value.slot,p:vector(value.p),
  sourcePosition:vector(value.sourcePosition),target:vector(value.target),volumeZ:value.volumeZ}:value;
const projection=value=>value&&typeof value==='object'?{lens:value.lens,yaw:value.y,pitch:value.pitch,zoom:value.zoom,
  pan:value.pan&&typeof value.pan==='object'?{x:value.pan.x,y:value.pan.y}:value.pan,
  selectedId:id(value.selectedId),relationId:id(value.relationId),panelOpen:value.panelOpen,cardTab:value.cardTab,
  vertices:Array.isArray(value.vertices)?value.vertices.slice(0,41).map(vertex):value.vertices}:value;
const signedZero=(source,target,key)=>{if(Object.is(source?.[key],-0))target[key]=-0;};
const vectorZero=(source,target)=>{for(let i=0;i<3;i++)if(Object.is(source?.[i],-0))target[i]=-0;};

export function installPoseRules(runtime){
  if(typeof runtime?.normalize_observatory_pose_wasm_v1!=='function')throw new TypeError('Observatory pose WASM rule is unavailable');
  normalize=runtime.normalize_observatory_pose_wasm_v1;
}

export function normalizePose(value){
  if(!normalize)return null;
  try{
    const result=JSON.parse(decoder.decode(normalize(encoder.encode(packet(projection(value))))));
    // JSON.stringify loses negative zero. Restore its sign in the bounded
    // numeric fields of the already validated Rust projection.
    for(const key of ['yaw','pitch','zoom'])signedZero(value,result,key);
    for(const key of ['x','y'])signedZero(value.pan,result.pan,key);
    result.vertices.forEach((vertex,index)=>{
      const original=value.vertices[index];
      for(const key of ['slot','volumeZ'])signedZero(original,vertex,key);
      for(const key of ['p','sourcePosition','target'])vectorZero(original[key],vertex[key]);
      vectorZero(original.target,vertex.pos);
    });
    return result;
  }catch(error){
    throw new Error(t(String(error).includes('invalid_pose_vertices')?
      'Сохранённое расположение звёзд повреждено.':'Не удалось прочитать положение сохранённого места.'),{cause:error});
  }
}
