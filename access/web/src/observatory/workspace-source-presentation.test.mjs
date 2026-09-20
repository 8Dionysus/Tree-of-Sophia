import {afterEach,expect,test,vi} from 'vitest';
import {sourceReferenceLink,humanSourceStatus} from './workspace.mjs';
import {setUiLanguage} from './ui-i18n.mjs';
class Element{
  static TEXT_NODE=3;
  constructor(tag){this.tagName=tag;this.nodeType=1;this._text='';this.children=[];this.events={};this.classList={add:()=>{}};}
  set textContent(value){this._text=String(value);this.children=[];}
  get textContent(){return this._text+this.children.map(value=>value.textContent).join('');}
  append(...values){this.children.push(...values);}
  addEventListener(name,handler){this.events[name]=handler;}
  click(){this.events.click?.();}
}
afterEach(()=>{setUiLanguage('ru');vi.useRealTimers();vi.unstubAllGlobals();});
test('a non-HTTP source reference is downloadable without a path dump',async()=>{
  vi.stubGlobal('Node',Element);vi.stubGlobal('document',{documentElement:{},dispatchEvent:()=>{},createElement:tag=>new Element(tag),createTextNode:text=>{const node=new Element('#text');node.nodeType=3;node.textContent=text;return node;}});vi.useFakeTimers();let blob;
  vi.stubGlobal('URL',{createObjectURL:value=>{blob=value;return 'blob:source';},revokeObjectURL:vi.fn()});
  const ref='ToS/source-witnesses/works/a/source.json',button=sourceReferenceLink(ref);
  expect(button.tagName).toBe('button');expect(button.textContent).toContain('source.json');expect(button.textContent).not.toContain('ToS/');
  button.click();expect(JSON.parse(await blob.text())).toEqual({source_ref:ref});vi.runOnlyPendingTimers();
});
test('rights and request states stay distinct across interface languages',()=>{
  vi.stubGlobal('document',{documentElement:{},dispatchEvent:()=>{}});vi.stubGlobal('CustomEvent',class{});
  const states=['public-domain','open-licensed','permission-granted','research-only','restricted','rights-unknown','rejected',
    'draft-not-sent','awaiting-human-send-approval','sent','response-received','permission-denied','expired','withdrawn'];
  for(const language of ['ru','en','es']){
    setUiLanguage(language);const labels=states.map(value=>String(humanSourceStatus(value)));
    expect(new Set(labels).size).toBe(states.length);
    expect(labels.some(value=>states.includes(value))).toBe(false);
    expect(String(humanSourceStatus('future-owner-state'))).toContain('future-owner-state');
    expect(String(humanSourceStatus(null))).not.toContain('Unavailable');
  }
});

test('exact-read failures retain distinct actionable status labels',()=>{
  vi.stubGlobal('document',{documentElement:{},dispatchEvent:()=>{}});vi.stubGlobal('CustomEvent',class{});
  for(const language of ['ru','en','es']){
    setUiLanguage(language);
    const labels=['access-restricted','corrupt','over-budget','unsupported','missing'].map(value=>String(humanSourceStatus(value)));
    expect(new Set(labels).size).toBe(5);
    expect(labels.join(' ')).not.toMatch(/позже|try later/i);
  }
});
