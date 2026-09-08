import {test} from 'vitest';
import assert from 'node:assert/strict';
import {readingForm,readingLanguages,readingSnapshot,readingDocument,createReadingShelf} from './reader-model.mjs';
import {RequestError} from './knowledge-client.mjs';

const revision='a'.repeat(64),content='b'.repeat(64);
const node=(id='opaque:one')=>({id,kind_id:'work',content_revision:content,source_refs:['ToS/test/source.md'],
  display:{title:{ru:'Предмет',original:'λόγος'},kind_label:{ru:'Произведение'},summary:{ru:'Возможно, это не та же трактовка.\nУсловие сохраняется.'},summary_state:'authored'},
  epistemic:{review_posture:'disputed',confidence:null}});
const answer=(raw=node(),rev=revision,endpoints=[])=>({packet:{source_revision:rev,endpoints},match:raw});
const target=(raw=node(),rev=revision)=>({raw,kind:'node',sourceRevision:rev,bookmark:{graph:{packet:{source_revision:rev}}}});
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};

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
