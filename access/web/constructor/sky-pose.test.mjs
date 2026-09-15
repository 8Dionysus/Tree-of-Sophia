import {test} from 'vitest';
import assert from 'node:assert/strict';
import {validateSkyPose} from './sky-pose.mjs';
import {StableExplorationLayout} from './live-model.mjs';
test('saved camera coordinates are bounded and independent of graph payloads',()=>{
  const pose={v:1,yaw:1,pitch:0,zoom:1.4,pan:[10,20],center:[-30,25,8],fit:1.2};
  assert.deepEqual(validateSkyPose({...pose,text:'not a camera property'}),pose);
  for(const change of [{zoom:0},{fit:Infinity},{center:[0,0]},{pan:[0,NaN]},{pitch:100}])assert.throws(()=>validateSkyPose({...pose,...change}));
});
test('restoring layout positions is atomic and copies coordinates without fabricating source facts',()=>{
  const layout=new StableExplorationLayout(),value={v:1,serial:2,positions:[['one',[1,2,3]],['two',[4,5,6]]]};layout.restore(value);
  value.positions[0][1][0]=999;assert.deepEqual(layout.position('one'),[1,2,3]);
  for(const bad of [{...value,serial:1},{...value,positions:[['one',[1,2,3]],['one',[4,5,6]]]},{...value,positions:[['one',[NaN,2,3]]]}]){
    assert.throws(()=>layout.restore(bad));assert.deepEqual(layout.position('one'),[1,2,3]);
  }
  const copy=new StableExplorationLayout();copy.restore(layout.capture());assert.deepEqual(copy.capture(),layout.capture());
});
