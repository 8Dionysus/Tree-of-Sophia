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

test.each(['requires-exact-context','complete'])('fallback presents known facts without a raw packet: %s',state=>{
  dom();const before=structuredClone(raw);
  const node=renderEssentialContext({state:'available',items:[{state:'available',pointer:'/record',value:raw}]},{...gap,state});
  const all=flatten(node);
  expect(node.textContent).toContain('Отрицание');expect(node.textContent).toContain('Нет');
  expect(node.textContent).toContain('Оспаривается');expect(all.some(value=>value.tagName==='pre')).toBe(false);
  if(state==='requires-exact-context')expect(node.textContent).toContain('Часть контекста доступна в источнике.');
  expect(raw).toEqual(before);
});

test('Claim fallback preserves negation and dispute as human facts',()=>{
  dom();const before=structuredClone(raw),node=renderClaimContext({semantics:{assertion_contexts:[raw]},relations:[]},gap),all=flatten(node);
  expect(node.textContent).toContain('Отрицание');expect(node.textContent).toContain('Оспаривается');
  expect(all.some(value=>value.dataset.contextPresentation==='requires-exact-context')).toBe(true);
  expect(all.some(value=>value.tagName==='pre')).toBe(false);expect(raw).toEqual(before);
});

test('structured descriptions retain their substance instead of only displaying language metadata',()=>{
  dom();const value={semantic_scope:{scope_note:'Историческая языковая традиция.',identity_criterion:'Разновидности рассматриваются отдельно.',language:'ru',script:'Cyrl'},
    semantic_content:{system_account:'A supplied source description.',language:'en',script:'Latn'}};
  const before=structuredClone(value),node=renderEssentialContext({state:'available',items:[{state:'available',pointer:'/record',value}]});
  for(const wording of ['Историческая языковая традиция.','Разновидности рассматриваются отдельно.','A supplied source description.'])expect(node.textContent).toContain(wording);
  expect(node.textContent).not.toContain('system_account');expect(value).toEqual(before);
});
