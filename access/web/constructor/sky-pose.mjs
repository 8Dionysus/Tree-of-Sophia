// Presentation coordinates are independent of query/source identity.
export function validateSkyPose(value){
  const fail=()=>{throw new TypeError('Invalid saved camera position.');};
  const number=(v,limit=100000)=>{if(!Number.isFinite(v)||Math.abs(v)>limit)fail();return v;};
  const vector=(v,length)=>{if(!Array.isArray(v)||v.length!==length)fail();return v.map(n=>number(n));};
  if(value?.v!==1)fail();
  const zoom=number(value.zoom),fit=number(value.fit);
  if(zoom<.65||zoom>2.2||fit<=0||fit>10)fail();
  return {v:1,yaw:number(value.yaw,1e6),pitch:number(value.pitch,Math.PI),zoom,pan:vector(value.pan,2),center:vector(value.center,3),fit};
}
