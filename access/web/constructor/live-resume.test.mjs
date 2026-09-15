import {test,expect} from 'vitest';
import {makeLiveResume,validateLiveResume} from './live-resume.mjs';
import {StableExplorationLayout} from './live-model.mjs';
import {ExplorationSceneCache} from '../src/observatory/exploration-cache.mjs';
import {pageFixture} from '../src/observatory/exploration-test-fixtures.mjs';

test('resume preserves a bounded query and geometry, without source packets or cursors',()=>{
  const cache=new ExplorationSceneCache(),first=pageFixture();cache.accept(cache.begin(first.query),first);const view=cache.snapshot();
  const presentation={layout:new StableExplorationLayout().capture(),pose:{v:1,yaw:0,pitch:0,zoom:1,pan:[3,4],center:[0,0,0],fit:1},mode:'compact'};
  const saved=makeLiveResume({view,selection:view.selection,areaKind:'exploration'},presentation);
  expect(saved.area.type).toBe('route');expect(saved.area.target.origin.id).toBe(first.origin.id);
  expect(saved.presentation.pose.pan).toEqual([3,4]);expect(JSON.stringify(saved)).not.toContain('next_cursor');
  expect(JSON.stringify(saved)).not.toContain('source_refs');expect(validateLiveResume(saved)).toEqual(saved);
  expect(()=>validateLiveResume({...saved,cursor:'opaque'})).toThrow();
  expect(()=>validateLiveResume({...saved,sourceRevision:'b'.repeat(64)})).toThrow();
  expect(()=>validateLiveResume({...saved,presentation:{...presentation,pose:{...presentation.pose,zoom:Infinity}}})).toThrow();
  expect(()=>validateLiveResume({...saved,area:{type:'route',target:{...saved.area.target,options:{...saved.area.target.options,cursor:'opaque'}}}})).toThrow();
});
