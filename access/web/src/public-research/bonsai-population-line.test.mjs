import {existsSync,readFileSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {semanticRegistry} from '../../../../bonsai/lib/objects.mjs';
import {populationIssues} from '../../../../bonsai/lib/population.mjs';
import {agreement,itemId,verdictProblems} from '../../../../bonsai/review/population/line/kit.mjs';
import {plant} from '../../../../bonsai/review/population/plant.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const review=join(root,'bonsai','review','population');
const layersFetched=existsSync(join(review,'layer','agents'));
const jsonl=path=>readFileSync(path,'utf8').split('\n').filter(Boolean).map(line=>JSON.parse(line));

// A member's question as kit.mjs cuts it, with one row and its fields.
const question=(type,fields={life_dates:'рождение; ок. 1275; смерть; ок. 1344',label_ru:'Уолтер Бёрли'})=>({item:'q-x',type,sha:'s',member:'m-aaaaaaaaaa',
  ...(type==='kind'?{row:'T2-26:T2-26-A006',signals:['conventional?']}:{}),rows:[{row:'T2-26:T2-26-A006',kind:'person',fields}]});
const quote={row:'T2-26:T2-26-A006',field:'life_dates',text:'ок. 1275'};
const answer=(fields)=>({item:'q-x',sha:'s',note:'what decided it',quotes:[quote],basis:['register'],...fields});

describe('the Population line reads a member\'s questions',()=>{
  it('names a member\'s question by its type and what it is about',()=>{
    expect(itemId('dates','m-aaaaaaaaaa')).toMatch(/^q-[a-z2-7]{12}$/);
    expect(itemId('dates','m-aaaaaaaaaa')).not.toBe(itemId('latin','m-aaaaaaaaaa'));
  });
  it('takes a kind the row\'s words give, and two readers agree on the same kind',()=>{
    expect(verdictProblems(question('kind'),answer({verdict:'conventional'}))).toEqual([]);
    expect(verdictProblems(question('kind'),answer({verdict:'saint'}))[0]).toMatch(/is not a kind/);
    expect(verdictProblems(question('kind'),answer({verdict:'conventional',quotes:[]}))).toContain('no quote of the rows');
    expect(agreement(question('kind'),{verdict:'conventional'},{verdict:'conventional'})).toMatchObject({agreed:true,value:{kind:'conventional'}});
    expect(agreement(question('kind'),{verdict:'conventional'},{verdict:'person'}).agreed).toBe(false);
  });
  it('takes a name in Latin script only, and keeps the form with more of its marks',()=>{
    expect(verdictProblems(question('latin'),answer({verdict:'latin',latin:'Huìyuǎn',basis:['name']}))).toEqual([]);
    expect(verdictProblems(question('latin'),answer({verdict:'latin',latin:'Хуэйюань',basis:['name']}))).toContain('latin must be the name in Latin script');
    expect(verdictProblems(question('latin'),answer({verdict:'latin',latin:'Huiyuan',basis:['name'],quotes:[]}))).toContain('a Latin name needs a quote of the rows or a source you opened');
    expect(verdictProblems(question('latin'),answer({verdict:'latin',latin:'Huiyuan',basis:['text'],quotes:[],sources:['https://example.org/huiyuan']}))).toEqual([]);
    expect(agreement(question('latin'),{verdict:'latin',latin:'Huiyuan'},{verdict:'latin',latin:'Huìyuǎn'})).toMatchObject({agreed:true,value:{latin:'Huìyuǎn'}});
    expect(agreement(question('latin'),{verdict:'latin',latin:'Huiyuan'},{verdict:'latin',latin:'Hui-neng'}).agreed).toBe(false);
  });
  it('takes years as eras and certainties, refuses what no life can be, and settles two close answers',()=>{
    const years=(birth,death,more={})=>answer({verdict:'dates',birth,death,activity:null,...more});
    const at=(year,era='CE',certainty='exact')=>({year,era,certainty});
    expect(verdictProblems(question('dates'),years(at(1275,'CE','approximate'),at(1344,'CE','approximate')))).toEqual([]);
    expect(verdictProblems(question('dates'),years(null,null))).toContain('the years give nothing: name a birth, a death, years of activity, or that the person lives');
    expect(verdictProblems(question('dates'),years(at(1344),at(1275)))).toContain('the birth comes after the death');
    expect(verdictProblems(question('dates'),years(at(428,'BCE'),at(348,'BCE')))).toEqual([]);
    expect(verdictProblems(question('dates'),years(at(348,'BCE'),at(428,'BCE')))).toContain('the birth comes after the death');
    expect(verdictProblems(question('dates'),years({year:1275},null))[0]).toMatch(/^birth must be/);
    expect(verdictProblems(question('living'),years(at(1931),at(2020),{living:true}))).toContain('a living person has no death');
    expect(verdictProblems(question('living'),years(at(1850),null,{living:true}))).toContain('a living person born before 1880');
    expect(verdictProblems(question('dates'),years(at(1275),null,{quotes:[]}))).toContain('years need a quote of the rows or a source you opened');
    const one={verdict:'dates',birth:at(428,'BCE','approximate'),death:at(348,'BCE'),activity:null};
    const two={verdict:'dates',birth:at(427,'BCE','exact'),death:at(348,'BCE'),activity:null};
    const settled=agreement(question('dates'),one,two);
    expect(settled).toMatchObject({agreed:true,value:{birth:{year:428,era:'BCE',certainty:'approximate'},death:{year:348,era:'BCE',certainty:'exact'},living:false}});
    expect(agreement(question('dates'),one,{...two,death:null}).agreed).toBe(false);
    expect(agreement(question('dates'),one,{...two,birth:at(380,'BCE')}).agreed).toBe(false);
    expect(agreement(question('living'),{verdict:'undecidable'},{verdict:'undecidable'})).toMatchObject({agreed:true,verdict:'undecidable'});
  });
  it('keeps the identity rules as they were',()=>{
    const pair={item:'q-x',type:'identity',sha:'s',a:{member:'m-a',rows:[{row:'A01:A01-A001',fields:{label_ru:'Платон'}}]},b:{member:'m-b',rows:[{row:'A02:A02-A001',fields:{label_ru:'Платон'}}]}};
    const same={item:'q-x',sha:'s',verdict:'same',basis:['name'],quotes:[{side:'a',row:'A01:A01-A001',field:'label_ru',text:'Платон'}],note:'n'};
    expect(verdictProblems(pair,same)).toContain('same needs a basis beyond the name');
    expect(agreement(pair,{verdict:'different'},{verdict:'related',relation:'namesake_of'})).toMatchObject({agreed:true,verdict:'different'});
  });
});

describe.skipIf(!layersFetched)('the Population bonsai plants what two readers decided',()=>{
  const registry=semanticRegistry(root);
  const dates=jsonl(join(review,'queue','dates.jsonl')).find(entry=>entry.reason==='life:unchecked');
  const latin=jsonl(join(review,'queue','latin.jsonl')).find(entry=>entry.member!==dates.member);
  const kind=jsonl(join(review,'queue','kind.jsonl')).find(entry=>entry.member!==dates.member&&entry.member!==latin.member);
  const decisions=[
    {id:'d-lifeaaaaaa',type:'life',question:'dates',rows:dates.rows,item:dates.item,by:'test',birth:{year:428,era:'BCE',certainty:'approximate'},death:{year:348,era:'BCE',certainty:'exact'},activity:null,living:false,
      sources:['https://plato.stanford.edu/entries/plato/'],readers:[{verdict:'dates',basis:['text'],note:'The SEP article dates the life.'}]},
    {id:'d-nameaaaaaa',type:'name',question:'latin',rows:latin.rows,item:latin.item,by:'test',latin:'Testname Latinus',readers:[{verdict:'latin',basis:['name'],note:'A transliteration.'}]},
    {id:'d-kindaaaaaa',type:'kind',question:'kind',rows:[kind.row],item:kind.item,by:'test',kind:'conventional',readers:[{verdict:'conventional',basis:['register'],note:'The row says so.'}]},
  ];
  const decisionsText=[...readFileSync(join(review,'decisions.jsonl'),'utf8').split('\n').filter(Boolean),...decisions.map(decision=>JSON.stringify(decision))].join('\n')+'\n';
  const {files}=plant({files:{'decisions.jsonl':decisionsText}});
  const population=JSON.parse(files['population.json']),chronology=JSON.parse(files['chronology.json']),identities=JSON.parse(files['identities.json']);

  it('dates a member by two readers\' years and their source, in place of the registers\' gap',()=>{
    const member=population.members[dates.member];
    expect(member.gaps.filter(gap=>gap.startsWith('life:'))).toEqual([]);
    const life=chronology.situations[chronology.lives[dates.member]];
    expect(life.dating).toMatchObject({kind:'interval-assertion',certainty:'approximate',interval:{start:'-0427',end:'-0347'}});
    expect(life).toMatchObject({footing:'medium',epistemic:'reported',sources:['line-d-lifeaaaaaa']});
    expect(chronology.sources['line-d-lifeaaaaaa'].url).toBe('https://plato.stanford.edu/entries/plato/');
    expect(life.statement.ru).toContain('рождение — ок. 428 до н. э.');
    expect(life.statement.en).toContain('died 348 BCE');
  });
  it('names a member in Latin script and types a row as two readers decided',()=>{
    expect(population.members[latin.member].name.en).toBe('Testname Latinus');
    expect(population.members[latin.member].gaps).not.toContain('name:latin');
    expect(population.members[kind.member].kind).toBe('conventional');
  });
  it('keeps the module\'s contract with the decisions planted',()=>{
    expect(populationIssues(population,{chronology,identities,registry})).toEqual([]);
  });
});
