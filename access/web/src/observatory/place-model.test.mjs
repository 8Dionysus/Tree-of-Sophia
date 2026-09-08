import {test,expect} from 'vitest';
import {KnowledgeClient,focusSpec,specForPacket} from './knowledge-client.mjs';
import {capturePlace,validatePlace,readPlaces,savePlace,readResume,reopenPlace,PLACES_KEY,RESUME_KEY} from './place-model.mjs';
import {validatePose} from './view-state.mjs';
import {DEFAULT_INTERFACE,validateInterface} from './interface-model.mjs';
import {inclusionRoles} from './scene-feedback.mjs';
const rev='a'.repeat(64),boundary={is_source:false,is_canon:false,writes_to_tree:false};
const node=id=>({id,source_refs:['ToS/example'],content_revision:'b'.repeat(64),display:{title:{ru:id}}});
const packet=(revision=rev)=>({schema:'tos_lens_result_v1',source_revision:revision,authority_boundary:boundary,nodes:[node('opaque:a'),node('opaque:b')],relations:[],focus:{node_id:'opaque:a'}});
const pose=()=>({lens:'constellations',yaw:.6,pitch:-.1,zoom:1.3,pan:{x:12,y:35},selectedId:'opaque:a',relationId:null,panelOpen:true,cardTab:'relations',vertices:[{id:'opaque:a',slot:0,p:[0,1,2],sourcePosition:[0,1,2],target:[0,1,2],volumeZ:2}]});
const storage=()=>{const values=new Map();return {getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)};};
const place=()=>capturePlace(packet(),pose(),{name:'Истоки',id:'place-1',route:'?focus=opaque%3Aa',savedAt:1});

test('persisted views retain opaque identity and pose, not graph text, hidden fields or a cursor',()=>{
  const source=packet();source.page={next_cursor:'secret-cursor'};const value=capturePlace(source,{...pose(),raw:'not a view field'},{name:'Место',id:'one',route:''});
  const text=JSON.stringify(value);expect(text).not.toContain('secret-cursor');expect(text).not.toContain('source_refs');expect(text).not.toContain('not a view field');
  expect(value.spec.seed.node_ids).toEqual(['opaque:a','opaque:b']);expect(value.pose.vertices[0].id).toBe('opaque:a');
  expect(validatePlace({...value,endpoint:'https://untrusted.invalid',graph:source})).toEqual(value);
});
test('saved appearance rejects nonfinite camera values, duplicate vertices and out-of-budget requests',()=>{
  for(const invalid of [p=>p.zoom=Infinity,p=>p.pan.x=NaN,p=>p.vertices.push(p.vertices[0]),p=>p.vertices[0].target=[1,2,3,4],p=>p.lens='execute']){const p=pose();invalid(p);expect(()=>validatePose(p)).toThrow();}
  const value=place();value.spec.limits.nodes=41;expect(()=>validatePlace(value)).toThrow();value.spec.limits.nodes=40;value.spec.traversal.depth=4;expect(()=>validatePlace(value)).toThrow();
});
test('places are bounded, updates keep identity and corrupted storage remains untouched',()=>{
  const s=storage();for(let i=0;i<12;i++)savePlace(s,{...place(),id:String(i)});expect(readPlaces(s)).toHaveLength(12);
  expect(()=>savePlace(s,{...place(),id:'13'})).toThrow();savePlace(s,{...place(),id:'0',name:'Обновлено'});expect(readPlaces(s)[11].name).toBe('Обновлено');
  s.setItem(PLACES_KEY,'broken');expect(()=>readPlaces(s)).toThrow();expect(s.getItem(PLACES_KEY)).toBe('broken');
});
test('automatic return respects an explicit different route',()=>{
  const s=storage(),value=place();s.setItem(RESUME_KEY,JSON.stringify(value));expect(readResume(s,value.route)).toEqual(value);expect(readResume(s,'')).toEqual(value);expect(readResume(s,'?focus=someone-else')).toBeNull();
});
test('reopening always requests current data and reports a changed source revision',async()=>{
  const calls=[],value=place(),client={compile:async(...args)=>{calls.push(args);return packet('c'.repeat(64));}};
  value.spec.explain=false;
  const result=await reopenPlace(client,value);expect(calls).toHaveLength(1);expect(calls[0][0]).toEqual({...value.spec,explain:true});expect(value.spec.explain).toBe(false);expect(result.changed).toBe(true);expect(result.pose).toEqual(validatePose(pose()));
  client.compile=async()=>({...packet(),nodes:[]});await expect(reopenPlace(client,value)).rejects.toThrow('больше нет');
});
test('executed lens spec is copied before transport and retained only against its result',async()=>{
  const original=focusSpec('opaque:a');let sent;
  const client=new KnowledgeClient({fetcher:async(url,options)=>{sent=JSON.parse(options.body);return {ok:true,json:async()=>packet()};}});
  const loaded=await client.compile(original);original.seed.focus_node_id='changed';expect(specForPacket(loaded)).toEqual(sent);expect(specForPacket(packet())).toBeNull();
  const saved=capturePlace(loaded,pose(),{name:'Фокус',id:'focus',route:''});expect(saved.spec.seed.focus_node_id).toBe('opaque:a');
});
test('interface composition admits only registered tool IDs and bounded window sizes',()=>{
  const clean=validateInterface({...DEFAULT_INTERFACE,execute:'arbitrary',sizes:{evidence:{width:620,height:540},unknown:{width:500,height:400}}});expect(clean.execute).toBeUndefined();expect(clean.sizes.unknown).toBeUndefined();
  for(const value of [{...clean,pinned:['search','search']},{...clean,pinned:['unregistered-action']},{...clean,sizes:{studio:{width:9999,height:540}}}])expect(()=>validateInterface(value)).toThrow();
});
test('scene markers express query inclusion only and do not infer semantic importance',()=>{
  const p=packet();p.inclusion={authority:'query-execution-not-semantic-proof',nodes:{'opaque:a':{kind:'selector'},'opaque:b':{kind:'traversal'}}};expect([...inclusionRoles(p)]).toEqual([['opaque:a','matched'],['opaque:b','context']]);
  p.inclusion.authority='semantic-importance';expect(inclusionRoles(p).size).toBe(0);
  p.inclusion.authority='query-execution-not-semantic-proof';p.inclusion.nodes['opaque:b'].kind='future-kind';expect(inclusionRoles(p).has('opaque:b')).toBe(false);
});
