// A page-owned response packet is retained by reference for exact restoration.
// Comparing camera/selection changes must not traverse its source text. A new
// response is a distinct history boundary even when its fingerprint is equal;
// query metadata can belong to that specific packet outside its JSON fields.
export function sameSceneView(left,right){
  if(!left||!right||left.graph.packet!==right.graph.packet)return false;
  const pose=value=>{
    const {graph,...view}=value;
    const {packet,...geometry}=graph;
    return JSON.stringify({...view,graph:geometry});
  };
  return pose(left)===pose(right);
}
