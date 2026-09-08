import {test,expect} from 'vitest';
import {emptyReading,validateReading,readReading} from './reading-resume.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {readingKey} from './reader-model.mjs';

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
