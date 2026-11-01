import {readFileSync} from 'node:fs';
import {expect,test} from 'vitest';
import {attachCore,coreContext} from './core-projection.mjs';
import {pathThrough} from './space-play.mjs';
import {planeOfPlaces} from './space-geography.mjs';
import '../observatory/human-forms-wasm-test-runtime.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {createAtlasLibrary} from '../../constructor/atlas-data.mjs';

const read=name=>JSON.parse(readFileSync(new URL(`../../../../bonsai/zarathustra/core/${name}.json`,import.meta.url),'utf8'));
const core=Object.fromEntries(['concepts','figures','links','chains','proposed'].map(name=>[name,read(name)]));
const make=()=>attachCore({nodes:[{id:'work',kind:'work'},{id:'demo:affirmation',kind:'concept'}],atlas:{defaultIds:['work','demo:affirmation'],edges:[]},space:{routes:[]}},core);

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
  expect(library.space.routes).toHaveLength(97);
  for(const route of library.space.routes){
    const source=core.chains.find(chain=>chain.id===route.coreChainId);
    expect(route.steps).toHaveLength(source.steps.length+1);
    expect(route.transitions).toHaveLength(source.steps.length);
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
