import {test} from 'vitest';
import assert from 'node:assert/strict';
import {readingForm,readingLanguages,readingSnapshot,readingDocument,createReadingShelf} from './reader-model.mjs';
import {RequestError,KnowledgeClient} from './knowledge-client.mjs';
import {compactFormLens,memberFormLens} from '../../fixtures/human-form-data.mjs';
import {resolveClaimReading} from './human-forms.mjs';

const revision='a'.repeat(64),content='b'.repeat(64);
const node=(id='opaque:one')=>({id,kind_id:'work',content_revision:content,source_refs:['ToS/test/source.md'],
  display:{title:{ru:'Предмет',original:'λόγος'},kind_label:{ru:'Произведение'},summary:{ru:'Возможно, это не та же трактовка.\nУсловие сохраняется.'},summary_state:'authored'},
  epistemic:{review_posture:'disputed',confidence:null}});
const answer=(raw=node(),rev=revision,endpoints=[])=>({packet:{source_revision:rev,endpoints},match:raw});
const target=(raw=node(),rev=revision)=>({raw,kind:'node',sourceRevision:rev,bookmark:{graph:{packet:{source_revision:rev}}}});
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};

const path=packet=>packet.scene.compact.claim_paths[0];
const compoundTarget=packet=>({...target(packet.nodes.find(node=>node.id===path(packet).claim_node_id),packet.source_revision),
  bookmark:{graph:{packet}},preferred:'ru'});
function compoundShelf(respond){
  const sent=[],client=new KnowledgeClient({fetcher:async(url,options)=>{
    const spec=JSON.parse(options.body);sent.push(spec);
    return {ok:true,json:async()=>respond(spec)};
  }});
  return {sent,shelf:createReadingShelf({client})};
}

test('the common shelf pins the full Claim closure and selects fresh language forms and every context atomically',async()=>{
  for(const fixture of [compactFormLens,memberFormLens]){
    const scene=fixture('ru'),before=structuredClone(scene),fresh=fixture('en');
    const {shelf,sent}=compoundShelf(spec=>spec.language==='en'?fresh:fixture('ru'));
    const {key}=shelf.pin(compoundTarget(scene));await tick();
    assert.ok(shelf.entries[0].snapshot.claimReading);assert.equal(shelf.entries.length,1);
    await shelf.language(key,'en');
    const snapshot=shelf.entries[0].snapshot;
    assert.deepEqual(snapshot.claimNodes,fresh.nodes);
    assert.deepEqual(snapshot.claimReading,resolveClaimReading(fresh,path(fresh).reading));
    assert.equal(snapshot.claimReading.wording.language,'en');
    assert.deepEqual(snapshot.raw.human_form_selection,fresh.nodes.find(node=>node.id===path(fresh).claim_node_id).human_form_selection);
    assert.deepEqual(sent.map(spec=>spec.lens_id),['sophia-observatory-claim-material','sophia-observatory-claim-material']);
    assert.deepEqual(sent[1].limits,{nodes:fresh.nodes.length,relations:fresh.relations.length,groups:fresh.nodes.length});
    assert.deepEqual(scene,before);
    fresh.relations[0].qualifiers.unknown='mutated outside shelf';
    assert.equal(snapshot.claimReading.relations[0].qualifiers.unknown,false);
  }
});

test('compound language changes pin every context revision, while refresh and restored selectors read the current closure',async()=>{
  const scene=compactFormLens();let fresh=compactFormLens();
  const {shelf}=compoundShelf(()=>fresh),{key}=shelf.pin(compoundTarget(scene));await tick();
  const reference=structuredClone(shelf.entries[0].claimReference);
  fresh=compactFormLens('en');fresh.relations[0].content_revision='f'.repeat(64);
  await shelf.language(key,'en');assert.equal(shelf.entries[0].snapshot,null);
  assert.ok(shelf.entries[0].error);assert.equal(shelf.entries[0].bookmark,null);
  fresh.source_revision='e'.repeat(64);await shelf.refresh(key);
  assert.equal(shelf.entries[0].snapshot.sourceRevision,fresh.source_revision);
  assert.equal(shelf.entries[0].snapshot.claimReading.relations[0].content_revision,'f'.repeat(64));
  assert.equal(shelf.entries[0].changed,true);assert.equal(shelf.entries[0].preferred,'en');
  shelf.restore([{kind:'node',id:path(scene).claim_node_id,sourceRevision:revision,contentRevision:content,preferred:'en',claimReference:reference}]);await tick();
  assert.equal(shelf.entries[0].snapshot.sourceRevision,fresh.source_revision);
  assert.deepEqual(shelf.entries[0].snapshot.claimReading,resolveClaimReading(fresh,path(fresh).reading));
  assert.equal(shelf.entries[0].bookmark,null);
});

