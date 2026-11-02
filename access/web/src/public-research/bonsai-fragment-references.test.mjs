import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {describe,it} from 'vitest';
import * as atlas from '../../constructor/atlas-content.mjs';
import * as routes from '../../constructor/atlas-routes.mjs';
import * as guides from '../../constructor/close-reading-guides.mjs';
import * as foundation from '../../constructor/inquiry-foundation.mjs';
import * as readings from '../../constructor/inquiry-readings-echoes.mjs';
import * as sources from '../../constructor/inquiry-sources-concepts.mjs';
import * as relations from '../../constructor/relation-inquiry.mjs';
import '../observatory/human-forms-wasm-test-runtime.mjs';
import {createFragments} from './fragments.mjs';
import {readResearchSource} from './page-agent.mjs';

const read=path=>JSON.parse(readFileSync(new URL('../../../../'+path,import.meta.url),'utf8'));
function check(value,stand){
  const places=read(`bonsai/${stand}/fragments.json`).places,chapters=new Map();
  const visit=(v,path)=>{
    if(!v||typeof v!=='object')return;
    if(v.w&&(v.c||v.loc||Array.isArray(v.u))){
      if(v.loc)assert.ok(places[v.w]?.[v.loc],`${path}: missing ${v.w} ${v.loc}`);
      else if(v.c){
        assert.equal(v.w,'also-sprach-zarathustra',`${path}: chapter of another work`);
        if(!chapters.has(v.c))chapters.set(v.c,read(`bonsai/zarathustra/witnesses/${v.c}.json`));
        const units=chapters.get(v.c).units.de,a=units.findIndex(u=>u[0]===v.u?.[0]),b=units.findIndex(u=>u[0]===v.u?.[1]);
        assert.ok(a>=0&&b>=a,`${path}: invalid endpoints`);
        const s=v.s??0,e=v.e??units[b][2].length;
        assert.ok(Number.isInteger(s)&&Number.isInteger(e)&&s>=0&&s<=units[a][2].length&&e>=0&&e<=units[b][2].length&&(a!==b||e>s),`${path}: invalid offsets`);
      }else assert.ok(v.u.length===2&&v.u.every(id=>typeof id==='string'&&id),`${path}: invalid corpus endpoints`);
    }
    for(const [k,item] of Object.entries(v))visit(item,path+'.'+k);
  };
  visit(value,stand);
}

describe('every authored fragment reference',()=>{
  it('lets the agent read a complete multi-page citation with exact native selectors',async()=>{
    const witness=read('bonsai/zarathustra/witnesses.json'),edition=witness.corpus.dta;
    const raw=read('bonsai/zarathustra/witnesses/III.02.json').units.de.slice(5,18);
    const units=raw.map(([id,kind,text],i)=>({id,kind,text,ordinal:i+1}));
    const cite={w:'also-sprach-zarathustra',c:'III.02',u:[units[0].id,units.at(-1).id],s:1,e:units.at(-1).text.length-1};
    const source={workId:witness.corpus.documentId,expressionId:'expression',editionId:'edition',itemId:'item',fileId:'file',fileSha256:'a'.repeat(64),textLayerRef:'dta.txt',textLayerSha256:edition.revision};
    let changed=false;
    const provider={
      async document(args){assert.equal(args.documentId,source.workId);assert.equal(args.revision,edition.revision);return {id:source.workId,versions:[{id:edition.versionId,revision:edition.revision,source}]};},
      async window(args){assert.equal(args.documentId,source.workId);assert.equal(args.versionId,edition.versionId);assert.equal(args.revision,edition.revision);const unit=units.find(value=>value.id===args.unitId);return {documentId:source.workId,versionId:edition.versionId,revision:edition.revision,units:[changed?{...unit,text:unit.text+'changed'}:unit]};},
    };
    const fragments=createFragments({provider,profile:'zarathustra',locale:()=> 'ru'});
    const index={revision:'b'.repeat(64),get:id=>['passage','other'].includes(id)?{id,kind:'passage',record:cite}:null};
    const context={index,provider,fragments},pages=[];
    let cursor;
    do{const page=await readResearchSource(context,{item_id:'passage',limit:2,...(cursor?{cursor}:{})});pages.push(page);cursor=page.nextCursor;}while(cursor);
    const received=pages.flatMap(page=>page.units);
    assert.equal(received.length,13);assert.deepEqual(received.map(unit=>unit.id),units.map(unit=>unit.id));
    assert.equal(pages[0].rangeComplete,false);assert.equal(pages.at(-1).rangeComplete,true);
    assert.equal(received[0].selection.text,Array.from(units[0].text).slice(1).join(''));
    assert.equal(received.at(-1).selection.text,Array.from(units.at(-1).text).slice(0,-1).join(''));
    assert.deepEqual(received[0].exactReference.selector,{type:'text_position',start:1,end:Array.from(units[0].text).length,positionUnit:'unicode_code_point',interval:'half_open'});
    assert.equal(received[0].exactReference.target.textLayerSha256,edition.revision);
    await assert.rejects(readResearchSource(context,{item_id:'other',cursor:pages[0].nextCursor}),/another citation/);
    changed=true;await assert.rejects(readResearchSource(context,{item_id:'passage'}),/witness words changed/);
  });
  it('does not substitute Zarathustra for an unbound work or bibliographic pointer',async()=>{
    let reads=0;
    const fragments=createFragments({provider:{window(){reads++;}},profile:'zarathustra',locale:()=> 'ru'});
    await assert.rejects(fragments.sourceSelection({w:'unknown-book',u:['a','b']}),/No exact source binding/);
    const context={index:{get:()=>({kind:'bibliographic-pointer',record:{w:'ethica',loc:'I.D7'}})},provider:{},fragments};
    await assert.rejects(readResearchSource(context,{item_id:'bibliography'}),/exact passage ID/);
    assert.equal(reads,0);
  });
  it('includes close-reading guides as well as the six atlas modules',()=>{
    check([atlas,routes,guides,foundation,readings,sources,relations],'zarathustra');
  });
  for(const stand of ['zarathustra','antiquity'])it(`${stand}: checks objects and their cited grounds`,()=>{
    check(read(`bonsai/${stand}/objects.json`),stand);
  });
  it('checks the Antiquity collection and its first branches',()=>{
    check([read('bonsai/antiquity/collection.json'),read('bonsai/antiquity/first-branches.json')],'antiquity');
  });
  it('keeps the missing Spinoza scholium in explicitly identified digital witnesses',()=>{
    const data=read('bonsai/zarathustra/fragments.json'),place=data.places.ethica['III.P9.S'];
    assert.equal(place.witness.la,'la-wikisource');assert.equal(place.witness.en,'en-gutenberg');
    for(const language of ['la','en'])assert.ok(place.texts[language]&&data.works.ethica.witnesses[place.witness[language]].url);
  });
});
