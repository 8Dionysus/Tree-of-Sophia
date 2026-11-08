import {readFileSync} from 'node:fs';
import {describe,expect,it} from 'vitest';
import {corpusOnDisk} from '../../../../bonsai/lib/graft.mjs';
import {normalLabel,placeInEdition,sectionPlace,sectionsBefore,zarathustraChapter} from './edition-place.mjs';
import {bindCorpusProvider,CORPUS_OPERATIONS} from './provider.mjs';

// Another edition of a work opens where the reader is, as finely as the Tree aligns the two.
const read=path=>JSON.parse(readFileSync(new URL(`../../../../${path}`,import.meta.url),'utf8'));
const witness=read('bonsai/zarathustra/witnesses.json'),chapter=read('bonsai/zarathustra/witnesses/I.01.json');
const document={id:witness.corpus.documentId};
const titles=chapter.titles;
// Zarathustra's four editions, each with the first chapter's units and a heading the way that edition gives it.
const dta={id:witness.corpus.dta.versionId,units:chapter.units.de.map(unit=>({id:unit[0]})),structure:[{id:'bonsai.de.I.01',label:titles.de,unitId:chapter.units.de[0][0]}]};
const ru={id:witness.corpus.ru.versionId,units:chapter.units.ru.map(unit=>({id:unit[0]})),structure:[{id:'bonsai.ru.I.01',label:titles.ru,unitId:chapter.units.ru[0][0]}]};
const en={id:witness.corpus.en.versionId,units:chapter.units.en.map(unit=>({id:unit[0]})),structure:[{id:'tos.anchor.prologue',label:titles.en,unitId:chapter.units.en[0][0]}]};
const bellenUnits=chapter.units.de.flatMap(unit=>unit[4]??[]);
const bellen={id:witness.corpus.de.versionId,units:bellenUnits.map(id=>({id})),structure:[{id:'tos.anchor.vorrede',label:'Zarathustra’s Vorrede.',unitId:bellenUnits[0]}]};
const unitOf=(lang,pattern)=>chapter.units[lang].find(unit=>pattern.test(unit[2]));
const seht=unitOf('de',/^Seht, ich lehre euch den Übermenschen!/),smotrite=unitOf('ru',/^Смотрите, я учу вас о сверхчеловеке!/);
// A row the alignment pairs one to one in all three witnesses.
const row=chapter.rows.find(item=>item.de?.length===1&&item.ru?.length===1&&item.en?.length===1&&item.de[0]>30);
const [de1,ru1,en1]=[chapter.units.de[row.de[0]],chapter.units.ru[row.ru[0]],chapter.units.en[row.en[0]]];

