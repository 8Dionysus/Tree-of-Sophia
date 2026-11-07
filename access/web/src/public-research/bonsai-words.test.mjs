import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {describe,expect,it} from 'vitest';
import {layerStatus} from '../../../../bonsai/lib/layers.mjs';
import {spellingIssues,termFormGaps,tokenChapterIssues,unitWords,unitsDigest,wordsAt,sourceForm} from '../../../../bonsai/lib/tokens.mjs';
import {isSingleWord} from '../corpus-reader/selection-menu.mjs';
import {shardOf} from '../corpus-reader/word-origin.mjs';
import {grammarLabel,relationLabel} from '../corpus-reader/word-context.mjs';

// The word layer reads every word of a witness at its exact place; the origin of a word is gathered by lemma.
const sha256=text=>createHash('sha256').update(text).digest('hex');
const read=path=>JSON.parse(readFileSync(new URL(`../../../../bonsai/zarathustra/${path}`,import.meta.url),'utf8'));
// The stand's own layer lives in R2 (bonsai/zarathustra/layers.json): its cases run where a checkout fetched it.
const fetched=layerStatus(new URL('../../../../bonsai/zarathustra',import.meta.url).pathname).every(entry=>entry.state==='present');
const witness={chapter:'X.01',units:{de:[['u1','p','Ich lehre euch den Über¬\nmenschen. Gieng er?']]}};
const layer=()=>({schema:'tos.bonsai.tokens.chapter.v1',chapter:'X.01',
  entries:[['ich','PRON','Case=Nom|Number=Sing|Person=1'],['lehren','VERB','Mood=Ind|Tense=Pres'],['euch','PRON','Case=Dat'],
    ['der','DET','Case=Acc'],['Übermensch','NOUN','Case=Acc|Gender=Masc|Number=Sing'],['.','PUNCT',''],['gehen','VERB','Tense=Past'],['er','PRON','Case=Nom'],['?','PUNCT','']],
  witnesses:{de:{digest:unitsDigest(witness.units.de,sha256),units:{u1:[
    [0,3,0,1,'nsubj','a'],[4,9,1,-1,'root','a'],[10,14,2,1,'iobj','a'],[15,18,3,4,'det','a'],[19,33,4,1,'obj','a','Übermenschen'],[33,34,5,1,'punct','m'],
    [35,40,6,-1,'root','l','ging'],[41,43,7,6,'nsubj','a'],[43,44,8,6,'punct','m']]}}}});

describe('the word layer',()=>{
  it('reads each word at its exact place, a line-end break read whole',()=>{
    const words=unitWords(layer(),'de','u1',witness.units.de[0][2]);
    expect(words[4]).toMatchObject({lemma:'Übermensch',upos:'NOUN',text:'Über¬\nmenschen',reading:'Übermenschen',head:1,deprel:'obj'});
    expect(sourceForm(words[4].text)).toBe('Übermenschen');
    expect(words[6]).toMatchObject({lemma:'gehen',basis:'l',reading:'ging'});
  });
  it('finds the words a selection covers, punctuation aside',()=>{
    expect(wordsAt(layer(),'de','u1',witness.units.de[0][2],20,25).map(word=>word.lemma)).toEqual(['Übermensch']);
    expect(wordsAt(layer(),'de','u1',witness.units.de[0][2],33,34)).toEqual([]);
  });
  it('fails a layer whose witness text changed and a row outside its unit',()=>{
    expect(tokenChapterIssues(layer(),witness,sha256)).toEqual([]);
    const changed={...witness,units:{de:[['u1','p','Ich lehre euch.']]}};
    expect(tokenChapterIssues(layer(),changed,sha256)[0]).toMatch(/witness changed/);
    const broken=layer();broken.witnesses.de.units.u1[0]=[0,99,0,1,'nsubj','a'];
    expect(tokenChapterIssues(broken,witness,sha256).join()).toMatch(/span 0-99/);
  });
  it('reports the forms of a term the layer finds under its lemma',()=>{
    const terms={terms:[{id:'overman',original:{lang:'de',lemma:'Übermensch',forms:['Übermensch']}}]};
    expect(termFormGaps(terms,{'X.01':layer()},{'X.01':witness})).toEqual({overman:{'Übermenschen':1}});
  });
  it('checks the spelling module and its grounds',()=>{
    expect(spellingIssues({schema:'tos.bonsai.spelling.v1',rules:{th:'th → t'},forms:{Thier:{modern:'Tier',basis:'wiktionary',count:3},Thiere:{modern:'Tiere',basis:'rules',rules:['th'],count:1}}})).toEqual([]);
    expect(spellingIssues({schema:'tos.bonsai.spelling.v1',rules:{},forms:{Thiere:{modern:'Tiere',basis:'rules',rules:['th'],count:1}}})).toEqual(['Thiere: unknown rule']);
  });
  it.skipIf(!fetched)('binds every chapter of the stand to its witnesses',()=>{
    const manifest=read('tokens.json');
    expect(manifest.schema).toBe('tos.bonsai.tokens.v1');
    for(const key of ['I.01','III.02']){
      const chapter=read(`witnesses/${key}.json`);
      expect(tokenChapterIssues(read(`tokens/${key}.json`),chapter,sha256)).toEqual([]);
    }
    // gieng is read as ging, and its lemma is gehen
    const prologue=read('tokens/I.01.json'),text=read('witnesses/I.01.json').units.de;
    const gieng=text.flatMap(unit=>unitWords(prologue,'de',unit[0],unit[2])).find(word=>word.text==='gieng');
    expect(gieng).toMatchObject({lemma:'gehen',reading:'ging'});
    expect(read('spelling.json').forms.Thier).toMatchObject({modern:'Tier',basis:'wiktionary'});
  });
});

describe('the origin of a word',()=>{
  it('finds a lemma\'s shard as origins.py writes it',()=>{
    expect(shardOf('Übermensch')).toBe(4188479793%32);
    expect(shardOf('сверхчеловек')).toBe(644943858%32);
  });
  it.skipIf(!fetched)('holds a word\'s descent, oldest form first, and its first editions\' spelling',()=>{
    const shard=(language,lemma)=>read(`origins/${language}-${String(shardOf(lemma)).padStart(2,'0')}.json`).lemmas[lemma];
    const tier=shard('de','Tier');
    expect(tier.origin[0].chain[0]).toEqual(['inherited','ine-pro','*dʰewsóm']);
    expect(tier.origin[0].chain.at(-1)).toEqual(['inherited','gmh','tier']);
    expect(tier.spelling).toContain('Thier');
    expect(shard('de','Übermensch').origin[0].parts).toEqual(['über-','Mensch']);
    // Antonovsky's word is a calque of the original's, and renders it
    const word=shard('ru','сверхчеловек');
    expect(word.origin[0].chain[0]).toEqual(['calque','de','Übermensch']);
    expect(word.renders[0][0]).toBe('Übermensch');
  });
  it('offers its menu for one word only',()=>{
    expect(isSingleWord('Übermenschen')).toBe(true);
    expect(isSingleWord('Über¬\nmenschen')).toBe(true);
    expect(isSingleWord('den Übermenschen')).toBe(false);
    expect(isSingleWord('')).toBe(false);
  });
  it('names grammar in the reader\'s language',()=>{
    expect(grammarLabel('NOUN','Case=Acc|Number=Sing','ru')).toBe('существительное · винительный падеж, единственное число');
    expect(grammarLabel('VERB','Tense=Past','en')).toBe('verb · past');
    expect(relationLabel('obj','ru')).toBe('прямое дополнение');
  });
});
