import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test} from 'vitest';
import {createReference} from './model.mjs';
import {bonsaiCorrespondences} from './bonsai-correspondence.mjs';
import {lexicalReading} from './bonsai-lexical-readings.mjs';
import chapter from '../../../../bonsai/zarathustra/witnesses/I.01.json';
import witnesses from '../../../../bonsai/zarathustra/witnesses.json';

const source={workId:witnesses.corpus.documentId,expressionId:'x',editionId:'e',itemId:'i',fileId:'f',fileSha256:'a'.repeat(64),textLayerRef:'source.txt',textLayerSha256:'b'.repeat(64)};
const version={id:witnesses.corpus.dta.versionId,source,units:chapter.units.de.map(([id])=>({id})),structure:[{id:'bonsai.de.I.01',unitId:chapter.units.de[0][0]}]};
const targets=['en','ru'].map(lang=>({id:witnesses.corpus[lang].versionId,label:lang,source:{...source,editionId:lang}}));
const document={versions:[version,...targets]};
const all=new Map(['en','ru'].flatMap(lang=>chapter.units[lang].map(([id,,raw])=>[id,raw])));

test('sentence correspondences retain exact witness units and reject changed target bytes',async()=>{
 const raw=chapter.units.de.find(([id])=>id==='tos.text-unit.sid-1af39a4d5256f672c2e64f1b997143f0')[2];
 const start=Array.from(raw.slice(0,raw.indexOf('schenken'))).length;
 const reference=createReference({source,versionId:version.id,unitId:'tos.text-unit.sid-1af39a4d5256f672c2e64f1b997143f0',start,end:start+8});
 const args={document,version,reference,form:'schenken',readTarget:async (_,id)=>all.get(id)};
 const found=await bonsaiCorrespondences(args);
 expect(found.map(item=>item.targetLanguage)).toEqual(expect.arrayContaining(['en','ru']));
 for(const item of found)expect(Array.from(all.get(item.targetReference.unitId)).slice(item.targetReference.selector.start,item.targetReference.selector.end).join('')).toBe(item.targetQuote);
 expect(await bonsaiCorrespondences({...args,readTarget:async()=> 'changed'})).toEqual([]);
});

test('bounded lexical observations only appear for the named witness and occurrence',()=>{
 const unitId='tos.text-unit.sid-23adf687a84b3225f8781f8fc5aeb845';
 expect(lexicalReading({versionId:version.id,unitId},'Brüder')?.lemma).toBe('Bruder');
 expect(lexicalReading({versionId:witnesses.corpus.de.versionId,unitId},'Brüder')).toBeNull();
 expect(lexicalReading({versionId:version.id,unitId},'Bruder')).toBeNull();
});
