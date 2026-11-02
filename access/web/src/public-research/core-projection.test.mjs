import {readFileSync} from 'node:fs';
import {expect,test,vi} from 'vitest';
import {attachCore,coreContext} from './core-projection.mjs';
import {createCoreView} from './core-view.mjs';
import {pathThrough} from './space-play.mjs';
import {planeOfPlaces} from './space-geography.mjs';
import {citationId,citedPlaces,createResearchIndex} from './research-index.mjs';
import '../observatory/human-forms-wasm-test-runtime.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {createAtlasLibrary} from '../../constructor/atlas-data.mjs';

const read=name=>JSON.parse(readFileSync(new URL(`../../../../bonsai/zarathustra/core/${name}.json`,import.meta.url),'utf8'));
const core=Object.fromEntries(['concepts','figures','links','chains','proposed'].map(name=>[name,read(name)]));
const make=()=>attachCore({nodes:[{id:'work',kind:'work'},{id:'demo:affirmation',kind:'concept'}],atlas:{defaultIds:['work','demo:affirmation'],edges:[]},space:{routes:[]}},core);

test('research citation addresses preserve works, ranges and unambiguous old links',()=>{
  const whole={w:'also-sprach-zarathustra',c:'I.02',u:['unit-1','unit-1']};
  const quoted={...whole,q:'a source quotation'},first={...whole,s:4,e:12},second={...whole,s:13,e:20};
  const otherWork={...first,w:'another-work'},otherVersion={...first,versionId:'another-witness'};
  const located={w:'ethica',loc:'I.D7'};
  const ambiguousA={w:'another-work',c:'I.03',u:['unit-2','unit-2'],s:1,e:3};
  const ambiguousB={...ambiguousA,s:4,e:7};
  const unambiguous={w:'another-work',c:'I.04',u:['unit-3','unit-3'],s:1,e:3};
  const foreignUnits={w:'sophist',u:['unit-4','unit-4'],s:2,e:8};
  const library={nodes:[
    {id:'a',kind:'concept',title:{en:'A'},body:{cite:[whole,quoted,first,otherWork,otherVersion,located,ambiguousA,unambiguous,foreignUnits]}},
    {id:'b',kind:'concept',title:{en:'B'},body:{cite:[second,ambiguousB]}},
  ],space:{}};
  const index=createResearchIndex(library),legacy='tos.bonsai.passage.dta.I.02.unit-1..unit-1';
  expect(citationId(whole)).toBe(legacy);
  expect(citationId(quoted)).toBe(legacy);
  expect(new Set([whole,first,second,otherWork,otherVersion].map(citationId)).size).toBe(5);
  expect(citedPlaces(library.nodes[0].body)).toHaveLength(9);
  expect(index.get(legacy).record).toEqual(whole);
  expect(index.get(legacy).citations).toEqual([whole,quoted]);
  expect(index.get(citationId(first)).record).toEqual(first);
  expect(index.get(citationId(otherWork)).record).toEqual(otherWork);
  expect(index.get(citationId(otherVersion)).record).toEqual(otherVersion);
  expect(index.references(citationId(first)).map(item=>item.id)).toEqual(['a']);
  expect(index.references(citationId(second)).map(item=>item.id)).toEqual(['b']);
  expect(index.get('tos.bonsai.passage.dta.I.03.unit-2..unit-2')).toBeNull();
  expect(index.get('tos.bonsai.passage.dta.I.04.unit-3..unit-3').record).toEqual(unambiguous);
  expect(index.get(citationId(foreignUnits)).record).toEqual(foreignUnits);
  expect(index.get(citationId(located))).toMatchObject({kind:'bibliographic-pointer',resolution:'unresolved-bibliographic',record:located});
});

test('the actual Rust constructor accepts core materials while its default saved graph stays unchanged',async()=>{
  const nodes=['work','chapter-p3.r2','moment','chapter-p3.r13','all-things','same-life'].map(id=>({id,kind:id==='work'?'work':id.startsWith('chapter')?'chapter':'fragment',parentId:id==='work'?null:'work',title:{ru:id,en:id}}));
  const library=await createAtlasLibrary({schema:'tos_constructor_library_v1',rootId:'work',fingerprint:'0'.repeat(64),nodes});
  library.space={routes:library.atlas.routes};
  const original=createConstructorModel(library,{storage:null}),before=original.getState();original.dispose();
  attachCore(library,core);
  const model=createConstructorModel(library,{storage:null});
  expect(model.getState()).toEqual(before);
  model.addMaterial(library.space.core.materialIds.disciples);
  expect(model.getState().nodes.some(node=>node.materialId===library.space.core.materialIds.disciples)).toBe(true);
  model.dispose();
});

test('core preserves atlas identity and defaults, projects only admitted links and resolves every chain endpoint',()=>{
  const library=make(),projection=library.space.core;
  expect(library.atlas).toEqual({defaultIds:['work','demo:affirmation'],edges:[]});
  expect(projection.materialIds.affirmation).toBe('demo:affirmation');
  expect(new Set(library.nodes.map(node=>node.id)).size).toBe(library.nodes.length);
  expect(projection.edges.filter(edge=>edge.id.startsWith('core-link:'))).toHaveLength(397);
  const ids=new Set(library.nodes.map(node=>node.id));
  for(const edge of projection.edges){expect(ids.has(edge.from),edge.id).toBe(true);expect(ids.has(edge.to),edge.id).toBe(true);}
  expect(library.space.routes).toHaveLength(core.chains.filter(chain=>chain.admission==='map'&&chain.steps.length).length);
  for(const route of library.space.routes){
    const source=core.chains.find(chain=>chain.id===route.coreChainId);
    if(source.structure==='comparison-branching'){
      expect(new Set(route.steps.map(step=>step.nodeId))).toEqual(new Set(source.steps.flatMap(step=>[projection.materialIds[step.from],projection.materialIds[step.to]])));
      expect(route.steps.every(step=>step.body.ru&&step.body.en)).toBe(true);
    }else expect(route.steps).toHaveLength(source.steps.length+1);
    expect(route.transitions).toHaveLength(source.steps.length);
    expect(route.transitions.map(turn=>[turn.fromNodeId,turn.toNodeId])).toEqual(source.steps.map(step=>[projection.materialIds[step.from],projection.materialIds[step.to]]));
    expect(route.steps.every(step=>ids.has(step.nodeId))).toBe(true);
    expect(route.transitions.map(turn=>turn.body.cite[0])).toEqual(source.steps.map(step=>step.cite));
  }
});