describe('the place of a reading in another edition',()=>{
  it('compares section labels across editions, apostrophes, case and punctuation aside',()=>{
    expect(normalLabel('Zarathustra’s Vorrede.')).toBe(normalLabel("Zarathustra's Vorrede"));
    expect(normalLabel('ZARATHUSTRA’S PROLOGUE.')).toBe('zarathustras prologue');
    expect(normalLabel('Книга 1 · §1')).toBe('книга 1 1');
  });
  it('carries a Zarathustra unit through the first editions to each other edition',async()=>{
    expect(zarathustraChapter(en,en1[0])).toBe('I.01');
    expect(zarathustraChapter(bellen,seht[4][0])).toBe('I.01');
    expect(await placeInEdition({document,from:dta,to:ru,unitId:seht[0]})).toEqual({unitId:smotrite[0],by:'unit'});
    expect(await placeInEdition({document,from:dta,to:en,unitId:de1[0]})).toEqual({unitId:en1[0],by:'unit'});
    expect(await placeInEdition({document,from:ru,to:en,unitId:ru1[0]})).toEqual({unitId:en1[0],by:'unit'});
    expect(await placeInEdition({document,from:en,to:dta,unitId:en1[0]})).toEqual({unitId:de1[0],by:'unit'});
    // Where a row pairs several units, the edition opens at the row's first unit.
    const wide=chapter.rows.find(item=>item.en?.length>1);
    expect(await placeInEdition({document,from:en,to:dta,unitId:chapter.units.en[wide.en.at(-1)][0]})).toEqual({unitId:chapter.units.de[wide.de[0]][0],by:'unit'});
    expect(await placeInEdition({document,from:dta,to:bellen,unitId:seht[0]})).toEqual({unitId:seht[4][0],by:'unit'});
    expect(await placeInEdition({document,from:bellen,to:ru,unitId:seht[4][0]})).toEqual({unitId:smotrite[0],by:'unit'});
    expect(await placeInEdition({document,from:dta,to:dta,unitId:seht[0]})).toBe(null);
  });
  it('carries a cited unit of an ancient work through its original',async()=>{
    const alignment=read('bonsai/antiquity/alignment.json');
    const [original,entry]=Object.entries(alignment.units).find(([,value])=>Object.keys(value.t).length>=2);
    const [first,second]=Object.entries(entry.t);
    const work={id:alignment.works[entry.w].documentId};
    const version=(id,unit)=>({id,units:[{id:unit},{id:'other'}],structure:[]});
    const greek=version(alignment.works[entry.w].original.versionId,original),a=version(first[0],first[1].u[0]),b=version(second[0],second[1].u[0]);
    expect(await placeInEdition({document:work,from:greek,to:a,unitId:original})).toEqual({unitId:first[1].u[0],by:'unit'});
    expect(await placeInEdition({document:work,from:a,to:greek,unitId:first[1].u[0]})).toEqual({unitId:original,by:'unit'});
    expect(await placeInEdition({document:work,from:a,to:b,unitId:first[1].u[0]})).toEqual({unitId:second[1].u[0],by:'unit'});
  });
  it('else opens the section both editions label alike, across languages for Zarathustra\'s chapters',async()=>{
    const units=(prefix,count)=>Array.from({length:count},(_,index)=>({id:`${prefix}${index}`}));
    const lamb={id:'lamb',units:units('a',9),structure:[{label:'70',unitId:'a0'},{label:'71',unitId:'a5'}]};
    const burnet={id:'burnet',units:units('b',6),structure:[{label:'70',unitId:'b0'},{label:'71',unitId:'b3'}]};
    expect(sectionPlace(lamb,burnet,'a7')).toBe('b3');
    expect(await placeInEdition({document:{id:'meno'},from:lamb,to:burnet,unitId:'a2'})).toEqual({unitId:'b0',by:'section'});
    expect(await placeInEdition({document:{id:'meno'},from:lamb,to:{id:'x',units:units('c',3),structure:[{label:'Prologue',unitId:'c0'}]},unitId:'a2'})).toBe(null);
    // An edition the alignment does not reach still finds its chapter by title.
    const other={id:'another-ru',units:[{id:'r0'},{id:'r1'}],structure:[{label:'Предисловие Заратустры',unitId:'r0'}]};
    expect(await placeInEdition({document,from:bellen,to:other,unitId:seht[4][0]})).toEqual({unitId:'r0',by:'section'});
  });
  it.skipIf(corpusOnDisk(new URL('../../../../',import.meta.url).pathname)!=='present')('finds the places in the published editions',async()=>{
    const corpus='access/web/public/public-corpus/',manifest=read(`${corpus}manifest.json`);
    const open=id=>read(`${corpus}${manifest.documents.find(item=>item.id===id).manifest.path}`);
    const zarathustra=open(document.id),version=id=>zarathustra.versions.find(item=>item.id===id);
    for(const [to,expected] of [[witness.corpus.ru.versionId,ru1[0]],[witness.corpus.en.versionId,en1[0]],[witness.corpus.de.versionId,de1[4][0]]])
      expect(await placeInEdition({document:zarathustra,from:version(dta.id),to:version(to),unitId:de1[0]})).toEqual({unitId:expected,by:'unit'});
    const meno=open('tos.work.plato.meno'),lambMeno=meno.versions.find(item=>item.id==='perseus-lamb-1924-en'),burnetMeno=meno.versions.find(item=>item.id==='perseus-burnet-1903-grc');
    const at71=lambMeno.structure.find(entry=>entry.label==='71');
    const place=await placeInEdition({document:meno,from:lambMeno,to:burnetMeno,unitId:at71.unitId});
    // A cited unit lands on its aligned unit, any other on the section's start: either way in Stephanus page 71.
    expect(sectionsBefore(burnetMeno,place?.unitId)[0]?.label).toBe('71');
  });
  it('is a provider operation the reader asks, bounded at the provider\'s edge',async()=>{
    const base=Object.fromEntries(CORPUS_OPERATIONS.map(name=>[name,async()=>null]));
    const asked=[];
    const provider=bindCorpusProvider({...base,place:async args=>{asked.push(args);return args.unitId==='u1'?{unitId:'t1',by:'section',extra:true}:args.unitId==='bad'?{unitId:'',by:'unit'}:null;}});
    expect(provider.capabilities.place).toBe(true);
    expect(await provider.place({documentId:'d',versionId:'a',targetVersionId:'b',unitId:'u1'})).toEqual({unitId:'t1',by:'section'});
    expect(await provider.place({documentId:'d',versionId:'a',targetVersionId:'b',unitId:'u2'})).toBe(null);
    await expect(provider.place({documentId:'d',versionId:'a',targetVersionId:'b',unitId:'bad'})).rejects.toThrow(/place must be null/);
    await expect(provider.place({documentId:'d',versionId:'a',unitId:'u1'})).rejects.toThrow(/targetVersionId/);
    expect(asked[0]).toMatchObject({documentId:'d',versionId:'a',targetVersionId:'b',unitId:'u1'});
    expect(bindCorpusProvider(base).capabilities?.place).toBeUndefined();
  });
});
