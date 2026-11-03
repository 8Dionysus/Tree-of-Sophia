import {readFileSync,readdirSync} from 'node:fs';
import {describe,expect,it} from 'vitest';

// The Antiquity stand's own bilingual forms, excluding quoted witness texts.
const root=new URL('../../../../bonsai/antiquity/',import.meta.url);
const files=readdirSync(root).filter(name=>name.endsWith('.json')&&name!=='alignment.json');
const outside=[
  ['a padding turn built on not only or not merely',/не только.{0,100}но и|not only.{0,100}but (?:also )?|не просто|not merely/i, new Set(['collection.json:branches[11].edges[0].body.en','first-branches.json:branches[5].edges[0].body.en'])],
  ['a contrast expressed through non-identity',/не тождествен/i],
  ['a padding turn using rather than',/\brather than\b/i, new Set(['collection.json:branches[14].nodes[5].reading.en'])],
  ['empty descriptions of a reading or comparison',/Это предложение для проверки|Это отправная точка сравнения|Это нравственный совет правителю|This is a proposal to test|This is the starting point for comparison|This is moral advice to a ruler/i],
  ['padding about a completed result',/готов(?:ое|ым) опровержени|завершённый итог|ready-made refutation|completed result/i],
  ['a contrast with a universal theory or template',/а не единую теорию искусства|а не шаблон для всех сюжетов|not one theory of art|not a template for every plot/i],
  ['a contrast with an exhaustive answer',/а не исчерпывающий ответ|not an exhaustive answer/i],
  ['a contrast with a complete school opposition',/не полное противопоставление двух школ|not a complete opposition between the two schools/i],
  ['model-oriented language in a reading',/методологический якорь ветви|branch.s methodological anchor|формул\w* узла|словесном окружении/i],
  ['the split rendering of doxa in claims about true or false opinion',/\b(?:true|false) beliefs?\b/i],
  ['the split Russian rendering of phantasia in the reviewed assent forms',/От впечатления к согласию с ним|соглашаться с впечатлением|Распознать впечатление|впечатление становится моим решением|Дать впечатлению подождать|переход от возникшего впечатления к его одобрению|отделяет возникшее впечатление от суждения|когда впечатление стало принятым суждением/i],
  ['the split rendering of ethical arete in the virtue entry',/Совершенство в том, чтобы хорошо жить и действовать|Excellence in living and acting well/i],
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
  it('retains the question about inquiry testing opinions and the grammatical preposition',()=>{
    const text=(path)=>texts.find(row=>row.name==='collection.json'&&row.path===path)?.text;
    expect(text('passages[8].readingSteps[0].question.ru')).toBe('Что делает такую беседу самостоятельным исследованием, которое проверяет мнения?');
    expect(text('passages[8].readingSteps[0].question.en')).toBe('What makes such a conversation an independent inquiry that tests opinions?');
    expect(text('entities[58].workMap.stages[4].title.ru')).toBe('Связать приёмы со знанием слушателя');
  });
  it('keeps the shared branch reading and route in agreement',()=>{
    const collection=JSON.parse(readFileSync(new URL('collection.json',root),'utf8'));
    const first=JSON.parse(readFileSync(new URL('first-branches.json',root),'utf8'));
    for(const branch of first.branches){
      const expanded=collection.branches.find(item=>item.id===branch.id);
      for(const field of ['rootTitle','summary','question'])for(const lang of ['ru','en'])
        expect(branch[field]?.[lang],`${branch.id} ${field}.${lang}`).toBe(expanded[field]?.[lang]);
      for(const [index,node] of branch.nodes.entries()){
        for(const field of ['title','reading','question'])for(const lang of ['ru','en'])
          expect(node[field]?.[lang],`${branch.id} node ${index} ${field}.${lang}`).toBe(expanded.nodes[index][field]?.[lang]);
      }
      for(const [index,edge] of branch.edges.entries()){
        for(const field of ['label','body'])for(const lang of ['ru','en'])
          expect(edge[field]?.[lang],`${branch.id} edge ${index} ${field}.${lang}`).toBe(expanded.edges[index][field]?.[lang]);
      }
    }
  });
  it('states Aristotle’s distinction and retains the contrast in Plato’s Ion',()=>{
    const text=(path)=>texts.find(row=>row.name==='collection.json'&&row.path===path)?.text;
    expect(text('branches[11].edges[0].body.en')).toBe('In the Politics, Aristotle distinguishes kinds of rule by their character, not merely by the number of those ruled.');
    expect(text('branches[14].nodes[5].reading.en')).toBe('In Plato’s Ion, Socrates attributes poetic creation to divine inspiration rather than skill.');
  });
  it('uses one word for a sense while distinguishing general from Stoic assent',()=>{
    const text=(path)=>texts.find(row=>row.name==='collection.json'&&row.path===path)?.text;
    expect(text('passages[153].readingSteps[1].question.en')).toBe("Is one's own nature enough to call an action free?");
    expect(text('terms[31].body.en')).toBe('Assent; in Stoic philosophy, assent to an impression.');
    expect(text('terms[25].body.ru')).toContain('рассудительность в ведении дел');
    expect(text('branches[0].nodes[6].question.en')).toBe('Is true opinion with an account enough, or must the account disclose the cause?');
    expect(text('branches[0].nodes[7].question.en')).toBe('What grounds would let us accept an account if it too requires examination?');
  });
  it('asks directly about knowledge, action and explanation',()=>{
    const text=(path)=>texts.find(row=>row.name==='collection.json'&&row.path===path)?.text;
    expect(text('entities[82].question.ru')).toBe('Когда убеждение становится знанием?');
    expect(text('passages[28].readingSteps[0].question.ru')).toBe('Что такое объяснение говорит о конкретной вещи?');
    expect(text('passages[85].readingSteps[0].question.ru')).toBe('Что нужно человеку для успешного действия?');
  });
  for(const [what,pattern,exceptions=new Set()] of outside)it(`holds no ${what}`,()=>{
    expect(texts.filter(({name,path,text})=>pattern.test(text)&&!exceptions.has(`${name}:${path}`)).map(({name,path,text})=>`${name}:${path}: ${text}`)).toEqual([]);
  });
});
