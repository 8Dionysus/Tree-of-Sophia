// The retained library's map and the editorial annotations of its texts come when they are first needed: a text opens
// in the reader, a search labels its matches, or Antiquity binds its collection to public places. A page that only
// shows the Zarathustra space carries neither. Until they have come, readers see no retained apparatus and no
// editorial ranges, so each of those paths waits for them before it shows anything.
let map=null,annotations=null,mapLoading=null,annotationsLoading=null,library=null;
const settle=(load,keep,reset)=>load().then(module=>keep(module.default),error=>{reset();throw error;});
export function loadRetainedMap(){
  return mapLoading??=settle(()=>import('../../../public-corpus/retained-library-map.v1.json',{with:{type:'json'}}),value=>map=value,()=>mapLoading=null);
}
function loadAnnotations(){
  return annotationsLoading??=settle(()=>import('../../../public-corpus/editorial-annotations.v1.json',{with:{type:'json'}}),value=>annotations=value,()=>annotationsLoading=null);
}
export const retainedLibrary=()=>library;
export function loadRetainedLibrary(){
  return Promise.all([loadRetainedMap(),loadAnnotations()]).then(()=>library??={map,annotations});
}
