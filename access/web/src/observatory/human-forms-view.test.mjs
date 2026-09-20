import {afterEach,expect,test,vi} from 'vitest';
import {renderHumanForms} from './human-forms-view.mjs';
import {formNode,ref} from '../../fixtures/human-form-data.mjs';

// This is a small DOM model for the presentation contract. Native details
// behavior, focus order and reduced-motion rendering remain browser checks;
// these tests assert grouping, visibility and retained reading anchors.
class Element{
  static TEXT_NODE=3;
  constructor(tag,nodeType=1){this.tagName=tag;this.nodeType=nodeType;this.children=[];this.parentNode=null;this.dataset={};this.attributes={};this.listeners={};this._text='';this.className='';this.hidden=false;this.open=false;}
  set textContent(value){this._text=String(value??'');this.children=[];}
  get textContent(){return this._text+this.children.map(value=>value.textContent).join('');}
  append(...values){for(const value of values){if(value===null||value===undefined)continue;value.parentNode=this;this.children.push(value);}}
  remove(){if(!this.parentNode)return;const index=this.parentNode.children.indexOf(this);if(index>=0)this.parentNode.children.splice(index,1);this.parentNode=null;}
  setAttribute(name,value){this.attributes[name]=String(value);}
  getAttribute(name){return this.attributes[name]??null;}
  addEventListener(type,listener){this.listeners[type]=listener;}
  dispatchEvent(event){this.listeners[event.type]?.(event);}
}

const descendants=node=>[node,...node.children.flatMap(descendants)];
const one=(node,predicate)=>descendants(node).find(predicate);
const roleSections=node=>descendants(node).filter(value=>value.tagName==='section'&&value.dataset.formRole);
const detailsFor=node=>descendants(node).filter(value=>value.tagName==='details');

function dom(){
  vi.stubGlobal('Node',Element);
  vi.stubGlobal('document',{createElement:tag=>new Element(tag),createTextNode:text=>{const node=new Element('#text',Element.TEXT_NODE);node.textContent=text;return node;}});
}

afterEach(()=>vi.unstubAllGlobals());

test('shows the statement and populated supporting material without transport diagnostics',()=>{
  dom();const raw=formNode(),before=structuredClone(raw),view=renderHumanForms(raw);
  expect(roleSections(view).map(value=>value.dataset.formRole)).toEqual(['statement','grounds','history']);
  const statement=roleSections(view)[0];expect(statement.parentNode).toBe(view);
  expect(one(statement,value=>value.dataset.readingAnchor==='form:statement:wording')).toBeTruthy();
  expect(statement.textContent).toContain('НЕ доказано');
  expect(statement.textContent).toContain('Отрицание');
  expect(detailsFor(view).map(value=>value.dataset.formGroup)).toEqual(['grounds','history']);
  expect(descendants(view).some(value=>value.tagName==='pre')).toBe(false);
  expect(view.textContent).not.toContain('tos.form.');expect(view.textContent).not.toContain('семантического принятия');
  expect(raw).toEqual(before);
});

test('keeps supplied qualification and distinguishes an incomplete context',()=>{
  dom();const view=renderHumanForms(formNode(),{readableContext:{state:'requires-exact-context',contexts:[]}});
  const statement=one(view,value=>value.dataset.formRole==='statement');
  expect(statement.textContent).toContain('НЕ доказано');
  expect(statement.textContent).not.toContain('Часть контекста доступна в источнике.');
  expect(one(statement,value=>value.dataset.contextPresentation==='requires-exact-context')).toBeTruthy();
  expect(one(statement,value=>value.dataset.contextSlot==='qualification')).toBeTruthy();
  expect(descendants(view).some(value=>value.tagName==='pre')).toBe(false);
});

test('omits absent roles and offers the complete bounded text without candidate IDs',()=>{
  dom();const raw=formNode(),selection=raw.human_form_selection;
  for(const role of ['name','caption','hover','history','technical'])selection.roles[role]={state:'missing',reason:'no-ready-form',form:null,packet:null};
  selection.roles.grounds.state='over-budget';selection.roles.grounds.reason='inspect-exact-form';selection.roles.grounds.packet=null;
  const view=renderHumanForms(raw),sections=roleSections(view);
  expect(sections.map(value=>value.dataset.formRole)).toEqual(['statement','grounds']);
  expect(view.textContent).not.toContain('Форма не предоставлена');expect(view.textContent).not.toContain('tos.form.');
  const grounds=sections.find(value=>value.dataset.formRole==='grounds');
  one(grounds,value=>value.className==='sc-form-inspect').dispatchEvent({type:'click'});
  expect(grounds.dataset.exactFormInspected).toBe('true');
  expect(one(grounds,value=>value.dataset.readingAnchor==='form:grounds:wording')).toBeTruthy();
});

test('seeded role availability keeps statements qualified and machine details out of the reading tree',()=>{
  dom();let seed=915236;const next=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;};
  for(let run=0;run<120;run++){
    const raw=formNode();
    for(const role of ['name','caption','hover','grounds','history','technical'])if(next()%3){
      raw.human_form_selection.roles[role]={state:next()%2?'missing':'unavailable',reason:'no-ready-form',form:null,packet:null};
    }
    const before=structuredClone(raw),view=renderHumanForms(raw);
    expect(view.textContent).toContain(raw.human_form_selection.roles.statement.packet.display_text);
    expect(view.textContent).toContain('НЕ доказано');
    expect(view.textContent).not.toMatch(/tos\.form\.|sha256:|schema_version|семантического принятия/);
    expect(descendants(view).some(value=>value.tagName==='pre')).toBe(false);
    expect(raw).toEqual(before);
  }
});


test.each([['missing','Текст пока не предоставлен.'],['unavailable','Текст недоступен.'],['ambiguous','Вариант текста не определён.']])('an empty primary selection exposes its state: %s',(state,message)=>{
  dom();const raw=formNode();
  for(const role of Object.keys(raw.human_form_selection.roles))raw.human_form_selection.roles[role]={state:'missing',reason:'no-ready-form',form:null,packet:null};
  raw.human_form_selection.roles.statement={state,reason:'no-ready-form',form:null,packet:null};
  expect(renderHumanForms(raw).textContent).toContain(message);
});
