import {test} from 'vitest';
import assert from 'node:assert/strict';
import {createNativeReference,validateNativeReference,nativeReferenceKey,matchNativeReference,nativeSelectedText} from './native-reference.mjs';
import {referenceKey,referenceDocumentId,referenceVersionId} from './model.mjs';
import {createCorpusNotebook,createMemoryCorpusNotebookState,readingSlot} from './notebook.mjs';
import {readNativeReference,readNativeSelection,NativeReferenceError} from './native-source.mjs';

const H='a'.repeat(64),B='b'.repeat(64);
function result(){return {status:'available',selection:{kind:'node',id:'source:tos.text-unit.fixture',source_revision:H,content_revision:B},
  record_ref:{id:'tos.text-unit.fixture',version:1,digest:'sha256:'+H},native_unit:{schema_version:'tos_native_public_unit_return_v1',
  packet:{id:'tos.source-text-unit-packet.fixture',version:1,sha256:H},summary:{unit_id:'tos.text-unit.fixture',unit_version:1,
    segmentation_id:'tos.text-segmentation.fixture',segmentation_version:1,layer_id:'tos.text-layer.fixture',layer_version:1},
  layer_record_sha256:H,representation_sha256:B,spans:[
    {anchor_ref:'tos.anchor.one',selector:{start:20,end:27},exact_sha256:H,text:'A😀e\u0301אבZ'},
    {anchor_ref:'tos.anchor.two',selector:{start:42,end:46},exact_sha256:B,text:'next'}]}};}

test('native references retain exact segmentation, span and code-point positions without joining gaps',()=>{
  const read=result(),reference=createNativeReference(read,0,{start:21,end:24});
  assert.equal(nativeSelectedText(reference,read.native_unit.spans[0].text),'😀e\u0301');
  assert.equal(referenceKey(reference),nativeReferenceKey(reference));
  assert.equal(referenceDocumentId(reference),'tos.text-layer.fixture');
  assert.deepEqual(matchNativeReference(reference,read),{status:'exact',spanIndex:0});
  for(const selector of [{start:19,end:21},{start:26,end:43},{start:21.5,end:22},{start:20,end:Number.MAX_SAFE_INTEGER+1}]){
    assert.throws(()=>createNativeReference(read,0,selector),TypeError);
  }
  const mutated=structuredClone(reference);mutated.target.packet.sha256+='\n';assert.throws(()=>validateNativeReference(mutated));
});

test('unrelated corpus growth preserves a reference; any pinned text identity change leaves it stale',()=>{
  const read=result(),reference=createNativeReference(read,1),old=JSON.stringify(reference);
  read.selection.source_revision='c'.repeat(64);
  assert.equal(matchNativeReference(reference,read).status,'exact');
  for(const mutate of [r=>r.native_unit.summary.segmentation_version++,r=>r.native_unit.summary.unit_version++,
    r=>r.native_unit.packet.sha256=B,r=>r.native_unit.layer_record_sha256=B,r=>r.native_unit.representation_sha256=H,
    r=>r.native_unit.spans[1].exact_sha256=H,r=>r.record_ref.version++,r=>r.native_unit.spans.pop()]){
    const changed=structuredClone(read);mutate(changed);assert.equal(matchNativeReference(reference,changed).status,'stale');
  }
  assert.equal(JSON.stringify(reference),old);
});

test('native note, exact position, CAS conflict and additive export/import use the existing notebook',async()=>{
  const memoryStore=createMemoryCorpusNotebookState(),book=createCorpusNotebook({adapter:'memory',memoryStore});
  const other=createCorpusNotebook({adapter:'memory',memoryStore}),ref=createNativeReference(result(),0,{start:21,end:24});
  const {item}=await book.putNote({kind:'note',reference:ref,text:'Keep the qualification.',quote:'😀e\u0301',expectedRecordRevision:null});
  const second=await other.putNote({id:item.id,kind:'note',reference:ref,text:'From another tab.',expectedRecordRevision:item.revision});
  await assert.rejects(book.putNote({id:item.id,kind:'note',reference:ref,text:'Stale edit.',expectedRecordRevision:item.revision}),e=>e.code==='conflict');
  await book.saveReading({slot:readingSlot(ref),documentId:referenceDocumentId(ref),versionId:referenceVersionId(ref),reference:ref,offset:17});
  assert.equal((await book.loadReading(readingSlot(ref))).offset,17);
  const copy=createCorpusNotebook({adapter:'memory'});await copy.importPacket(await book.exportPacket());
  const page=await copy.listNotes();assert.equal(page.items[0].text,second.item.text);assert.deepEqual(page.items[0].reference,ref);
  book.close();other.close();copy.close();
});

test('resume rediscovers the origin and validates the saved native binding before returning text',async()=>{
  const read=result(),ref=createNativeReference(read,0),calls=[];
  const client={inspect:async(...args)=>{calls.push(args);return {packet:{source_revision:B},match:{content_revision:H}};}};
  const exact=async(_client,selection,options)=>{calls.push([selection,options]);return {...read,selection};};
  const resolved=await readNativeReference(client,ref,{read:exact});
  assert.deepEqual(calls[0].slice(0,2),['node',ref.origin.id]);assert.ok(calls[0][2] instanceof AbortSignal);assert.equal(calls[1][0].source_revision,B);
  assert.deepEqual(resolved.reference,ref);
  read.native_unit.summary.segmentation_version++;
  await assert.rejects(readNativeReference(client,ref,{read:exact}),e=>e instanceof NativeReferenceError&&e.status==='stale');
});

test('source work is bounded to two operations and cancelled resume never returns a selection',async()=>{
  const read=result(),ref=createNativeReference(read,0),client={};let resolve;
  const pending=new Promise(done=>{resolve=done;}),options={read:async()=>pending};
  const a=readNativeSelection(client,read.selection,options),b=readNativeSelection(client,read.selection,options);
  await assert.rejects(readNativeSelection(client,read.selection,options),e=>e.status===429);resolve(read);await Promise.all([a,b]);
  const controller=new AbortController();controller.abort();
  await assert.rejects(readNativeReference(client,ref,{signal:controller.signal}),e=>e.name==='AbortError');
});

test('the resume deadline includes rediscovery even if a transport ignores cancellation',async()=>{
  const ref=createNativeReference(result(),0);
  await assert.rejects(readNativeReference({inspect:()=>new Promise(()=>{})},ref,{timeoutMs:10}),e=>e.status===504);
});
