import {expect,test} from 'vitest';
import {filterSteps,paperOf,throughSteps} from './sky-paper.mjs';

const DAY='invert(1) hue-rotate(180deg) contrast(1.25) saturate(1.25)';

test('the day filter becomes the colour steps the page applies, clamped after each',()=>{
  const steps=filterSteps(DAY);
  expect(steps).toHaveLength(4);
  const close=(value,expected)=>value.forEach((channel,i)=>expect(channel).toBeCloseTo(expected[i],6));
  close(throughSteps(steps,[0,0,0]),[1,1,1]);
  close(throughSteps(steps,[1,1,1]),[0,0,0]);
  close(throughSteps(steps,[.5,.5,.5]),[.5,.5,.5]);
  // Turned over, a dark blue of the night is a light blue of the day: the lightness turns, the hue stays.
  const [r,g,b]=throughSteps(steps,[.2,.3,.6]);expect(b).toBeGreaterThan(r);expect(r+g+b).toBeGreaterThan(.2+.3+.6);
  expect(filterSteps('invert(100%) hue-rotate(.5turn) contrast(125%) saturate(1.25)')).toEqual(steps);
});

test('a filter or paper the painter cannot reproduce stays with the page',()=>{
  expect(filterSteps('none')).toBeNull();
  expect(filterSteps('invert(1) blur(2px)')).toBeNull();
  expect(filterSteps('drop-shadow(0 0 2px black)')).toBeNull();
  expect(paperOf('none','rgba(238, 233, 222, 0.5)')).toBeNull();
  expect(paperOf('url("paper.png")','rgb(238, 233, 222)')).toBeNull();
  expect(paperOf('linear-gradient(red, blue)','white')).toBeNull();
});

test('the paper is the ground colour, or the lamp of colour stops the page computes',()=>{
  expect(paperOf('none','rgb(238, 233, 222)')).toEqual({stops:[[238/255,233/255,222/255,0]],center:[.5,.5],radii:[1,1]});
  const lamp=paperOf('radial-gradient(62% 58% at 54% 46%, rgb(253, 251, 246) 0%, rgb(248, 244, 236) 45%, rgb(240, 233, 221) 80%, rgb(233, 224, 209) 100%)','rgb(244, 239, 228)');
  expect(lamp.center).toEqual([.54,.46]);expect(lamp.radii).toEqual([.62,.58]);
  expect(lamp.stops.map(stop=>stop[3])).toEqual([0,.45,.8,1]);
  expect(paperOf('radial-gradient(ellipse 62% 58% at 54% 46%, #FDFBF6 0%, #E9E0D1 100%)','white').stops).toHaveLength(2);
});
