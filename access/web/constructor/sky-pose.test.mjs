import {test} from 'vitest';
import assert from 'node:assert/strict';
import {validateSkyPose} from './sky-pose.mjs';
test('saved camera coordinates are bounded and independent of graph payloads',()=>{
  const pose={v:1,yaw:1,pitch:0,zoom:1.4,pan:[10,20],center:[-30,25,8],fit:1.2};
  assert.deepEqual(validateSkyPose({...pose,text:'not a camera property'}),pose);
  for(const change of [{zoom:0},{fit:Infinity},{center:[0,0]},{pan:[0,NaN]},{pitch:100}])assert.throws(()=>validateSkyPose({...pose,...change}));
});
