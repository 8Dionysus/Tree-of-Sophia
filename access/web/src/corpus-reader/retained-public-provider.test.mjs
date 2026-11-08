import '../observatory/human-forms-wasm-test-runtime.mjs';
import {installReaderOccurrenceRules} from '../reader/occurrences-rust.mjs';
import * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {createHash} from 'node:crypto';
import {expect,test} from 'vitest';
import {createReference} from './model.mjs';
import {createCorpusReaderModel} from './view-model.mjs';
import {createRetainedPublicCorpusProvider} from './retained-public-provider.mjs';
installReaderOccurrenceRules(rules);
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
function fixture(){
 const files=new Map(),calls=[];
 const member=(path,value)=>{const bytes=Buffer.from(JSON.stringify(value));files.set('/static/public-corpus/'+path,bytes);return {path,sha256:hash(bytes),bytes:bytes.length};};
 function version(id,revision,text,unitId){
  const source={workId:'work',expressionId:'expression',editionId:'edition',itemId:'item',fileId:'file',fileSha256:revision,textLayerRef:'text.txt',textLayerSha256:revision};
  const page=member(`pages/${id}-${revision[0]}.json`,{documentId:'work',versionId:id,revision,units:[{id:unitId,ordinal:1,text,kind:'paragraph',reference:'Chapter'}]});
  return {id,label:id,language:'de',role:'source',source,revision,unitCount:1,pages:[{...page,firstOrdinal:1,count:1}],units:[{id:unitId,ordinal:1,page:0}],structure:[{id:'chapter',unitId,label:'Chapter',level:1,parentId:null}]};
 }
 const old=version('edition','a'.repeat(64),'🌳 old text','old-unit');
 const removed=version('removed','d'.repeat(64),'removed text','removed-unit');
 const oldStable=version('stable','e'.repeat(64),'same text','same-unit');
 const newer=version('edition','b'.repeat(64),'🌳 new text','new-unit');
 const currentStable=version('stable','e'.repeat(64),'same text','same-unit');
 currentStable.source.itemId='new-item';
 const oldRev='1'.repeat(64),newRev='2'.repeat(64);
 const doc=(name,versions)=>member(`documents/${name}.json`,{id:'work',title:'Work',author:'Author',versions});
 const entry=manifest=>({id:'work',title:'Work',author:'Author',manifest});
 const oldDoc=doc('old',[old,removed,oldStable]),newDoc=doc('new',[newer,currentStable]);
 const manifest=(revision,document)=>({schema_version:'tos_public_corpus_snapshot_v1',revision,profiles:{zarathustra:{documentIds:['work'],initialDocumentId:'work'},antiquity:{documentIds:['work'],initialDocumentId:'work'}},documents:[entry(document)]});
 const oldManifest=member(`revisions/${oldRev}.json`,manifest(oldRev,oldDoc));
 member('manifest.json',manifest(newRev,newDoc));
 member('retained.json',{manifests:[{schema:'tos_public_corpus_retained_manifest_v1',revision:oldRev,...oldManifest}]});
 const fetch=async path=>{calls.push(path);const body=files.get(path);return new Response(body??'{}',{status:body?200:404,headers:{'Content-Type':'application/json'}});};
 return {provider:createRetainedPublicCorpusProvider({fetch}),old,newer,removed,oldStable,oldRev,newRev,files,calls};
}

test('current, retained and absent exact text addresses choose the correct verified snapshot',async()=>{
 const {provider,old,newer,removed,oldRev,newRev}=fixture();
 const model=createCorpusReaderModel({provider});
 await model.open({documentId:'work',versionId:'edition',unitId:'new-unit',revision:newer.revision});
 expect(model.snapshot().document.versions.find(v=>v.id==='edition')).toMatchObject({revision:newer.revision,snapshotRevision:newRev,retainedSnapshot:false});
 await model.open({documentId:'work',versionId:'edition',unitId:'old-unit',revision:old.revision});
 expect(model.snapshot().document.versions.find(v=>v.id==='edition')).toMatchObject({revision:old.revision,snapshotRevision:oldRev,retainedSnapshot:true});
 expect(model.snapshot().windows['work\u0000edition'].units[0].text).toBe('🌳 old text');
 const ref=createReference({source:old.source,versionId:old.id,unitId:'old-unit',start:2,end:5});
 expect((await provider.resolve({reference:ref})).status).toBe('exact');
 const hits=await model.search({documentId:'work',versionId:'edition',query:'old',scope:'version'});
 expect(hits.items[0].reference).toMatchObject({unitId:'old-unit',selector:{start:2,end:5}});
 await model.open({documentId:'work',versionId:'removed',reference:createReference({source:removed.source,versionId:removed.id,unitId:'removed-unit',start:0,end:7})});
 expect(model.snapshot().windows['work\u0000removed'].units[0].text).toBe('removed text');
 await expect(model.open({documentId:'work',versionId:'edition',unitId:'missing',revision:'f'.repeat(64)})).rejects.toMatchObject({code:'retained-version-unavailable'});
 model.destroy();
});

test('an explicit retained snapshot is authenticated even when its layer is still current',async()=>{
 const {provider,oldRev,oldStable,calls}=fixture();
 const exact=createReference({source:oldStable.source,versionId:'stable',unitId:'same-unit',start:0,end:4});
 expect((await provider.resolve({reference:exact})).status).toBe('exact');
 expect((await provider.document({documentId:'work',versionId:'stable',reference:exact})).versions.find(v=>v.id==='stable').retainedSnapshot).toBe(true);
 const document=await provider.document({documentId:'work',versionId:'stable',unitId:'same-unit',revision:'e'.repeat(64),snapshotRevision:oldRev});
 expect(document.versions.find(v=>v.id==='stable')).toMatchObject({retainedSnapshot:true,snapshotRevision:oldRev});
 expect(calls).toContain(`/static/public-corpus/revisions/${oldRev}.json`);
 const broken=fixture();broken.files.set(`/static/public-corpus/revisions/${broken.oldRev}.json`,Buffer.from('{'));
 await expect(broken.provider.document({documentId:'work',versionId:'stable',revision:'e'.repeat(64),snapshotRevision:broken.oldRev})).rejects.toMatchObject({code:'invalid_response'});
});

test('a place lookup reads the snapshot that holds the place, and a lookup no snapshot can answer rejects',async()=>{
 const {provider}=fixture();
 // `edition` stands in both snapshots with other units: its old unit is read in the retained snapshot.
 expect(await provider.place({documentId:'work',versionId:'edition',targetVersionId:'stable',unitId:'old-unit'})).toEqual({unitId:'same-unit',by:'section'});
 expect(await provider.place({documentId:'work',versionId:'edition',targetVersionId:'stable',unitId:'new-unit'})).toEqual({unitId:'same-unit',by:'section'});
 // `removed` stands only in the retained snapshot, beside `stable`: the lookup reads that snapshot, by section.
 expect(await provider.place({documentId:'work',versionId:'removed',targetVersionId:'stable',unitId:'removed-unit'})).toEqual({unitId:'same-unit',by:'section'});
 await expect(provider.place({documentId:'work',versionId:'nowhere',targetVersionId:'stable',unitId:'u'})).rejects.toMatchObject({code:'retained-version-unavailable'});
});
