import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {actLines,mountDeviceHandoff} from './device-handoff.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';

test('the act journal leaves as JSON lines, oldest first, every record whole',async()=>{
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  for(let i=0;i<7;i++)await acts.act({kind:'note',targets:[`t${i}`],grounds:[],intent:`n${i}`,body:{text:`n${i}`}});
  const lines=(await actLines(acts,{page:3})).trim().split('\n').map(line=>JSON.parse(line));
  expect(lines.map(record=>record.intent)).toEqual(['n0','n1','n2','n3','n4','n5','n6']);
  expect(lines[0].digest).toMatch(/^sha256:|^[0-9a-f]{64}$/);
});

test('the sheet shows the link, a code for it as an image of its own, and the exports',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/?lens=time'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const added=[],actionLayer={addActions:list=>{added.push(...list);return ()=>{};},linkTo:()=> 'http://localhost/zarathustra/?lens=time&item=child',currentTarget:()=>({type:'star',id:'child'}),copy:vi.fn(async()=>true),say:vi.fn()};
  const exportWork=vi.fn(async()=>{});
  const handoff=mountDeviceHandoff({shell:document.querySelector('#tree'),actionLayer,acts:null,exportWork,locale:()=> 'en'});
  expect(added.map(action=>action.id)).toEqual(['continue-elsewhere']);
  const {href}=handoff.open();
  expect(href).toBe('http://localhost/zarathustra/?lens=time&item=child');
  expect(document.querySelector('.dh-link').value).toBe(href);
  const image=document.querySelector('.dh-code');
  expect(image.getAttribute('src')).toMatch(/^data:image\/svg\+xml;charset=utf-8,/);
  expect(decodeURIComponent(image.getAttribute('src'))).toContain('h1v1h-1z');
  [...document.querySelectorAll('button')].find(node=>node.textContent==='Copy the link').click();
  expect(actionLayer.copy).toHaveBeenCalledWith(href);
  [...document.querySelectorAll('button')].find(node=>node.textContent==='Complete export of the work').click();
  expect(exportWork).toHaveBeenCalled();
});
