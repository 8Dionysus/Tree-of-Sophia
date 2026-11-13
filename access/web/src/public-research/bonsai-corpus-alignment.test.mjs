import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {alignCorpus,alignVersion,citedUnits,corpusReader,formatAlignment,textEnd} from '../../../../bonsai/lib/corpus-align.mjs';
import {corpusAlignmentIssues,translationScopesIssues} from '../../../../bonsai/lib/fragments.mjs';
import * as atlasContent from '../../constructor/atlas-content.mjs';
import * as atlasRoutes from '../../constructor/atlas-routes.mjs';
import * as relationInquiry from '../../constructor/relation-inquiry.mjs';
import * as sourcesConcepts from '../../constructor/inquiry-sources-concepts.mjs';
import * as readingsEchoes from '../../constructor/inquiry-readings-echoes.mjs';
import * as foundation from '../../constructor/inquiry-foundation.mjs';
import * as atlasData from '../../constructor/atlas-data.mjs';
import * as sourceReferences from '../../constructor/source-references.mjs';
import * as closeReading from '../../constructor/close-reading-guides.mjs';
import {corpusOnDisk,loadBonsai} from '../../../../bonsai/lib/graft.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const read=corpusReader(join(root,'access','web','public','public-corpus'));
// The corpus snapshot's pages live in R2 (access/public-corpus/layers.json); the cases reading them run where a checkout fetched them.
const corpusFetched=corpusOnDisk(root)==='present';
const json=path=>JSON.parse(readFileSync(join(root,path),'utf8'));
// What the stand cites, as the bonsai's own mechanism reads it: the modules it borrows and its own modules.
function zarathustraContent(){
  const {manifest,modules}=loadBonsai(root,'zarathustra');
  const borrowed=[atlasContent,atlasData,atlasRoutes,relationInquiry,sourcesConcepts,readingsEchoes,foundation,sourceReferences,closeReading].map(module=>Object.values(module));
  return [...borrowed,...(manifest.modules??[]).filter(item=>!item.borrowed&&item.id!=='alignment'&&modules[item.id]).map(item=>modules[item.id])];
}
const worksOf=stand=>Object.fromEntries(Object.entries(json(`bonsai/${stand}/identities.json`).works).map(([key,value])=>[key,value.tree]));