test('a compound pin fails closed on missing closure, missing path or wrong requested language',async()=>{
  for(const mutate of [p=>p.nodes.pop(),p=>p.relations.pop(),p=>p.scene.compact.claim_paths=[],
    p=>p.nodes.find(node=>node.id==='fixture:claim').human_form_selection.requested_language='ru']){
    let fresh=memberFormLens('ru');const {shelf}=compoundShelf(()=>fresh);
    const {key}=shelf.pin(compoundTarget(memberFormLens()));await tick();assert.ok(shelf.entries[0].snapshot);
    fresh=memberFormLens('en');mutate(fresh);await shelf.language(key,'en');
    assert.equal(shelf.entries[0].snapshot,null);assert.equal(shelf.entries[0].bookmark,null);assert.ok(shelf.entries[0].error);
  }
});

test('a late compound language response cannot replace the newer entire reading packet',async()=>{
  const old=deferred();const {shelf}=compoundShelf(spec=>spec.language==='ru'?old.promise:compactFormLens('en'));
  const {key}=shelf.pin(compoundTarget(compactFormLens()));await shelf.language(key,'en');
  const snapshot=shelf.entries[0].snapshot;
  old.resolve(compactFormLens('ru'));await tick();
  assert.equal(shelf.entries[0].snapshot,snapshot);assert.equal(snapshot.claimReading.wording.language,'en');
});

test('an old identity-only Claim pin is explicitly incomplete and repinning its current path upgrades it without another shelf slot',async()=>{
  const scene=compactFormLens(),raw=scene.nodes.find(node=>node.id===path(scene).claim_node_id);
  const {shelf,sent}=compoundShelf(spec=>spec.lens_id==='sophia-observatory-material'
    ?{...answer().packet,schema:'tos_lens_result_v1',authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},nodes:[raw],relations:[]}
    :compactFormLens(spec.language));
  shelf.restore([{kind:'node',id:raw.id,sourceRevision:revision,contentRevision:content,preferred:'ru'}]);await tick();
  assert.equal(readingDocument(shelf.entries[0].snapshot).claimContextUnavailable,true);
  const key=shelf.entries[0].key;
  assert.equal(shelf.pin(compoundTarget(scene)).existing,true);await tick();
  assert.equal(shelf.entries.length,1);assert.equal(shelf.entries[0].key,key);
  assert.ok(shelf.entries[0].snapshot.claimReading);assert.equal(readingDocument(shelf.entries[0].snapshot).claimContextUnavailable,false);
  assert.deepEqual(sent.map(spec=>spec.lens_id),['sophia-observatory-material','sophia-observatory-claim-material']);
});

test('a navigation-only pin retains one identity across language changes and reopening',async()=>{
  const raw=node();raw.display.title={ru:'Запись утверждения',en:'Claim record'};
  raw.display.provenance={title:'navigation-template',source_title_available:false};
  const before=structuredClone(raw),shelf=createReadingShelf({client:{readMaterial:async()=>answer(raw)}});
  const {key}=shelf.pin({...target(raw),preferred:'ru'});await tick();
  for(const language of ['en','ru','en']){
    await shelf.language(key,language);assert.equal(shelf.entries.length,1);assert.equal(shelf.entries[0].key,key);
    const doc=readingDocument(shelf.entries[0].snapshot,language);
    assert.equal(doc.title.text,raw.display.title[language]);assert.equal(doc.title.navigationOnly,true);assert.equal(doc.humanForms,null);
  }
  shelf.restore([{kind:'node',id:raw.id,preferred:'en',sourceRevision:revision,contentRevision:content}]);await tick();
  assert.equal(shelf.entries[0].key,key);assert.equal(shelf.entries[0].preferred,'en');
  assert.equal(readingDocument(shelf.entries[0].snapshot,shelf.entries[0].preferred).title.text,'Claim record');
  assert.deepEqual(raw,before);
});

test('an identifier fallback is a UI title gap, never a selected source form or authored statement',()=>{
  const raw=node();raw.display.title={default:'claim:tos claim opaque'};raw.display.provenance={title:'identifier-fallback'};
  const snapshot=readingSnapshot(answer(raw),'node'),doc=readingDocument(snapshot,'en');
  assert.deepEqual(doc.title,{text:'Произведение · Нет читаемого названия',key:null,lang:null,fallback:false,unavailable:true});
  assert.equal(doc.humanForms,null);assert.equal(doc.blocks[0].form.text,raw.display.summary.ru);
  assert.deepEqual(snapshot.raw.display,raw.display);assert.equal(snapshot.raw.id,raw.id);
});

test('reading preserves exact qualifications, provenance and missing descriptions',()=>{
  const original=answer(),before=structuredClone(original),snapshot=readingSnapshot(original,'node');
  const doc=readingDocument(snapshot);
  assert.equal(doc.blocks[0].form.text,original.match.display.summary.ru);
  assert.equal(doc.posture.review_posture,'disputed');assert.equal(doc.posture.confidence,null);
  assert.deepEqual(doc.sourceRefs,original.match.source_refs);assert.deepEqual(original,before);
  original.match.display.summary.ru='Changed elsewhere';assert.notEqual(snapshot.raw.display.summary.ru,'Changed elsewhere');
  const missing=node();missing.display.summary_state='missing';missing.display.summary.ru='Generic placeholder';
  assert.equal(readingDocument(readingSnapshot(answer(missing),'node')).blocks[0].form,null);
});

