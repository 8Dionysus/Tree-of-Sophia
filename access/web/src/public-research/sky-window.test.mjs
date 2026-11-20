import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {mountSkyWindow,orientationTurn,sensorAllowed} from './sky-window.mjs';

test('a turn of the phone becomes a turn of the space, as the screen stands; a sensor\'s jump is no turn',()=>{
  const at=(alpha,beta,gamma=0)=>({alpha,beta,gamma});
  const turn=orientationTurn(at(10,40),at(20,35));
  expect(turn.yaw).toBeCloseTo(-10*Math.PI/180);expect(turn.pitch).toBeCloseTo(5*Math.PI/180);
  // Across north the heading wraps.
  expect(orientationTurn(at(355,0),at(5,0)).yaw).toBeCloseTo(-10*Math.PI/180);
  // A landscape screen reads the tilt from the roll.
  expect(orientationTurn(at(0,0,10),at(0,0,20),{angle:90}).pitch).toBeCloseTo(10*Math.PI/180);
  expect(orientationTurn(at(0,0),at(90,0))).toEqual({yaw:0,pitch:0});
  expect(orientationTurn(null,at(1,1))).toEqual({yaw:0,pitch:0});
});

test('the window says the site closes the sensor, and turns the space where the sensor is allowed',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const say=vi.fn(),turnBy=vi.fn();
  const view={DeviceOrientationEvent:class{},requestAnimationFrame:callback=>{callback();return 1;},cancelAnimationFrame(){},screen:{orientation:{angle:0}},
    addEventListener:window.addEventListener.bind(window),removeEventListener:window.removeEventListener.bind(window)};
  Object.defineProperty(document,'permissionsPolicy',{value:{allowsFeature:feature=>feature!=='gyroscope'},configurable:true});
  expect(sensorAllowed(document)).toBe(false);
  const sky=mountSkyWindow({shell:document.querySelector('#tree'),actionLayer:{addActions:()=>()=>{},say},space:{actions:{turnBy}},locale:()=> 'en',view});
  expect(await sky.toggle(true)).toBe(false);
  expect(say).toHaveBeenLastCalledWith(expect.stringContaining('closed by the permissions policy'));
  Object.defineProperty(document,'permissionsPolicy',{value:{allowsFeature:()=>true},configurable:true});
  expect(await sky.toggle(true)).toBe(true);
  expect(document.querySelector('.ps-sky-window').hidden).toBe(false);
  const orient=(alpha,beta)=>window.dispatchEvent(Object.assign(new window.Event('deviceorientation'),{alpha,beta,gamma:0}));
  orient(10,30);orient(20,30);
  expect(turnBy).toHaveBeenCalledTimes(1);expect(turnBy.mock.calls[0][0]).toBeCloseTo(-10*Math.PI/180);
  await sky.toggle(false);orient(40,30);
  expect(turnBy).toHaveBeenCalledTimes(1);
  sky.destroy();
});
