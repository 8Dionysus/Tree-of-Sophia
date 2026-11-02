import {readFileSync,readdirSync} from 'node:fs';
import {describe,expect,it} from 'vitest';

// The Antiquity stand's own bilingual forms, excluding quoted witness texts.
const root=new URL('../../../../bonsai/antiquity/',import.meta.url);
const files=readdirSync(root).filter(name=>name.endsWith('.json')&&name!=='alignment.json');
const outside=[
  ['a padding turn built on not only or not merely',/не только.{0,100}но и|not only.{0,100}but (?:also )?|не просто|not merely/i],
  ['a contrast expressed through non-identity',/не тождествен/i],
  ['a padding turn using rather than',/\brather than\b/i],
  ['padding about a completed result',/готов(?:ое|ым) опровержени|завершённый итог|ready-made refutation|completed result/i],
  ['a contrast with a universal theory or template',/а не единую теорию искусства|а не шаблон для всех сюжетов|not one theory of art|not a template for every plot/i],
  ['a contrast with an exhaustive answer',/а не исчерпывающий ответ|not an exhaustive answer/i],
  ['a contrast with a complete school opposition',/не полное противопоставление двух школ|not a complete opposition between the two schools/i],
  ['model-oriented language in a reading',/методологический якорь ветви|branch.s methodological anchor|формул\w* узла|словесном окружении/i],
  ['the split rendering of doxa in claims about true or false opinion',/\b(?:true|false) beliefs?\b/i],
  ['an account substituted for the common form in Meno',/Which example would make the demand for a general account clearer\?/i],
  ['a split rendering of phronesis in these Epicurean forms',/Prudence in a Pleasant Life|Pleasure Requires Prudence/i],
];
function readerTexts(value,path='',found=[]){
  if(Array.isArray(value))value.forEach((item,index)=>readerTexts(item,`${path}[${index}]`,found));
  else if(value&&typeof value==='object'){
    for(const lang of ['ru','en'])if(typeof value[lang]==='string')found.push({path:`${path}.${lang}`,text:value[lang]});
    for(const [key,item] of Object.entries(value))if(key!=='ru'&&key!=='en'&&key!=='texts')readerTexts(item,path?`${path}.${key}`:key,found);
  }
  return found;
}
describe('the language of the Antiquity stand',()=>{
  const texts=files.flatMap(name=>readerTexts(JSON.parse(readFileSync(new URL(name,root),'utf8'))).map(row=>({name,...row})));
  it('reads both languages throughout the stand modules',()=>{expect(texts.length).toBeGreaterThan(14000);});
  for(const [what,pattern] of outside)it(`holds no ${what}`,()=>{
    expect(texts.filter(({text})=>pattern.test(text)).map(({name,path,text})=>`${name}:${path}: ${text}`)).toEqual([]);
  });
});
