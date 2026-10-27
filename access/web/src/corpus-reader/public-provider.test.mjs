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
function fixture({corrupt=false,text='🌳 Kreis und Kreis.',terms=[{form:'Kreis',occurrences:[[1,2,7],[1,12,17]]}]}={}){
 const files=new Map(),calls=[];
 const member=(path,value)=>{const bytes=Buffer.from(JSON.stringify(value)+'\n');files.set('/static/public-corpus/'+path,bytes);return {path,sha256:hash(bytes),bytes:bytes.length};};
 const source={workId:'work',expressionId:'expression',editionId:'edition',itemId:'item',fileId:'file',fileSha256:'a'.repeat(64),textLayerRef:'source.txt',textLayerSha256:'b'.repeat(64)};
 const units=[{id:'unit',ordinal:1,text,kind:'paragraph',reference:'Section 1'}];
 const page=member('pages/text.json',{documentId:'work',versionId:'v1',revision:source.textLayerSha256,units});
 const index=member('indexes/forms.json',{terms});
 const version={id:'v1',label:'Edition',language:'de',revision:source.textLayerSha256,source,unitCount:1,pages:[{...page,firstOrdinal:1,count:1}],units:[{id:'unit',ordinal:1,page:0}],structure:[],searchIndex:index};
 const document={id:'work',title:'Work',author:'Author',versions:[version]};
 const descriptor=member('documents/work.json',document);
 member('manifest.json',{schema_version:'tos_public_corpus_snapshot_v1',revision:'c'.repeat(64),profiles:{zarathustra:{documentIds:['work'],initialDocumentId:'work'},antiquity:{documentIds:['work'],initialDocumentId:'work'}},documents:[{id:'work',title:'Work',author:'Author',manifest:descriptor}]});
 if(corrupt)files.set('/static/public-corpus/pages/text.json',Buffer.from(JSON.stringify({documentId:'work',units:[{text:'changed'}]})+'\n'));
 const fetch=async path=>{calls.push(path);const body=files.get(path);return new Response(body??'{}',{status:body?200:404,headers:{'Content-Type':'application/json'}});};
 return {provider:createPublicCorpusProvider({fetch}),source,document,version,calls};
}
function pagedFixture(){
 const files=new Map();
 const member=(path,value)=>{const bytes=Buffer.from(JSON.stringify(value)+'\n');files.set('/static/public-corpus/'+path,bytes);return {path,sha256:hash(bytes),bytes:bytes.length};};
 const source={workId:'work',expressionId:'expression',editionId:'edition',itemId:'item',fileId:'file',fileSha256:'a'.repeat(64),textLayerRef:'source.txt',textLayerSha256:'b'.repeat(64)};
 const units=Array.from({length:80},(_,index)=>({id:`unit-${index+1}`,ordinal:index+1,text:`Text ${index+1}`,kind:'paragraph'}));
 const pages=[];
 for(let pageIndex=0;pageIndex<2;pageIndex++){
  const pageUnits=units.slice(pageIndex*40,(pageIndex+1)*40);
  pages.push(member(`pages/page-${pageIndex}.json`,{documentId:'work',versionId:'v1',revision:source.textLayerSha256,units:pageUnits}));
 }
 const version={id:'v1',label:'Edition',language:'en',revision:source.textLayerSha256,source,unitCount:units.length,
  pages:pages.map((page,index)=>({...page,firstOrdinal:index*40+1,count:40})),
  units:units.map((unit,index)=>({id:unit.id,ordinal:unit.ordinal,page:Math.floor(index/40)})),structure:[]};
 const document={id:'work',title:'Work',author:'Author',versions:[version]},descriptor=member('documents/work.json',document);
 member('manifest.json',{schema_version:'tos_public_corpus_snapshot_v1',revision:'c'.repeat(64),profiles:{zarathustra:{documentIds:['work'],initialDocumentId:'work'},antiquity:{documentIds:['work'],initialDocumentId:'work'}},documents:[{id:'work',title:'Work',author:'Author',manifest:descriptor}]});
 const fetch=async path=>{const body=files.get(path);return new Response(body??'{}',{status:body?200:404,headers:{'Content-Type':'application/json'}});};
 return {provider:createPublicCorpusProvider({fetch})};
}
function mappingFixture({targetStale=false,analysis=false}={}){
 const files=new Map();
 const member=(path,value)=>{const bytes=Buffer.from(JSON.stringify(value)+'\n');files.set('/static/public-corpus/'+path,bytes);return {path,sha256:hash(bytes),bytes:bytes.length};};
 const source={workId:'work',expressionId:'expression',editionId:'de-edition',itemId:'de-item',fileId:'de-file',fileSha256:'a'.repeat(64),textLayerRef:'de.txt',textLayerSha256:'b'.repeat(64)};
 const targetSource={workId:'work',expressionId:'expression',editionId:'en-edition',itemId:'en-item',fileId:'en-file',fileSha256:'c'.repeat(64),textLayerRef:'en.txt',textLayerSha256:'d'.repeat(64)};
 const sourcePage=member('pages/de.json',{documentId:'work',versionId:'de',revision:source.textLayerSha256,units:[{id:'de-unit',ordinal:1,text:'Gold',kind:'paragraph'}]});
 const targetPage=member('pages/en.json',{documentId:'work',versionId:'en',revision:targetSource.textLayerSha256,units:[{id:'en-unit',ordinal:1,text:'Virtue',kind:'paragraph'}]});
 const analysisDescriptor=analysis?member('analyses/de.json',{schema_version:'tos_public_morphology_candidates_v1',textLayerSha256:source.textLayerSha256,language:'de',analysisVersion:'fixture',source:{id:'fixture'},forms:{Gold:{candidates:[{lemma:'Gold',morphology:'noun'}]}}}):null;
 const sourceVersion={id:'de',label:'German',language:'de',revision:source.textLayerSha256,source,unitCount:1,pages:[{...sourcePage,firstOrdinal:1,count:1}],units:[{id:'de-unit',ordinal:1,page:0}],structure:[],...(analysis?{analysis:analysisDescriptor}:{})};
 const targetVersion={id:'en',label:'English',language:'en',revision:targetSource.textLayerSha256,source:targetSource,unitCount:1,pages:[{...targetPage,firstOrdinal:1,count:1}],units:[{id:'en-unit',ordinal:1,page:0}],structure:[]};
 const sourceReference=createReference({source,versionId:'de',unitId:'de-unit',start:0,end:4});
 const targetReference=createReference({source:{...targetSource,textLayerSha256:targetStale?'e'.repeat(64):targetSource.textLayerSha256},versionId:'en',unitId:'en-unit',start:0,end:6});
 const entry={alignmentId:'meno-70-test',status:'proposed',reviewStatus:'model_proposal_owner_review_pending',sourceReference,sourceQuote:'Gold',sourceVersionId:'de',sourceVersionLabel:'German',sourceLanguage:'de',sourceRevision:source.textLayerSha256,targetReference,targetQuote:'Virtue',targetVersionId:'en',targetVersionLabel:'English',targetLanguage:'en',targetRevision:targetReference.target.textLayerSha256,reviewBasis:'bounded fixture proposal',evidence:[]};
 const mappings=member('mappings/proposals.json',{schema_version:'tos_public_translation_mappings_v1',documentId:'work',entries:[entry]});
 const document={id:'work',title:'Work',author:'Author',versions:[sourceVersion,targetVersion],mappings};
 const descriptor=member('documents/work.json',document);
 member('manifest.json',{schema_version:'tos_public_corpus_snapshot_v1',revision:'c'.repeat(64),profiles:{zarathustra:{documentIds:['work'],initialDocumentId:'work'},antiquity:{documentIds:['work'],initialDocumentId:'work'}},documents:[{id:'work',title:'Work',author:'Author',manifest:descriptor}]});
 const fetch=async path=>{const body=files.get(path);return new Response(body??'{}',{status:body?200:404,headers:{'Content-Type':'application/json'}});};
 return {provider:createPublicCorpusProvider({fetch}),sourceReference,targetReference};
}
describe('immutable public corpus delivery',()=>{
 it('searches source text across indexed form delimiters without losing exact Unicode offsets',async()=>{
  const {provider,calls}=fixture({text:'🌳 sense-organs and sense organs.',terms:[{form:'sense',occurrences:[[1,2,7]]},{form:'organs',occurrences:[[1,8,14]]}]});
  const result=await provider.search({documentId:'work',versionId:'v1',query:'sense-organs',scope:'version',limit:10});
  expect(result.items).toHaveLength(1);
  expect(result.items[0].exactReference.selector).toMatchObject({start:2,end:14});
  expect((await provider.resolve({reference:result.items[0].exactReference})).status).toBe('exact');
  expect(calls.some(path=>path.includes('/pages/'))).toBe(true);
 });
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
 it('pages backward from a deep jump without skipping the visible tail of the current page',async()=>{
  const {provider}=pagedFixture();
  const deep=await provider.window({documentId:'work',versionId:'v1',unitId:'unit-70',limit:40});
  const previous=await provider.window({documentId:'work',versionId:'v1',cursor:deep.previousCursor,limit:40});
  expect(previous.units.map(unit=>unit.ordinal)).toEqual(Array.from({length:40},(_,index)=>index+41));
  const priorPage=await provider.window({documentId:'work',versionId:'v1',cursor:previous.previousCursor,limit:40});
  expect(priorPage.units.map(unit=>unit.ordinal)).toEqual(Array.from({length:40},(_,index)=>index+1));
  const short=await provider.window({documentId:'work',versionId:'v1',unitId:'unit-70',limit:10});
  const shortPrevious=await provider.window({documentId:'work',versionId:'v1',cursor:short.previousCursor,limit:10});
  expect(shortPrevious.units.map(unit=>unit.ordinal)).toEqual(Array.from({length:10},(_,index)=>index+60));
 });
 it('uses a finite nonempty window when the low-level adapter receives no limit',async()=>{
  const {provider}=pagedFixture();
  const window=await provider.window({documentId:'work',versionId:'v1',unitId:'unit-41'});
  expect(window.units).toHaveLength(40);
  expect(window.units[0].ordinal).toBe(41);
 });
 it('returns only exact target mappings and drops stale target references',async()=>{
  const live=mappingFixture();
  const exact=await live.provider.analyze({reference:live.sourceReference,form:'Gold',language:'de'});
  expect(exact).toMatchObject({status:'unavailable',mappingStatus:'available',mappings:[{targetQuote:'Virtue',reviewStatus:'model_proposal_owner_review_pending'}]});
  const reverse=await live.provider.analyze({reference:live.targetReference,form:'Virtue',language:'en'});
  expect(reverse).toMatchObject({status:'unavailable',mappingStatus:'available',mappings:[{targetQuote:'Gold',navigationDirection:'target_to_source'}]});
  expect(referenceKey(reverse.mappings[0].targetReference)).toBe(referenceKey(live.sourceReference));
  const stale=mappingFixture({targetStale:true});
  const absent=await stale.provider.analyze({reference:stale.sourceReference,form:'Gold',language:'de'});
  expect(absent.status).toBe(exact.status);
  expect(absent.mappingStatus).toBe('none');
  expect(absent.mappings).toEqual([]);
 });
 it('keeps a form search link separate from candidate lemma links',async()=>{
  const {provider,sourceReference}=mappingFixture({analysis:true});
  const result=await provider.analyze({reference:sourceReference,form:'Gold',language:'de'});
  expect(result.dictionaryLinks[0].url).toBe('https://www.dwds.de/wb/Gold');
  expect(result.dictionaryLinks[0].label.en).toMatch(/form/i);
  expect(result.candidates[0].dictionaryUrl).toBe('https://www.dwds.de/wb/Gold');
 });
});
