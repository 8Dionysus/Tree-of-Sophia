const bounded=(v,min,max)=>typeof v==='number'&&Number.isFinite(v)&&v>=min&&v<=max;
const id=v=>v===null||typeof v==='string'&&v.length>0&&v.length<=1024;
const vector=v=>Array.isArray(v)&&v.length===3&&v.every(n=>bounded(n,-10000,10000));
export function validatePose(value){
  if(!value||!['constellations','plane','orbits'].includes(value.lens)||!bounded(value.yaw,-100000,100000)||!bounded(value.pitch,-2,2)||!bounded(value.zoom,.1,5)
    ||!bounded(value.pan?.x,-100000,100000)||!bounded(value.pan?.y,-100000,100000)||!id(value.selectedId)||!id(value.relationId)
    ||typeof value.panelOpen!=='boolean'||!['about','relations'].includes(value.cardTab)||!Array.isArray(value.vertices)||value.vertices.length>40)throw new Error('Не удалось прочитать положение сохранённого места.');
  const ids=new Set();
  const vertices=value.vertices.map(v=>{
    if(!id(v.id)||!v.id||ids.has(v.id)||!Number.isInteger(v.slot)||v.slot<0||v.slot>1000||!vector(v.p)||!vector(v.sourcePosition)||!vector(v.target)||!bounded(v.volumeZ,-10000,10000))throw new Error('Сохранённое расположение звёзд повреждено.');
    ids.add(v.id);return {id:v.id,slot:v.slot,p:[...v.p],sourcePosition:[...v.sourcePosition],target:[...v.target],pos:[...v.target],volumeZ:v.volumeZ};
  });
  return {lens:value.lens,yaw:value.yaw,pitch:value.pitch,zoom:value.zoom,pan:{x:value.pan.x,y:value.pan.y},selectedId:value.selectedId,relationId:value.relationId,panelOpen:value.panelOpen,cardTab:value.cardTab,vertices};
}
