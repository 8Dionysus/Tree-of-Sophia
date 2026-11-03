import {expect,test,vi} from 'vitest';
import {toggleResearchFullscreen} from './public-space.mjs';

test('fullscreen keeps the shared header and map together and can be exited',async()=>{
  const header={search:{},language:{}},map={};
  const doc={fullscreenElement:null,exitFullscreen:vi.fn(async()=>{doc.fullscreenElement=null;})};
  const host={children:[header,map],requestFullscreen:vi.fn(async()=>{doc.fullscreenElement=host;})};
  await toggleResearchFullscreen(host,doc);
  expect(doc.fullscreenElement.children).toEqual([header,map]);
  expect(host.requestFullscreen).toHaveBeenCalledOnce();
  await toggleResearchFullscreen(host,doc);
  expect(doc.exitFullscreen).toHaveBeenCalledOnce();
  expect(doc.fullscreenElement).toBeNull();
});

test('fullscreen rejection and an unavailable API do not break the map',async()=>{
  await expect(toggleResearchFullscreen({requestFullscreen:()=>Promise.reject(new Error('denied'))},{fullscreenElement:null})).resolves.toBeUndefined();
  expect(toggleResearchFullscreen({},{fullscreenElement:null})).toBeUndefined();
});
