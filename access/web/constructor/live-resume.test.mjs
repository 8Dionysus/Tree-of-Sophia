import {test,expect} from 'vitest';
import {makeLiveResume,validateLiveResume,resolveLiveResumeSelection} from './live-resume.mjs';
import {StableExplorationLayout} from './live-model.mjs';
import {ExplorationSceneCache} from '../src/observatory/exploration-cache.mjs';
import {pageFixture} from '../src/observatory/exploration-test-fixtures.mjs';
import {compactFormLens} from '../fixtures/human-form-data.mjs';
import {claimMaterialReference} from '../src/observatory/knowledge-client.mjs';
import {createLiveResearch} from './live-research.mjs';
import {knowledgeScene} from '../shared/knowledge-scene.ts';

test('resume preserves a bounded query and geometry, without source packets or cursors',()=>{
  const cache=new ExplorationSceneCache(),first=pageFixture();cache.accept(cache.begin(first.query),first);const view=cache.snapshot();
  const presentation={layout:new StableExplorationLayout().capture(),pose:{v:1,yaw:0,pitch:0,zoom:1,pan:[3,4],center:[0,0,0],fit:1},mode:'compact'};
  const saved=makeLiveResume({view,selection:view.selection,areaKind:'exploration'},presentation);
  expect(saved.area.type).toBe('route');expect(saved.area.target.origin.id).toBe(first.origin.id);
  expect(saved.presentation.pose.pan).toEqual([3,4]);expect(JSON.stringify(saved)).not.toContain('next_cursor');
  expect(JSON.stringify(saved)).not.toContain('source_refs');expect(validateLiveResume(saved)).toEqual(saved);
  expect(()=>validateLiveResume({...saved,cursor:'opaque'})).toThrow();
  expect(()=>validateLiveResume({...saved,sourceRevision:'b'.repeat(64)})).toThrow();
  expect(()=>validateLiveResume({...saved,presentation:{...presentation,pose:{...presentation.pose,zoom:Infinity}}})).toThrow();
  expect(()=>validateLiveResume({...saved,area:{type:'route',target:{...saved.area.target,options:{...saved.area.target.options,cursor:'opaque'}}}})).toThrow();
});

function claimResume(){
  const view=compactFormLens();
  // Match the graph fixture route: the independent endpoint has its own
  // declared identity rather than the generic reading fixture's shared one.
  view.nodes.find(item=>item.id==='fixture:subject').entity_id='tos.fixture.endpoint.subject';
  view.scene=knowledgeScene(view.nodes,view.relations,view.focus.node_id,null);
  const path=view.scene.compact.claim_paths[0],raw=view.nodes.find(item=>item.id===path.claim_node_id);
  const query=pageFixture().query;query.origin={kind:'node',id:raw.id,content_revision:raw.content_revision};
  view.contexts=[{query}];
  const selection={kind:'claim-path',id:path.id,claimId:path.claim_node_id};
  const presentation={layout:new StableExplorationLayout().capture(),pose:{v:1,yaw:0,pitch:0,zoom:1,pan:[0,0],center:[0,0,0],fit:1},mode:'compact'};
  return {view,path,selection,saved:makeLiveResume({view,selection,areaKind:'exploration'},presentation)};
}

test('resume roundtrips a Claim path and its full bounded binding instead of selecting its node',()=>{
  const {view,path,selection,saved}=claimResume();
  const restored=validateLiveResume(JSON.parse(JSON.stringify(saved)));
  expect(restored.selection.claimReference).toEqual(claimMaterialReference(view,path));
  expect(resolveLiveResumeSelection(restored.selection,view)).toEqual(selection);
  expect(JSON.stringify(restored)).not.toContain('display_text');
  expect(JSON.stringify(restored)).not.toContain('human_form_selection');
});

test('a missing, changed or differently bound Claim path never falls back to the Claim node',()=>{
  const {view,saved}=claimResume();
  const missing=structuredClone(view);missing.scene.compact.claim_paths=[];
  expect(resolveLiveResumeSelection(saved.selection,missing)).toBeNull();
  const changed=structuredClone(view);changed.nodes.find(item=>item.id===saved.selection.id).content_revision='f'.repeat(64);
  expect(resolveLiveResumeSelection(saved.selection,changed)).toBeNull();
  expect(resolveLiveResumeSelection(saved.selection,{...view,source_revision:'e'.repeat(64)})).toBeNull();
  const different=structuredClone(saved.selection);different.claimReference.pathId+=':other';
  expect(resolveLiveResumeSelection(different,view)).toBeNull();
  const reordered=structuredClone(saved.selection);reordered.claimReference.nodeIds.reverse();
  expect(resolveLiveResumeSelection(reordered,view)).toBeNull();
});

test('existing plain material resumes retain their kind and require the exact revision',()=>{
  const {view,saved}=claimResume();delete saved.selection.claimReference;
  expect(resolveLiveResumeSelection(saved.selection,view)).toEqual({kind:'node',id:saved.selection.id});
  const missing=structuredClone(view);missing.nodes=missing.nodes.filter(item=>item.id!==saved.selection.id);
  expect(resolveLiveResumeSelection(saved.selection,missing)).toBeNull();
});

test('the restored selection highlights the Claim edge and reads its compound source binding',async()=>{
  const {saved,selection,view:fresh}=claimResume(),path=fresh.scene.compact.claim_paths[0],edges=[],requests=[];
  delete fresh.contexts;
  const session={cancelScene(){},cancelInspect(){},dispose(){},client:{
    async readClaimReference(reference){requests.push(reference);return {packet:fresh,path,match:fresh.nodes.find(item=>item.id===path.claim_node_id),endpoints:[]};},
  }};
  const sky={update(){},select(){},selectEdge(id){edges.push(id);},frame(){},dispose(){}};
  const controller=createLiveResearch({session,sky});controller.showLens(fresh);
  controller.selectRaw(resolveLiveResumeSelection(saved.selection,controller.state().view));
  await controller.read();
  expect(controller.state().selection).toEqual(selection);expect(edges.at(-1)).toBe(path.id);
  expect(requests.at(-1)).toEqual(saved.selection.claimReference);
  expect(controller.state().reading.claimReference).toEqual(saved.selection.claimReference);
  expect(controller.state().reading.claimReading).toBeTruthy();
  controller.dispose();
});
