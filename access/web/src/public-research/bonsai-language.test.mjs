import {readFileSync,readdirSync} from 'node:fs';
import {describe,expect,it} from 'vitest';

// The reader texts of the Zarathustra core speak with the words of the Russian witness shown beside them (Antonovsky
// 1913). These are the words and turns the language review of 2026-11-02 took out of them; a text written later that
// brings one back fails here. Original reviews remain in the assembly inputs;
// selected plain-string reasons have separate reader-facing corrections.
const root=new URL('../../../../bonsai/zarathustra/core/',import.meta.url);
const read=name=>JSON.parse(readFileSync(new URL(name,root),'utf8'));
const terms=JSON.parse(readFileSync(new URL('../../../../bonsai/zarathustra/terms.json',import.meta.url),'utf8'));
function jsonFiles(directory=''){
  return readdirSync(new URL(directory,root),{withFileTypes:true}).flatMap(entry=>{
    const name=`${directory}${entry.name}`;
    return entry.isDirectory()?jsonFiles(`${name}/`):entry.isFile()&&entry.name.endsWith('.json')&&name!=='control.json'?[name]:[];
  });
}
const files=jsonFiles();
// The earlier review's phrase bans apply to its original reader-text scope;
// semantic-use assessments may explicitly state the limits of a comparison.
const reviewedFiles=new Set(['concepts.json','figures.json','links.json','chains.json','proposed.json','voices.json',...files.filter(name=>name.startsWith('occurrences/'))]);

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
    for(const lang of ['ru','en'])if(typeof value[lang]==='string')found.push({lang,text:value[lang]});
    for(const [key,item] of Object.entries(value))if(key!=='ru'&&key!=='en')readerTexts(item,found);
  }
  return found;
}

const escape=value=>value.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
const avoidPattern=(word,lang)=>{
  // Lower-case English entries reject any capitalization. An entry with capitals
  // names a case-sensitive form, while Russian avoid forms always ignore case.
  const caseSensitive=lang==='en'&&word!==word.toLocaleLowerCase('en');
  const flags=caseSensitive?'u':'iu';
  return new RegExp(`(?<![\\p{L}\\p{N}])${escape(word)}(?![\\p{L}\\p{N}])`,flags);
};

describe('the language of the bonsai core',()=>{
  const texts=files.flatMap(name=>readerTexts(read(name)).map(({lang,text})=>({name,lang,text})));
  it('reads the Russian and English reader texts of every core file',()=>{
    for(const lang of ['ru','en'])expect(texts.filter(entry=>entry.lang===lang).length).toBeGreaterThan(8000);
  });
  it('keeps the reviewer source while showing the corrected reason',()=>{
    const review=JSON.parse(readFileSync(new URL('../../../../bonsai/review/zarathustra-graph/graph-words/assembly-inputs/review-language-2026-11-02.json',import.meta.url),'utf8'));
    const entry=review.corrections.find(item=>item.ru?.[0].includes('возвещает сверхчеловека в ней сам Заратустра'));
    expect(entry).toBeTruthy();
    expect(JSON.stringify(read('chains.json'))).toContain(entry.ru[1]);
    const originals=readFileSync(new URL('../../../../bonsai/review/zarathustra-graph/graph-words/assembly-inputs/verify-concepts.json',import.meta.url),'utf8');
    expect(originals).toContain(entry.ru[0]);
  });
  for(const [what,pattern] of outside)it(`holds no ${what}`,()=>{
    expect(texts.filter(({name,lang,text})=>reviewedFiles.has(name)&&lang==='ru'&&pattern.test(text)).map(({name,text})=>`${name}: ${text}`)).toEqual([]);
  });
  it('uses the registered Russian and English words for every term',()=>{
    const violations=[];
    for(const term of terms.terms??[])for(const lang of ['ru','en']){
      const languageTexts=texts.filter(entry=>entry.lang===lang);
      for(const word of term.renderings?.[lang]?.avoid??[]){
        const pattern=avoidPattern(word,lang);
        for(const {name,text} of languageTexts)if(pattern.test(text))violations.push(`${name} (${term.id}, ${lang}, avoid “${word}”): ${text}`);
      }
    }
    expect(violations).toEqual([]);
  });
});
