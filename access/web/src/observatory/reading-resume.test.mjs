import {test,expect} from 'vitest';
import {emptyReading,validateReading,readReading} from './reading-resume.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {readingKey} from './reader-model.mjs';
import {claimMaterialReference} from './knowledge-client.mjs';
import {compactFormLens} from '../../fixtures/human-form-data.mjs';

const sourceRevision='a'.repeat(64),contentRevision='b'.repeat(64);
function entry(id='opaque:one',kind='node'){
  const key=JSON.stringify([readingKey(kind,id),sourceRevision,contentRevision,'original']);
  return {kind,id,sourceRevision,contentRevision,preferred:'original',positions:[[key,{top:430,details:[['sources',true]],anchor:{key:'description:6',offset:-12}}]]};
}
const state=()=>({v:1,activeKey:readingKey('relation','opaque:two'),entries:[entry(),entry('opaque:two','relation')]});
test('paired reading roundtrips exact references, language and bounded anchors without source copies',()=>{
  const value=state();value.entries[0].snapshot={text:'Never persist delivered wording'};value.entries[0].positions[0][1].anchor.text='Never persist paragraph text';
  const restored=validateReading(JSON.parse(JSON.stringify(value)));expect(restored).toEqual(state());expect(JSON.stringify(restored)).not.toContain('Never persist');
  expect(readReading(null)).toEqual(emptyReading());
});
test('compound reading persists bounded closure selectors without source packets and rejects a different or incomplete identity',()=>{
  const packet=compactFormLens(),path=packet.scene.compact.claim_paths[0],reference=claimMaterialReference(packet,path);
  const item={...entry(path.claim_node_id),claimReference:reference};
  const value={v:1,activeKey:readingKey('node',item.id),entries:[item]};
  value.entries[0].claimReference.packet=packet;value.entries[0].claimReading={text:'Never persist context'};
  const restored=validateReading(value);assertReference(restored);
  function assertReference(result){
    expect(result.entries[0].claimReference).toEqual(claimMaterialReference(packet,path));
    expect(JSON.stringify(result)).not.toContain('Never persist');
    expect(result.entries[0].claimReading).toBeUndefined();expect(result.entries[0].claimReference.packet).toBeUndefined();
  }
  assertReference(validateReading(JSON.parse(JSON.stringify(restored))));
  for(const mutate of [r=>r.claimId='another-claim',r=>r.closureNodeIds.shift(),r=>r.relationIds.push('extra'),
    r=>r.detailRelationIds.push(r.relationIds[0]),r=>r.closureNodeIds=Array.from({length:41},(_,i)=>'node:'+i)]){
    const broken=structuredClone(restored);mutate(broken.entries[0].claimReference);expect(()=>validateReading(broken)).toThrow();
  }
  for(const invalid of [null,false,0,'']){
    const broken=structuredClone(restored);broken.entries[0].claimReference=invalid;expect(()=>validateReading(broken)).toThrow();
  }
});
test('reading positions cannot migrate to another identity, version or a duplicate material',()=>{
  for(const change of [value=>value.entries.push(entry('three')),value=>value.entries[1]=entry(),value=>value.activeKey='other',
    value=>value.entries[0].contentRevision='c'.repeat(64),value=>value.entries[0].positions[0][1].anchor.offset=Infinity,
    value=>value.entries[0].positions[0][1].details=[['sources','yes']],value=>value.entries[0].positions[0][1].anchor.key='copied paragraph']){
    const value=state();change(value);expect(()=>validateReading(value)).toThrow();
  }
  expect(()=>readReading({getItem:()=>'{broken'})).toThrow();
  expect(()=>readReading({getItem:()=>'x'.repeat(200001)})).toThrow();
  const escaped=entry('"'.repeat(1024));expect(validateReading({v:1,entries:[escaped],activeKey:readingKey('node',escaped.id)}).entries[0]).toEqual(escaped);
});
test('durable reading positions strip page-local text fallback while retaining explicit paragraph anchors',()=>{
  const body={isConnected:true,scrollTop:90,getAttribute:()=>null,getClientRects:()=>[{}],getBoundingClientRect:()=>({top:0}),addEventListener:()=>{},
    querySelectorAll:selector=>selector==='details'?[]:[{dataset:{},textContent:'Private source sentence',getBoundingClientRect:()=>({top:-5,bottom:20})}]};
  const memory=createReadingMemory(body);memory.enter('view');memory.capture();
  expect(memory.exportPositions()).toEqual([['view',{top:90,details:[],anchor:null}]]);
  body.querySelectorAll=selector=>selector==='details'?[]:[{dataset:{readingAnchor:'description:2'},textContent:'Private source sentence',getBoundingClientRect:()=>({top:-5,bottom:20})}];
  memory.capture();expect(memory.exportPositions()[0][1].anchor).toEqual({key:'description:2',offset:-5});expect(JSON.stringify(memory.exportPositions())).not.toContain('Private');
});
test('captured diagnostic anchors cannot break paired export, strict validation and resume',()=>{
  const diagnostic={dataset:{readingAnchor:'unassigned-form'},textContent:'Private diagnostic source text',getBoundingClientRect:()=>({top:-12,bottom:80})};
  const body={isConnected:true,scrollTop:200,getAttribute:()=>null,getClientRects:()=>[{}],getBoundingClientRect:()=>({top:0}),addEventListener:()=>{},
    querySelectorAll:selector=>selector==='details'?[]:[diagnostic]};
  const memory=createReadingMemory(body),first=entry();memory.enter(first.positions[0][0]);memory.capture();
  first.positions=memory.exportPositions();const value={v:1,activeKey:readingKey('relation','opaque:two'),entries:[first,entry('opaque:two','relation')]};
  const saved=JSON.stringify(validateReading(value)),loaded=readReading({getItem:()=>saved});
  expect(loaded.entries[0].positions[0][1]).toEqual({top:200,details:[],anchor:null});expect(loaded.entries[1]).toEqual(value.entries[1]);
  expect(loaded.activeKey).toBe(value.activeKey);expect(saved).not.toContain('Private');
  const resumed=createReadingMemory(body);resumed.enter(first.positions[0][0]);resumed.importPositions(loaded.entries[0].positions);body.scrollTop=0;resumed.restore();expect(body.scrollTop).toBe(200);
  loaded.entries[0].positions[0][1].anchor={key:'unassigned-form',offset:-12};expect(()=>validateReading(loaded)).toThrow();
});
test('readable context positions roundtrip bounded numeric anchors without field names or wording',()=>{
  for(const key of ['record-context:0','record-context:1:part:42','claim-context:part:8','form:grounds:context:0:part:12']){
    const value=state();value.entries[0].positions[0][1].anchor={key,offset:-4};
    expect(readReading({getItem:()=>JSON.stringify(validateReading(value))}).entries[0].positions[0][1].anchor).toEqual({key,offset:-4});
  }
  for(const key of ['record-context:notes','record-context:0:part:source wording','record-context:0:part:1000000','claim-context:part:1\n']){
    const value=state();value.entries[0].positions[0][1].anchor={key,offset:0};expect(()=>validateReading(value)).toThrow();
  }
});
