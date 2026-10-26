import '../observatory/human-forms-wasm-test-runtime.mjs';
import {installReaderOccurrenceRules} from '../reader/occurrences-rust.mjs';
import * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {createPublicCorpusProvider} from './public-provider.mjs';
import {createReference,referenceKey} from './model.mjs';
import {normalizeSearch} from './view-model.mjs';
import {describe,it,expect} from 'vitest';
import {createHash} from 'node:crypto';
installReaderOccurrenceRules(rules);
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
function fixture({corrupt=false}={}){
 const files=new Map(),calls=[];
 const member=(path,value)=>{const bytes=Buffer.from(JSON.stringify(value)+'\n');files.set('/static/public-corpus/'+path,bytes);return {path,sha256:hash(bytes),bytes:bytes.length};};
 const source={workId:'work',expressionId:'expression',editionId:'edition',itemId:'item',fileId:'file',fileSha256:'a'.repeat(64),textLayerRef:'source.txt',textLayerSha256:'b'.repeat(64)};
 const units=[{id:'unit',ordinal:1,text:'🌳 Kreis und Kreis.',kind:'paragraph',reference:'Section 1'}];
 const page=member('pages/text.json',{documentId:'work',versionId:'v1',revision:source.textLayerSha256,units});
 const index=member('indexes/forms.json',{terms:[{form:'Kreis',occurrences:[[1,2,7],[1,12,17]]}]});
 const version={id:'v1',label:'Edition',language:'de',revision:source.textLayerSha256,source,unitCount:1,pages:[{...page,firstOrdinal:1,count:1}],units:[{id:'unit',ordinal:1,page:0}],structure:[],searchIndex:index};
 const document={id:'work',title:'Work',author:'Author',versions:[version]};
 const descriptor=member('documents/work.json',document);
 member('manifest.json',{schema_version:'tos_public_corpus_snapshot_v1',revision:'c'.repeat(64),profiles:{zarathustra:{documentIds:['work'],initialDocumentId:'work'},antiquity:{documentIds:['work'],initialDocumentId:'work'}},documents:[{id:'work',title:'Work',author:'Author',manifest:descriptor}]});
 if(corrupt)files.set('/static/public-corpus/pages/text.json',Buffer.from(JSON.stringify({documentId:'work',units:[{text:'changed'}]})+'\n'));
 const fetch=async path=>{calls.push(path);const body=files.get(path);return new Response(body??'{}',{status:body?200:404,headers:{'Content-Type':'application/json'}});};
 return {provider:createPublicCorpusProvider({fetch}),source,document,version,calls};
}
describe('immutable public corpus delivery',()=>{
 it('keeps the second equal word and its code-point coordinates through search, view and exact resolve',async()=>{
  const {provider,source,document,version,calls}=fixture();
  const page=await provider.search({documentId:'work',versionId:'v1',query:'Kreis',scope:'version',limit:1});
  const next=await provider.search({documentId:'work',versionId:'v1',query:'Kreis',scope:'version',limit:1,cursor:page.nextCursor});
  expect(next.items[0].exactReference.selector).toMatchObject({start:12,end:17});
  const normalized=normalizeSearch(next,{document,version});
  const reference=createReference({source,versionId:'v1',unitId:'unit',start:12,end:17});
  expect(referenceKey(normalized.items[0].reference)).toBe(referenceKey(reference));
  expect((await provider.resolve({reference})).status).toBe('exact');
  expect(calls.filter(path=>path.includes('/pages/'))).toHaveLength(1);
  await expect(provider.search({documentId:'work',versionId:'v1',query:'different',scope:'version',cursor:page.nextCursor})).rejects.toMatchObject({code:'stale_cursor'});
 });
 it('preserves an old reference as changed and refuses altered delivered bytes',async()=>{
  const {provider,source}=fixture();
  const old=createReference({source:{...source,textLayerSha256:'d'.repeat(64)},versionId:'v1',unitId:'unit',start:2,end:7});
  const result=await provider.resolve({reference:old});
  expect(result.status).toBe('changed');expect(referenceKey(result.reference)).toBe(referenceKey(old));
  const bad=fixture({corrupt:true});
  await expect(bad.provider.window({documentId:'work',versionId:'v1'})).rejects.toMatchObject({code:'invalid_response'});
 });
});