test('actual language selection is explicit and never labels an unknown-language default as Russian',()=>{
  const text={default:'Quelle',original:'λόγος',en:'word','ar':'لفظ'};
  assert.deepEqual(readingForm(text),{text:'Quelle',key:'default',lang:null,fallback:true});
  assert.equal(readingForm(text,'original').lang,null);
  assert.deepEqual(readingForm(text,'ar'),{text:'لفظ',key:'ar',lang:'ar',fallback:false});
  const raw=node();raw.display.summary=text;
  assert.ok(readingLanguages(readingSnapshot(answer(raw),'node')).includes('ar'));
  assert.equal(readingForm({ru:'   ',en:'word'}).key,'en');
});

test('a relation keeps its exact statement and both participants without synthesizing a conclusion',()=>{
  const raw={...node('edge:one'),from_id:'left',to_id:'right',display:{label:{ru:'Атрибуция'},statement:{ru:'Авторство оспаривается.'},explanation:{ru:'Два свидетельства расходятся.'}}};
  const snapshot=readingSnapshot(answer(raw,revision,[node('left'),node('right')]),'relation'),doc=readingDocument(snapshot);
  assert.equal(doc.blocks[0].form.text,'Авторство оспаривается.');assert.deepEqual(doc.participants.map(p=>p.id),['left','right']);
  assert.throws(()=>readingSnapshot(answer(raw),'relation'));
});

test('two identical labels remain two exact objects and a third pin does not evict either',async()=>{
  const shelf=createReadingShelf({client:{readMaterial:async(kind,id)=>answer(node(id))}});
  shelf.pin(target(node('one')));shelf.pin(target(node('two')));
  assert.equal(shelf.pin(target(node('one'))).existing,true);
  assert.throws(()=>shelf.pin(target(node('three'))),/два материала/);
  await tick();assert.deepEqual(shelf.entries.map(e=>e.id),['one','two']);
  assert.ok(shelf.entries.every(e=>e.snapshot&&!e.loading));
});

test('removing and repinning the same identity cannot admit a late response',async()=>{
  const first=deferred(),second=deferred();let calls=0;
  const shelf=createReadingShelf({client:{readMaterial:()=>++calls===1?first.promise:second.promise}});
  const {key}=shelf.pin(target());shelf.remove(key);shelf.pin(target());
  second.resolve(answer(node('opaque:one'),'c'.repeat(64)));await tick();
  first.resolve(answer());await tick();
  assert.equal(shelf.entries[0].snapshot.sourceRevision,'c'.repeat(64));
  assert.equal(shelf.entries[0].bookmark,null);
});

test('a snapshot change is explicit, network failure preserves the reading, revoked availability clears it',async()=>{
  let error=null;
  const shelf=createReadingShelf({client:{readMaterial:async()=>{if(error)throw error;return answer();}}});
  const {key}=shelf.pin(target());await tick();
  shelf.observeRevision('c'.repeat(64));assert.notEqual(shelf.sceneRevision,shelf.entries[0].sourceRevision);
  error=new RequestError(0,'Нет связи');await shelf.refresh(key);
  assert.ok(shelf.entries[0].snapshot);assert.equal(shelf.entries[0].error,'Нет связи');
  error=new RequestError(403,'Доступ ограничен');await shelf.refresh(key);
  assert.equal(shelf.entries[0].snapshot,null);assert.equal(shelf.entries[0].bookmark,null);
});

test('restored pairs fetch current material without carrying stored text or revision pins; late old requests stay excluded',async()=>{
  const late=deferred(),calls=[];let first=true;
  const shelf=createReadingShelf({client:{readMaterial:(kind,id,signal,source,revision)=>{calls.push({kind,id,source,revision});if(first){first=false;return late.promise;}return Promise.resolve(answer(node(id)));}}});
  shelf.pin(target(node('old')));
  const references=['one','two'].map(id=>({kind:'node',id,sourceRevision:revision,contentRevision:content}));
  shelf.restore(references);await tick();
  assert.deepEqual(shelf.entries.map(e=>e.id),['one','two']);assert.ok(shelf.entries.every(e=>e.snapshot&&!e.bookmark));
  assert.ok(calls.slice(1).every(call=>call.source===null&&call.revision===undefined));
  late.resolve(answer(node('old')));await tick();assert.deepEqual(shelf.entries.map(e=>e.id),['one','two']);
  shelf.restore([{...references[0],contentRevision:'c'.repeat(64)}]);await tick();assert.equal(shelf.entries[0].changed,true);
});
