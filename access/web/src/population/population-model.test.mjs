import {describe,expect,it} from 'vitest';
import population from '../../../../bonsai/population/population.json' with {type:'json'};
import chronology from '../../../../bonsai/population/chronology.json' with {type:'json'};
import {centuryOf,centuryText,displayName,filterMembers,findMember,fold,populationModel,rowsByBatch,sortMembers,spanOf,spanText,tally,yearOf,yearText} from './population-model.mjs';

const model=populationModel(population,chronology);
const named=name=>model.members.find(entry=>entry.member.name.ru===name);

describe('the Population stand reads its bonsai',()=>{
  it('writes years and centuries as readers do, in each language of the site',()=>{
    expect(yearOf('-0427')).toBe(-427);expect(yearOf('1646-07-01')).toBe(1646);expect(yearOf(undefined)).toBe(null);
    expect(yearText(-427,'ru')).toBe('428 до н. э.');expect(yearText(-427,'zh-Hans')).toBe('公元前428');expect(yearText(1275,'de')).toBe('1275');
    expect(centuryOf(1275)).toBe(13);expect(centuryOf(1300)).toBe(13);expect(centuryOf(1301)).toBe(14);
    expect(centuryOf(-427)).toBe(-5);expect(centuryOf(0)).toBe(-1);expect(centuryOf(-100)).toBe(-2);expect(centuryOf(null)).toBe(null);
    expect(centuryText(13,'ru')).toBe('XIII в.');expect(centuryText(-5,'ru')).toBe('V в. до н. э.');
    expect(centuryText(21,'en')).toBe('21st c.');expect(centuryText(-12,'en')).toBe('12th c. BCE');
    expect(centuryText(4,'es')).toBe('s. IV');expect(centuryText(-1,'de')).toBe('1. Jh. v. Chr.');expect(centuryText(9,'zh-Hans')).toBe('9世纪');
  });
  it('dates a span with its certainty, open bounds and the activity it may be',()=>{
    expect(spanText({from:-427,to:-347,certainty:'approximate',activity:false},'ru')).toBe('ок. 428–348 до н. э.');
    expect(spanText({from:-3,to:65,certainty:'exact',activity:false},'en')).toBe('4 BCE–65');
    expect(spanText({from:1275,to:1325,certainty:'approximate',activity:true},'en')).toBe('fl. c. 1275–1325');
    expect(spanText({from:2022,to:2022,certainty:'exact',activity:true},'de')).toBe('wirkte 2022');
    expect(spanText({from:1260,to:null,certainty:'approximate',activity:false},'ru')).toBe('род. ок. 1260');
    expect(spanText({from:null,to:1810,certainty:'exact',activity:false},'es')).toBe('m. 1810');
    expect(spanText({from:1904,to:1941,certainty:'uncertain',activity:false},'en')).toBe('1904–1941 (?)');
    expect(spanText(null,'zh-Hans')).toBe('生卒年不详');
    expect(spanOf({id:'x.lifetime',dating:{kind:'unknown-date'}})).toBe(null);
    expect(spanOf({id:'x.activity',dating:{kind:'date-assertion',value:'0300',certainty:'approximate'}})).toEqual({from:300,to:300,certainty:'approximate',activity:true});
  });
  it('gives every planted member a place, its life from the chronology and its names to be found by',()=>{
    expect(model.members).toHaveLength(Object.keys(population.members).length);
    const aristotle=named('Аристотель');
    expect(aristotle.member.grew_from.length).toBeGreaterThan(30);
    expect(aristotle.century).toBe(-4);
    expect(spanText(aristotle.span,'ru')).toMatch(/до н\. э\.$/);
    expect(fold('Ḥunayn ibn Isḥāq')).toBe('hunayn ibn ishaq');expect(fold('Ёлкин')).toBe('елкин');
    expect(filterMembers(model,{query:'aristot'}).map(entry=>entry.id)).toContain(aristotle.id);
    expect(filterMembers(model,{query:'аристотел'}).map(entry=>entry.id)).toContain(aristotle.id);
    expect(filterMembers(model,{century:-4}).every(entry=>entry.century===-4)).toBe(true);
    expect(filterMembers(model,{kind:'pseudonymous'}).every(entry=>entry.member.kind==='pseudonymous')).toBe(true);
    expect(displayName(aristotle.member,'en')).toBe(aristotle.member.name.en);
  });
  it('leads a merged ID to the member it joined',()=>{
    const [from,to]=Object.entries(population.redirects)[0];
    expect(findMember(model,from)?.id).toBe(to);
    expect(findMember(model,'m-aaaaaaaaaa')).toBe(null);
  });
  it('orders by name or by time, the undated last, and counts what the filters offer',()=>{
    const byTime=sortMembers(model.members,{by:'time',locale:'en'});
    const dated=byTime.filter(entry=>entry.anchor!==null);
    expect(dated.every((entry,at)=>at===0||dated[at-1].anchor<=entry.anchor)).toBe(true);
    expect(byTime.slice(dated.length).every(entry=>entry.anchor===null)).toBe(true);
    const {centuries,kinds}=tally(model.members);
    expect([...kinds.values()].reduce((sum,count)=>sum+count,0)).toBe(model.members.length);
    expect([...centuries.values()].reduce((sum,count)=>sum+count,0)).toBe(dated.length);
  });
  it('shows the register rows a member grew from with the register each batch cites',()=>{
    const batches=rowsByBatch(named('Аристотель').member,population.batches);
    expect(batches.length).toBeGreaterThan(20);
    for(const {batch,rows,register} of batches){
      expect(rows.every(row=>row.startsWith(`${batch}-A`))).toBe(true);
      expect(register).toMatch(/^https:\/\/treeofsophia\.com\/__research\/layers\/sha256\/[0-9a-f]{64}$/);
    }
  });
});