describe('the corpus alignment of the stands',()=>{
  it('keeps the edition notes out of the text a unit is paired by',()=>{
    expect(textEnd('Text.\n〔1〕 A note.')).toBe(5);
    expect(textEnd('Text without notes.')).toBe(19);
  });
  it('does not attach an absent named tractate to the preceding translated tractate',()=>{
    const original={language:'grc',units:Array.from({length:6},(_,i)=>({id:`g${i}`,reference:`Книга ${i+1}`,text:'λόγος.'}))};
    const translation={language:'en',units:original.units.slice(0,5).map((unit,i)=>({id:`e${i}`,reference:unit.reference,text:'Word.'}))};
    original.units.push({id:'continuation',reference:'Unit 7',text:'λόγος.'});
    const result=alignVersion(original,translation,new Set(['g4','g5','continuation']));
    expect(result.entries.g4.u).toContain('e4');
    expect(result.entries.g5).toBeUndefined();
    expect(result.entries.continuation).toBeUndefined();
  });
  it('limits a partial translation to the declared original range and rejects stale scope',()=>{
    const original={id:'latin',language:'la',revision:'rev',units:Array.from({length:6},(_,i)=>({id:`u${i}`,reference:`§${i+1}`,text:'Verba.'}))};
    const translation={id:'partial',language:'en',revision:'trans-rev',units:original.units.slice(0,5).map((unit,i)=>({id:`e${i}`,reference:unit.reference,text:'Words.'}))};
    const read=()=>({id:'work',versions:[original,translation]});
    const cited=new Map([['a',{original:'latin',units:new Set(['u4','u5'])}]]),works={a:'work'};
    const scopes=[{work:'a',translationVersionId:'partial',translationRevision:'trans-rev',originalVersionId:'latin',originalRevision:'rev',firstUnitId:'u0',lastUnitId:'u4',source:'https://example.test/book'}];
    const result=alignCorpus(cited,works,read,scopes);
    expect(result.units.u4.t.partial).toBeDefined();
    expect(result.units.u5).toBeUndefined();
    expect(result.receipt['a:partial'].sourceScope.lastUnitId).toBe('u4');
    expect(()=>alignCorpus(cited,works,read,[{...scopes[0],originalRevision:'stale'}])).toThrow(/invalid or stale translation scope/);
    expect(()=>alignCorpus(cited,works,read,[{...scopes[0],translationRevision:'stale'}])).toThrow(/invalid or stale translation scope/);
    expect(()=>alignCorpus(cited,works,()=>({id:'work',versions:[original,{...translation,revision:undefined}]}),[{...scopes[0],translationRevision:undefined}])).toThrow(/invalid or stale translation scope/);
  });
  it('checks the stored scope schema and rejects duplicate or incomplete bindings',()=>{
    const module=json('bonsai/antiquity/translation-scopes.json');
    expect(translationScopesIssues(module)).toEqual([]);
    expect(translationScopesIssues({...module,scopes:[...module.scopes,module.scopes[0]]})).toContain(`duplicate academica:${module.scopes[0].translationVersionId}`);
    expect(translationScopesIssues({...module,scopes:[{...module.scopes[0],originalRevision:'stale'}]})).toContain('originalRevision');
    expect(translationScopesIssues({...module,scopes:[{...module.scopes[0],translationRevision:'stale'}]})).toContain('translationRevision');
  });
  it('can align a sentence across a translation reference inside a sentence',()=>{
    const original={language:'grc',units:[
      {id:'g1',reference:'189',text:'Τί λέγεις;'},
      {id:'g2',reference:'190',text:'λόγον ὃν αὐτὴ πρὸς αὑτὴν ἡ ψυχὴ διεξέρχεται περὶ ὧν ἂν σκοπῇ. ἄλλο τι;'},
    ]};
    const translation={language:'ru',units:[
      {id:'r1',reference:'189',text:'Что ты говоришь? Беседу, которую душа ведет сама с собою, когда что-нибудь рассматривает. Вот как она размышляя разговаривает, сама себя '},
      {id:'r2',reference:'190',text:'спрашивает и отвечает. Дальше идёт разговор.'},
    ]};
    const entry=alignVersion(original,translation,new Set(['g2'])).entries.g2;
    const first=entry.pairs.find(([source])=>source.some(([start,end])=>start===0&&end>0));
    const text=first[1].map(([i,start,end])=>Array.from(translation.units.find(unit=>unit.id===entry.u[i]).text).slice(start,end).join('')).join(' ');
    expect(entry.u).toContain('r1');
    expect(text).toContain('Беседу, которую душа ведет сама с собою');
    expect(text).toContain('спрашивает и отвечает.');
    translation.units[0].text='Предыдущая мысль завершена.';
    translation.units[1].text='Новая мысль начинается здесь.';
    expect(alignVersion(original,translation,new Set(['g2'])).entries.g2.u).not.toContain('r1');
  });
  it('leaves a finished sentence of the preceding unit with the place that says it',()=>{
    // Theaetetus 189e–190a: the Greek reference falls before τοῦτο γάρ, the Russian one inside the next sentence.
    const original={language:'grc',units:[
      {id:'g1',reference:'189',text:'Τί λέγεις; λόγον ὃν αὐτὴ πρὸς αὑτὴν ἡ ψυχὴ διεξέρχεται περὶ ὧν ἂν σκοπῇ.'},
      {id:'g2',reference:'190',text:'τοῦτο γάρ μοι ἰνδάλλεται διανοουμένη, αὐτὴ ἑαυτὴν ἐρωτῶσα καὶ ἀποκρινομένη. ἄλλο τι;'},
    ]};
    const translation={language:'ru',units:[
      {id:'r1',reference:'189',text:'Что ты говоришь? Беседу, которую душа ведет сама с собою, когда что-нибудь рассматривает. Вот как она размышляя разговаривает, сама себя '},
      {id:'r2',reference:'190',text:'спрашивает и отвечает. Дальше идёт разговор.'},
    ]};
    const entry=alignVersion(original,translation,new Set(['g2'])).entries.g2;
    const said=entry.pairs.flatMap(([,targets])=>targets).map(([i,start,end])=>Array.from(translation.units.find(unit=>unit.id===entry.u[i]).text).slice(start,end).join('')).join(' ');
    expect(said).toContain('Вот как она размышляя разговаривает, сама себя');
    expect(said).toContain('спрашивает и отвечает.');
    expect(said).not.toContain('Беседу');
    // An edition's notes after the preceding unit's text do not make its finished text look cut.
    translation.units[0].text='Что ты говоришь? Беседу, которую душа ведет сама с собою, когда что-нибудь рассматривает.\n〔1〕 Примечание без точки';
    translation.units[1].text='вот как она размышляя разговаривает, сама себя спрашивает и отвечает. Дальше идёт разговор.';
    expect(alignVersion(original,translation,new Set(['g2'])).entries.g2.u).not.toContain('r1');
  });
  it.skipIf(!corpusFetched)('is, for Zarathustra, what the mechanism gives from the stand\'s own citations',()=>{
    const content=zarathustraContent();
    const works=worksOf('zarathustra'),stored=readFileSync(join(root,'bonsai','zarathustra','alignment.json'),'utf8');
    expect(formatAlignment(alignCorpus(citedUnits(content,works,read),works,read))).toBe(stored);
  },120000);
  for(const stand of ['zarathustra','antiquity'])it.skipIf(!corpusFetched)(`${stand}: holds every cited unit except places outside all translation scopes`,()=>{
    const module=json(`bonsai/${stand}/alignment.json`),works=worksOf(stand);
    const scopes=stand==='antiquity'?json('bonsai/antiquity/translation-scopes.json').scopes:[];
    expect(corpusAlignmentIssues(module)).toEqual([]);
    const content=stand==='antiquity'?[json('bonsai/antiquity/collection.json'),json('bonsai/antiquity/first-branches.json')]:zarathustraContent();
    for(const [key,{units}] of citedUnits(content,works,read)){
      const work=module.works[key];
      if(!work?.translations?.length)continue;
      const original=read(works[key]).versions.find(v=>v.id===work.original.versionId);
      for(const unit of units){
        if(module.units[unit]){expect(module.units[unit].w,`${stand} ${key} ${unit}`).toBe(key);continue;}
        const index=original.units.findIndex(value=>value.id===unit);
        expect(work.translations.every(version=>{
          const scope=scopes.find(item=>item.work===key&&item.translationVersionId===version.versionId);
          if(!scope)return false;
          const first=original.units.findIndex(value=>value.id===scope.firstUnitId),last=original.units.findIndex(value=>value.id===scope.lastUnitId);
          return index<first||index>last;
        }),`${stand} ${key} ${unit}: a missing mapping must lie outside every translation's scope`).toBe(true);
      }
    }
  },120000);
});
