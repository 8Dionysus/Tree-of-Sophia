import {describe,expect,it} from 'vitest';
import {CONTROLS_KEY,controlsDismissed,controlsFor,controlsShouldOpen,rememberControls} from './space-controls.mjs';

const memoryStorage=()=>{const data=new Map();return {getItem:key=>data.has(key)?data.get(key):null,setItem:(key,value)=>data.set(key,String(value)),removeItem:key=>data.delete(key),data};};
const brokenStorage={getItem(){throw new Error('blocked');},setItem(){throw new Error('blocked');},removeItem(){throw new Error('blocked');}};

describe('the controls window remembers "don’t show again"',()=>{
  it('opens on a first visit to a space that turns',()=>{
    expect(controlsShouldOpen({storage:memoryStorage()})).toBe(true);
  });
  it('stays closed once the reader asked not to see it again, on every later visit',()=>{
    const storage=memoryStorage();
    expect(rememberControls(storage,true)).toBe(true);
    expect(storage.data.get(CONTROLS_KEY)).toBe('hidden');
    expect(controlsDismissed(storage)).toBe(true);
    for(let visit=0;visit<3;visit++)expect(controlsShouldOpen({storage})).toBe(false);
  });
  it('comes back when the box is unticked again',()=>{
    const storage=memoryStorage();rememberControls(storage,true);
    expect(rememberControls(storage,false)).toBe(true);
    expect(controlsShouldOpen({storage})).toBe(true);
  });
  it('does not open by itself on a flat lens, twice in one visit, or where nothing can be remembered',()=>{
    expect(controlsShouldOpen({storage:memoryStorage(),flat:true})).toBe(false);
    expect(controlsShouldOpen({storage:memoryStorage(),opened:true})).toBe(false);
    expect(controlsShouldOpen({storage:brokenStorage})).toBe(false);
    expect(controlsShouldOpen({storage:null})).toBe(false);
    expect(controlsDismissed(brokenStorage)).toBe(false);
    expect(rememberControls(brokenStorage,true)).toBe(false);
  });
});

describe('the controls window names gestures, not buttons',()=>{
  it('shows the gestures of a mouse and keyboard',()=>{
    const rows=controlsFor('mouse','ru');
    expect(rows.map(row=>row.sign)).toEqual(['drag','pan','wheel','hold','arrows','click']);
    expect(rows[0]).toEqual({sign:'drag',gesture:'Тянуть',action:'повернуть пространство'});
  });
  it('shows the gestures of fingers on a touch screen',()=>{
    const rows=controlsFor('touch','en');
    expect(rows.map(row=>row.sign)).toEqual(['finger','fingers','pinch','hold','tap']);
    expect(rows[0]).toEqual({sign:'finger',gesture:'One finger',action:'turn the space'});
  });
  it('speaks the interface language beyond Russian and English',()=>{
    expect(controlsFor('mouse','de')[0]).toEqual({sign:'drag',gesture:'Ziehen',action:'den Raum drehen'});
    expect(controlsFor('touch','es')[2]).toEqual({sign:'pinch',gesture:'Juntar o separar los dedos',action:'acercar y alejar'});
    expect(controlsFor('mouse','zh-Hans')[5].gesture).toBe('点击');
  });
});
