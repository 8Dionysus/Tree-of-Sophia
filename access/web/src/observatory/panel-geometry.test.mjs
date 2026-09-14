import {test,expect} from 'vitest';
import {windowBounds,windowFraction,windowPoint} from './panel-geometry.mjs';
test('saved window fractions remain reachable across desktop and phone viewports',()=>{
  const desktop=windowBounds({width:1280,height:800,header:86,footer:90},{width:430,height:440});
  const saved=windowFraction({x:520,y:205},desktop);expect(windowPoint(saved,desktop)).toEqual({x:520,y:205});
  const phone=windowBounds({width:390,height:844,header:72,footer:90},{width:360,height:350});
  const p=windowPoint(saved,phone);expect(p.x).toBe(15);expect(p.y).toBeGreaterThanOrEqual(84);expect(p.y+350).toBeLessThanOrEqual(754);
  expect(windowFraction({x:-200,y:2000},desktop)).toEqual({x:0,y:1});
});
