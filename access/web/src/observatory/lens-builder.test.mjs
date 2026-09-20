import {afterEach,test,expect} from 'vitest';
import {lensBuilderAreaBudget,normalizeLensBuilderArea,prepareLensBuilderDraft,lensBuilderCanApply,lensBuilderFocusLabel,LENS_BUILDER_LIMITS} from './lens-builder.mjs';
import {setUiLanguage} from './ui-i18n.mjs';

const revision='a'.repeat(64);
const boundary={is_source:false,is_canon:false,writes_to_tree:false};
const catalog={capabilities:{sources:['philosophy','canon']}};
const context={catalog};
const node=id=>({id});
const relation=id=>({id,from_id:'n0',to_id:'n1'});
const area=(nodes,relations=[],selection={kind:'node',id:nodes[0]?.id})=>({packet:{schema:'tos_browser_exploration_view_v1',source_revision:revision,nodes,relations,selection,authority_boundary:boundary},selection});

afterEach(()=>setUiLanguage('ru'));

test('area budget reports the original packet size without slicing it',()=>{
  const value=area(Array.from({length:41},(_,i)=>node('n'+i)),Array.from({length:81},(_,i)=>relation('r'+i)));
  expect(lensBuilderAreaBudget(value)).toMatchObject({nodes:41,relations:81,nodeOverflow:1,relationOverflow:1,overBudget:true});
  expect(value.packet.nodes).toHaveLength(41);expect(value.packet.relations).toHaveLength(81);
});

test('oversized retained area offers focus/all and keeps source node IDs in packet',()=>{
  const value=area(Array.from({length:41},(_,i)=>node('n'+i)),[],{kind:'node',id:'n7'});
  const before=value.packet.nodes.map(item=>item.id),prepared=prepareLensBuilderDraft({packet:value.packet,context,selection:value.selection});
  expect(prepared.areaChoice).toMatchObject({required:true,nodeCount:41,defaultScope:'focus',choices:['focus','all']});
  expect(prepared.draft.scope).toBe('focus');expect(prepared.draft.focusId).toBe('n7');expect(prepared.draft.nodeIds).toEqual([]);
  expect(value.packet.nodes.map(item=>item.id)).toEqual(before);
});

test('oversized imported draft remains an explicit invalid state',()=>{
  const value=area([node('n0')]),draft={v:2,name:'Imported',scope:'area',sources:['philosophy'],nodeIds:Array.from({length:41},(_,i)=>'x'+i),focusId:null,query:'',kinds:[],predicates:[],depth:0,direction:'either',profile:'all',limit:40,relations:true,conditions:{nodes:[],relations:[]},paths:[]};
  const prepared=prepareLensBuilderDraft({packet:value.packet,context,selection:value.selection,draft});
  expect(prepared.areaChoice).toMatchObject({required:true,invalid:true,sourceNodeIds:41});
  expect(prepared.draft.nodeIds).toHaveLength(41);
});

test('a bounded imported draft still asks for an explicit scope on a large area',()=>{
  const value=area(Array.from({length:41},(_,i)=>node('n'+i)),[],{kind:'node',id:'n2'});
  const draft={v:2,name:'Imported',scope:'area',sources:['philosophy'],nodeIds:['n0'],focusId:null,query:'',kinds:[],predicates:[],depth:0,direction:'either',profile:'all',limit:40,relations:true,conditions:{nodes:[],relations:[]},paths:[]};
  const prepared=prepareLensBuilderDraft({packet:value.packet,context,selection:value.selection,draft});
  expect(prepared.areaChoice).toMatchObject({required:true,defaultScope:'focus'});
  expect(prepared.areaChoice.invalid).toBeUndefined();
  expect(prepared.draft.nodeIds).toEqual(['n0']);
});

test('an empty opening keeps the area absent while preparing an all-tree draft',()=>{
  const prepared=prepareLensBuilderDraft({packet:null,context});
  expect(prepared.areaChoice).toBeNull();
  expect(prepared.sourceNodeCount).toBe(0);
  expect(prepared.draft.scope).toBe('all');
  expect(prepared.draft.nodeIds).toEqual([]);
  const saved={...prepared.draft,scope:'focus',focusId:'saved:node'};
  const restored=prepareLensBuilderDraft({packet:null,context,draft:saved});
  expect(restored.draft).toEqual(saved);
});

test('focus scope resolves a saved center from the bounded packet in the active locale',()=>{
  const value=area([{id:'n0',display:{title:{ru:'Русский центр',en:'English center',es:'Centro español'}}}],[],{kind:'node',id:'n0'});
  const saved={v:2,name:'Saved',scope:'focus',sources:['philosophy'],nodeIds:[],focusId:'n0',query:'',kinds:[],predicates:[],depth:0,direction:'either',profile:'all',limit:40,relations:true,conditions:{nodes:[],relations:[]},paths:[]};
  const before=structuredClone(saved),restored=prepareLensBuilderDraft({packet:null,context,draft:saved}).draft;
  expect(restored).toEqual(saved);
  for(const [locale,label] of [['ru','Русский центр'],['en','English center'],['es','Centro español']]){
    setUiLanguage(locale);expect(String(lensBuilderFocusLabel(value,restored))).toBe(label);
  }
  setUiLanguage('en');expect(String(lensBuilderFocusLabel(null,restored))).toBe('Star unavailable');
  expect(saved).toEqual(before);
});

test('area carrier accepts lens and exploration schemas only with an exact selection',()=>{
  const value=area([node('n0')]);expect(normalizeLensBuilderArea(value).packet).toBe(value.packet);
  expect(()=>normalizeLensBuilderArea({packet:{...value.packet,schema:'future'},selection:value.selection})).toThrow();
  expect(()=>normalizeLensBuilderArea({packet:value.packet,selection:{kind:'node',id:'missing'}})).toThrow();
  expect(()=>normalizeLensBuilderArea({packet:value.packet,selection:{kind:'node',id:'n0',source_revision:revision}})).toThrow();
});

test('apply seam requires a preview for the current draft and source revision',()=>{
  const preview={schema:'tos_lens_result_v1',source_revision:revision,nodes:[node('n0')]};
  expect(lensBuilderCanApply({preview,previewRevision:2,draftRevision:2,areaRevision:revision,sourceRevision:revision})).toBe(true);
  for(const change of [{previewRevision:1},{draftRevision:3},{areaRevision:'b'.repeat(64)},{preview:{schema:'tos_lens_result_v1',source_revision:'b'.repeat(64),nodes:[node('n0')]}},{preview:null},{preview:{schema:'tos_lens_result_v1',source_revision:revision,nodes:[]}}])
    expect(lensBuilderCanApply({preview,previewRevision:2,draftRevision:2,areaRevision:revision,sourceRevision:revision,...change})).toBe(false);
  expect(lensBuilderCanApply({preview,previewRevision:2,draftRevision:2,areaRevision:null,sourceRevision:revision})).toBe(true);
  expect(LENS_BUILDER_LIMITS).toEqual({nodes:40,relations:80,catalogPage:40});
});
