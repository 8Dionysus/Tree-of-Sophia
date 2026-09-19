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

test('places statement first and keeps every declared role behind an accessible disclosure',()=>{
  dom();const raw=formNode(),before=structuredClone(raw),view=renderHumanForms(raw);
  expect(roleSections(view).map(value=>value.dataset.formRole)).toEqual(['statement','name','caption','hover','grounds','history','technical']);
  const statement=roleSections(view)[0];expect(statement.parentNode).toBe(view);
  expect(one(statement,value=>value.dataset.readingAnchor==='form:statement:wording')).toBeTruthy();
  expect(descendants(statement).some(value=>value.textContent.includes('НЕ доказано'))).toBe(true);
  expect(detailsFor(view).filter(value=>value.dataset.formGroup).map(value=>value.dataset.formGroup)).toEqual(['additional','context','exact']);
  expect(detailsFor(view).filter(value=>value.dataset.formGroup).every(value=>value.open===false)).toBe(true);
  expect(raw).toEqual(before);
});

test('keeps mandatory form context beside wording when readable context falls back',()=>{
  dom();const raw=formNode(),view=renderHumanForms(raw,{readableContext:{state:'requires-exact-context',contexts:[]}});
  const statement=one(view,value=>value.dataset.formRole==='statement');
  const all=descendants(statement),gap=all.find(value=>value.dataset.contextPresentation);
  expect(gap?.dataset.contextPresentation).toBe('requires-exact-context');
  expect(all.some(value=>value.dataset.contextSlot==='qualification')).toBe(true);
  expect(statement.textContent.indexOf('Текст формы statement')).toBeLessThan(statement.textContent.indexOf('НЕ доказано'));
  expect(all.some(value=>value.className==='sc-context-fallback')).toBe(false);
  expect(all.some(value=>value.className==='sc-context-exact'&&value.open===false)).toBe(true);
});

test('retains role states and exact diagnostics inside the matching disclosures',()=>{
  dom();const raw=formNode(),selection=raw.human_form_selection;
  selection.roles.name={state:'missing',reason:'no-ready-form',form:null,packet:null};
  selection.candidates.find(value=>value.role==='name').state='stale';
  selection.roles.caption={state:'ambiguous',reason:'multiple-forms',form:null,packet:null};
  selection.roles.hover={state:'unavailable',reason:'no-ready-form',form:null,packet:null};
  selection.candidates.find(value=>value.role==='hover').state='restricted';
  selection.roles.history={state:'unavailable',reason:'no-ready-form',form:null,packet:null};
  selection.candidates.find(value=>value.role==='history').state='needs-assessment';
  selection.roles.grounds.state='over-budget';selection.roles.grounds.reason='inspect-exact-form';selection.roles.grounds.packet=null;
  selection.candidates.push({form:ref('tos.form.fixture.unassigned','e'),role:null,language:null,state:'invalid',source_pointer:'/attributes/human_forms/7'});
  const view=renderHumanForms(raw),sections=roleSections(view),all=descendants(view);
  expect(sections).toHaveLength(7);
  expect(sections.find(value=>value.dataset.formRole==='name').textContent).toContain('Форма не предоставлена');
  expect(sections.find(value=>value.dataset.formRole==='caption').textContent).toContain('Есть несколько форм');
  expect(all.some(value=>value.textContent.includes('Доступ ограничен'))).toBe(true);
  expect(all.some(value=>value.textContent.includes('Требуется оценка'))).toBe(true);
  const exact=all.find(value=>value.dataset.formGroup==='exact');
  expect(exact.children.some(value=>value.dataset.formRole==='technical')).toBe(true);
  const diagnostic=one(exact,value=>value.dataset.candidateState==='invalid');
  expect(diagnostic?.open).toBe(false);
  expect(one(diagnostic,value=>value.dataset.readingAnchor==='unassigned-form')).toBeTruthy();
  const grounds=sections.find(value=>value.dataset.formRole==='grounds');
  const inspect=one(grounds,value=>value.className==='sc-form-inspect');expect(inspect).toBeTruthy();
  inspect.dispatchEvent({type:'click'});
  expect(grounds.dataset.exactFormInspected).toBe('true');
  expect(one(grounds,value=>value.dataset.readingAnchor==='form:grounds:exact:wording')).toBeTruthy();
});
