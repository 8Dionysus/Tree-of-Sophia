import {test,expect} from 'vitest';
import {createResearchWorkspace} from '../research-workspace';
import {capturePlace,pinHistoryPlace,readPlaces,savePlace} from './place-model.mjs';
import {COPY_SCHEMA,validateWorkspaceCopy,snapshotCopyStorage,copyStorageKeys,commitWorkspaceCopy} from './workspace-copy.mjs';
import {DEFAULT_INTERFACE} from './interface-model.mjs';
import {emptyReading,readReading,READING_KEY} from './reading-resume.mjs';
import {createTravelStore,HISTORY_KEY} from './travel-model.mjs';
import {readSaved} from './lens-model.mjs';
const packet={schema:'tos_lens_result_v1',source_revision:'a'.repeat(64),authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},
  nodes:[{id:'opaque:a',source_refs:['ToS/example'],content_revision:'b'.repeat(64),display:{title:{ru:'Delivered source text'}}}],relations:[],page:{next_cursor:'ephemeral'}};
function place(id){return capturePlace(packet,{lens:'constellations',yaw:0,pitch:0,zoom:1,pan:{x:0,y:0},selectedId:'opaque:'+id,relationId:null,panelOpen:true,cardTab:'about',vertices:[]},
  {name:'Место '+id,id,route:'?focus=opaque%3Aa',savedAt:1});}
function storage(){const values=new Map();return {values,getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};}
function copy(){const workspace=createResearchWorkspace({persistence:false});workspace.addNote({id:'note',body:'My thought'});return {schema:COPY_SCHEMA,v:1,exportedAt:'2026-09-07T20:00:00Z',
  history:{v:1,entries:[place('one'),place('two')],cursor:0},places:[place('saved')],lenses:[{v:2,name:'Моя линза',scope:'all',sources:['knowledge'],nodeIds:[],focusId:null,query:'мысль',kinds:[],predicates:[],depth:0,direction:'either',profile:'all',limit:20,relations:true,conditions:{nodes:[],relations:[]}}],resume:place('one'),preferences:structuredClone(DEFAULT_INTERFACE),reading:emptyReading(),research:JSON.parse(workspace.exportPacket())};}
test('whole workspace roundtrips through actual owner readers with bidirectional history and saved local notes',()=>{
  const s=storage(),value=copy();s.setItem('unrelated','keep');commitWorkspaceCopy(s,'/new',value,snapshotCopyStorage(s,'/new'));
  expect(createTravelStore(s,HISTORY_KEY+':/new').state).toEqual(value.history);expect(readPlaces(s)).toEqual(value.places);expect(readReading(s,READING_KEY+':/new')).toEqual(value.reading);
  const workspace=createResearchWorkspace({persistence:{load:()=>s.getItem('tos-research-workspace-v1'),save:()=>{}}});expect(workspace.getState().notes[0].body).toBe('My thought');
  expect(s.getItem('unrelated')).toBe('keep');expect(s.getItem(HISTORY_KEY+':/')).toBeNull();
  expect(readSaved(s)).toEqual(value.lenses);
  const serialized=JSON.stringify(validateWorkspaceCopy(value));for(const absent of ['Delivered source text','source_refs','ephemeral','authority_boundary'])expect(serialized).not.toContain(absent);
});
test('invalid or oversized sections, duplicate IDs and promoted hypotheses reject the entire import before any write',()=>{
  for(const mutate of [v=>v.schema='future',v=>v.history.cursor=20,v=>v.places.push(v.places[0]),v=>v.lenses.push(v.lenses[0]),v=>v.preferences.dock='elsewhere',
    v=>v.reading.entries=[{}],v=>v.research.hypotheses=[{id:'h',title:'title',body:'body',posture:{session_hypothesis:true,source:true,reviewed:true,canon:true}}]]){
    const s=storage(),before=snapshotCopyStorage(s,'/'),value=copy();mutate(value);expect(()=>commitWorkspaceCopy(s,'/',value,before)).toThrow();expect(s.values.size).toBe(0);
  }
  expect(()=>validateWorkspaceCopy('x'.repeat(4000001))).toThrow();
});
test('storage exhaustion rolls back every earlier write, and concurrent edits abort before replacement',()=>{
  const s=storage();for(const key of copyStorageKeys('/'))s.setItem(key,'old:'+key);const before=snapshotCopyStorage(s,'/');let writes=0;
  const quota={...s,setItem:(key,value)=>{if(++writes===4)throw new Error('quota');s.setItem(key,value);}};
  expect(()=>commitWorkspaceCopy(quota,'/',copy(),before)).toThrow(/Прежнее исследование восстановлено/);expect(snapshotCopyStorage(s,'/')).toEqual(before);
  s.setItem(copyStorageKeys('/')[1],'other-tab');expect(()=>commitWorkspaceCopy(s,'/',copy(),before)).toThrow(/изменилось/);expect(s.getItem(copyStorageKeys('/')[0])).toBe(before.get(copyStorageKeys('/')[0]));
});
test('pinning history is one idempotent save of that step; renaming preserves its query, revision and pose',()=>{
  const s=storage(),step=place('history-step'),pinned=pinHistoryPlace(s,step);expect(readPlaces(s)).toHaveLength(1);
  expect(pinned.pose).toEqual(step.pose);expect(pinned.spec).toEqual(step.spec);savePlace(s,{...pinned,name:'Моя мысль'});
  expect(pinHistoryPlace(s,step).name).toBe('Моя мысль');expect(readPlaces(s)).toHaveLength(1);expect(step.name).toBe('Место history-step');
});
