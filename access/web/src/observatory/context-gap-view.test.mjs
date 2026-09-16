import {afterEach,expect,test,vi} from 'vitest';
import {renderEssentialContext,renderClaimContext} from './human-forms-view.mjs';
import {renderReadableContexts} from './readable-context-view.mjs';

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

test('seeded classified claim contexts render readable facts without treating reference categories as prose',()=>{
  dom();let seed=915715;
  for(let index=0;index<80;index++){
    seed=(Math.imul(seed,1664525)+1013904223)>>>0;
    const opaque='tos.claim.'+seed,quote='Leipzig / Druck und Verlag von C. G. Naumann. / 1886.';
    const fields={claim_id:opaque,subject_ref:'tos.edition.'+seed,predicate:'provision_activity',claim_type:'bibliographic',value:opaque,
      evidence_refs:['ToS/sources/'+seed+'.json',opaque],provenance_event_ref:'tos.event.'+seed,
      object:{provision_kind:'manufacture',transcribed_statement:quote,places:[{literal_form:'Leipzig',normalized_place_ref:'tos.place.leipzig'}],
        agents:[{role:'printer',literal_form:'C. G. Naumann',normalized_agent_ref:'tos.agent.naumann'}],temporal:{kind:'date',value:'1886',precision:'year'},
        activity_warning:'The printer identity remains provisional.'},[index%2?'negative':'negation']:false,confidence:0.5};
    const entries=Object.entries(fields).map(([key,value])=>({key,category:['semantic','provenance','identity'][seed%3],label:{ru:key},
      display:{type:Array.isArray(value)?'array':typeof value,text:typeof value==='string'?value:JSON.stringify(value)}}));
    const contexts=[{entries}],before=structuredClone(contexts);
    const node=renderReadableContexts(contexts,'claim');
    for(const value of [quote,'Leipzig','C. G. Naumann','1886','Печатник','Изготовление','Библиографическое','The printer identity remains provisional.','Нет'])expect(node.textContent).toContain(value);
    for(const value of ['tos.','ToS/','provision_activity','bibliographic','normalized_place_ref'])expect(node.textContent).not.toContain(value);
    expect(flatten(node).some(value=>value.tagName==='pre')).toBe(false);expect(contexts).toEqual(before);
    const named=renderReadableContexts(contexts,'claim',{resolveValue:entry=>entry.key==='subject_ref'?'Издание 1886 года':entry.key==='predicate'?'Выходные сведения':undefined});
    expect(named.textContent).toContain('Издание 1886 года');expect(named.textContent).toContain('Выходные сведения');expect(named.textContent).not.toContain(opaque);
  }
});
