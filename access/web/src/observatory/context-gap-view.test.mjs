import {afterEach,expect,test,vi} from 'vitest';
import {renderEssentialContext,renderClaimContext} from './human-forms-view.mjs';

// Minimal DOM model for the disclosure contract, not browser layout proof.
class Element {
  static TEXT_NODE=3;
  constructor(tag,nodeType=1){this.tagName=tag;this.nodeType=nodeType;this.children=[];this.dataset={};this.open=false;this._text='';}
  set textContent(value){this._text=value;this.children=[];}
  get textContent(){return this._text+this.children.map(value=>value.textContent).join('');}
  append(...values){this.children.push(...values);}
  get lastChild(){return this.children.at(-1);}
}
function dom(){
  vi.stubGlobal('Node',Element);
  vi.stubGlobal('document',{createElement:tag=>new Element(tag),createTextNode:text=>{
    const node=new Element('#text',3);node.textContent=text;return node;
  }});
}
afterEach(()=>vi.unstubAllGlobals());
const flatten=node=>[node,...node.children.flatMap(flatten)];
const raw={fields:{negation:{value:false,source_pointer:'/negation'}},conflicts:['disputed'],unknown:null};
const gap={state:'requires-exact-context',contexts:[]};

test.each(['requires-exact-context','complete'])('missing record presentation stays explicit and exact data stays closed: %s',state=>{
  dom();const before=structuredClone(raw);
  const node=renderEssentialContext({state:'available',items:[{state:'available',pointer:'/record',value:raw}]},{...gap,state});
  const all=flatten(node),notice=all.find(value=>value.dataset.contextPresentation);
  expect(notice.dataset.contextPresentation).toBe(state==='complete'?'not-included':state);
  expect(notice.textContent).toContain('перед выводами');
  const disclosure=all.find(value=>value.tagName==='details'&&value.textContent.includes('disputed'));
  expect(disclosure.open).toBe(false);
  expect(JSON.parse(flatten(disclosure).find(value=>value.tagName==='pre').textContent)).toEqual(raw);
  expect(raw).toEqual(before);
});

test('Claim fallback preserves conflict, negation and unknown in exact disclosure without rendering a technical field wall',()=>{
  dom();const node=renderClaimContext({semantics:{assertion_contexts:[raw]},relations:[]},gap),all=flatten(node);
  expect(all.some(value=>value.dataset.contextPresentation==='requires-exact-context')).toBe(true);
  expect(all.some(value=>value.tagName==='dl')).toBe(false);
  const details=all.filter(value=>value.tagName==='details');
  expect(details.every(value=>!value.open)).toBe(true);
  expect(details.some(value=>flatten(value).some(child=>child.tagName==='pre'&&JSON.stringify(JSON.parse(child.textContent))===JSON.stringify(raw)))).toBe(true);
});
