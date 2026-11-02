import {readFileSync,readdirSync} from 'node:fs';
import {describe,expect,it} from 'vitest';

// The reader texts of the Zarathustra core speak with the words of the Russian witness shown beside them (Antonovsky
// 1913). These are the words and turns the language review of 2026-11-02 took out of them; a text written later that
// brings one back fails here. A reviewer's note kept as a plain string is provenance and keeps its words.
const root=new URL('../../../../bonsai/zarathustra/core/',import.meta.url);
const read=name=>JSON.parse(readFileSync(new URL(name,root),'utf8'));
const files=['concepts.json','figures.json','links.json','chains.json','proposed.json','voices.json',...readdirSync(new URL('occurrences/',root)).map(name=>`occurrences/${name}`)];

const outside=[
  ['a word the translation does not use for Kobolde',/домов(ой|ых|ые|ым)/i],
  ['the theological Creator for der Schaffende',/Создател/],
  ['a coinage for Mitschaffende',/со-творц/i],
  ['a calque for eine erste Bewegung',/перв(ое|ым|ого) движени/i],
  ['a calque for ein aus sich rollendes Rad',/катящ\w* из себя/i],
  ['a registry family read out as a sentence',/^(Действие, ход или изменение|Сходство, различие или сопоставление|Выражение или представление через образ|Основание, условие или конфликт|Атрибуция, роль или отношение)\s*:/],
  ['words about the data model',/формул\w* узла|начало узла|маршрут\w* понятия|словесном окружении/i],
  ['a negation the book does not make',/не тождеств|остаётся отдельн/i],
];

function readerTexts(value,found=[]){
  if(Array.isArray(value))for(const item of value)readerTexts(item,found);
  else if(value&&typeof value==='object'){
    if(typeof value.ru==='string')found.push(value.ru);
    for(const [key,item] of Object.entries(value))if(key!=='ru'&&key!=='en')readerTexts(item,found);
  }
  return found;
}

describe('the language of the bonsai core',()=>{
  const texts=files.flatMap(name=>readerTexts(read(name)).map(text=>({name,text})));
  it('reads the reader texts of every core file',()=>{expect(texts.length).toBeGreaterThan(8000);});
  for(const [what,pattern] of outside)it(`holds no ${what}`,()=>{
    expect(texts.filter(({text})=>pattern.test(text)).map(({name,text})=>`${name}: ${text}`)).toEqual([]);
  });
});