test('a chain uses only its reviewed transitions and context does not mutate stored graphs or add rejected links',()=>{
  const library=make(),projection=library.space.core,before=JSON.stringify(library);
  const chain=coreContext(projection,{chainId:'disciples-freedom'});
  expect(chain.materialIds).toHaveLength(5);expect(chain.edges).toHaveLength(4);
  const node=coreContext(projection,{materialId:projection.materialIds.affirmation});
  expect(node.edges.every(edge=>edge.coreRecord.admission==='map')).toBe(true);
  expect(coreContext(projection,{})).toEqual({materialIds:[],edges:[]});
  expect(JSON.stringify(library)).toBe(before);
});

test('a disconnected third stop remains on a route without an invented edge',()=>{
  expect(pathThrough(['a','b','c','d'],{},(_,from,to)=>from==='a'?{nodeIds:[from,to],edgeIds:['ab']}:from==='c'?{nodeIds:[from,to],edgeIds:['cd']}:null)).toEqual({nodeIds:['a','b','c','d'],edgeIds:['ab','cd'],restrictEdges:true});
});

test('a place label exposes exactly the materials counted there',()=>{
  const map=planeOfPlaces({stars:['a','b','unknown'],placeOf:id=>id==='unknown'?null:{place:'athens'},places:{athens:{point:[23.72,37.98],label:{en:'Athens',ru:'Афины'}}}});
  expect(map.members('athens')).toEqual(['a','b']);expect(map.members('missing')).toEqual([]);
  expect(map.labels('en').find(label=>label.ref==='athens').title).toBe('Athens · 2');
});

test('link paths show their actual endpoints, verbs and cited places for both recorded shapes',()=>{
  class Element{
    constructor(tag){this.tag=tag;this.children=[];this.listeners={};this.className='';this.textContent='';}
    append(...children){this.children.push(...children);}
    addEventListener(type,handler){this.listeners[type]=handler;}
  }
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});
  try{
    const projection=make().space.core,onNode=vi.fn(),fragment=vi.fn(cite=>Object.assign(new Element('cite'),{cite}));
    const view=createCoreView({core:projection,locale:()=> 'en',fragments:{fragment,render:vi.fn()},onNode,onRoute:vi.fn()});
    for(const id of ['attention:link:02:gateway','animals--zarathustra--1']){
      const record=core.links.find(link=>link.id===id),root=new Element('root');view.link(root,record);
      const path=root.children[0].children.find(child=>child.className==='ps-core-path');
      const rows=path.children.slice(1);
      expect(rows).toHaveLength(record.path.length);
      expect(rows.flatMap(row=>row.children).some(child=>child.textContent==='[object Object]')).toBe(false);
      rows.forEach((row,index)=>{
        const step=record.path[index],ids=step.id?[step.id]:[step.from,step.to];
        const buttons=row.children.filter(child=>child.tag==='button');
        expect(buttons).toHaveLength(ids.length);
        buttons.forEach((item,i)=>{item.listeners.click();expect(onNode).toHaveBeenLastCalledWith(projection.materialIds[ids[i]]);});
        if(step.verb)expect(row.children.some(child=>child.textContent===step.verb.en)).toBe(true);
        for(const cite of Array.isArray(step.cite)?step.cite:[step.cite])expect(fragment).toHaveBeenCalledWith(cite);
      });
    }
  }finally{vi.unstubAllGlobals();}
});

test('relation actions and reference view retain the precise record and source',()=>{
  class Element{
    constructor(tag){this.tag=tag;this.children=[];this.listeners={};this.className='';this.textContent='';}
    append(...children){this.children.push(...children);}
    addEventListener(type,handler){this.listeners[type]=handler;}
    setAttribute(){}
  }
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});
  try{
    const projection=make().space.core,onAction=vi.fn(),fragments={fragment:cite=>Object.assign(new Element('cite'),{cite}),render:vi.fn()};
    const view=createCoreView({core:projection,locale:()=> 'en',fragments,onAction});
    const record=projection.links.find(item=>item.id==='affirmation:link:01:child'),root=new Element('root');
    expect(view.renderReference(root,{kind:'relation',id:record.id})).toBe(true);
    const link=root.children[0].children[0],actions=link.children.find(child=>child.className==='ps-kernel-actions');
    actions.children[0].listeners.click();
    expect(onAction).toHaveBeenCalledWith({action:'read',recordId:record.id,kind:'relation',record,cite:record.cite??record.grounds[0]});
    expect(view.renderReference(new Element('root'),{kind:'relation',id:'missing'})).toBe(false);
    const chain=projection.chains.find(item=>item.steps.length),stepRoot=new Element('root');
    expect(view.renderReference(stepRoot,{kind:'relation',id:`core-chain:${chain.id}:0`})).toBe(true);
  }finally{vi.unstubAllGlobals();}
});
