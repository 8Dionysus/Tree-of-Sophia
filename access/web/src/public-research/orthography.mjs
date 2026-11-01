// Russian texts printed before the reform of 1918 are shown in today's spelling.
//
// The source text is never changed: alignment, citation offsets and search keep
// counting its characters. Only what a reader sees is respelled, unit by unit or
// place by place, after the place has been cut out of the source. The rules are
// the reform's own: the letters ѣ, і, ѳ and ѵ give way to е, и, ф and и; a hard
// sign at the end of a word goes; adjective endings -аго/-яго, -ыя/-ія become
// -ого/-его, -ые/-ие; prefixes in -з keep -с before a voiceless consonant; онѣ,
// однѣ and ея become они, одни and её. Where a rule cannot see the grammar
// (-ія of a noun or of an adjective, -аго under stress after a hushing
// consonant), the common case is taken and the known exceptions are listed.

const OLD_LETTERS=/[ѣѢіІѳѲѵѴ]/u;
const LETTER=/\p{L}/u;

/** Whether a Russian text is in the spelling before 1918. */
export function isPreReform(text){
  if(typeof text!=='string'||!text)return false;
  return OLD_LETTERS.test(text)||/[бвгджзклмнпрстфхцчшщ]ъ(?![\p{L}])/u.test(text);
}

// Whole words the endings' rules would get wrong, or that the reform respelled outright.
const WORDS=new Map(Object.entries({
  'онѣ':'они','однѣ':'одни','однѣхъ':'одних','однѣмъ':'одним','однѣми':'одними',
  'ея':'её','нея':'неё','божія':'божия',
  'благо':'благо','наго':'наго','саго':'саго',
  'большаго':'большого','чужаго':'чужого','дорогаго':'дорогого','другаго':'другого','всякаго':'всякого',
}));
// Adjectives in -нія and -кія/-гія/-хія: their plural ending is -ие, while nouns in -ія keep -ия.
const ADJECTIVES_IA=new Set([
  'древнія','послѣднія','синія','искреннія','нижнія','верхнія','внутреннія','внѣшнія','здѣшнія','тамошнія',
  'тогдашнія','нынѣшнія','вчерашнія','сегодняшнія','утреннія','вечернія','лѣтнія','зимнія','осеннія','весеннія',
  'домашнія','среднія','крайнія','прежнія','давнія','позднія','раннія','излишнія','кромѣшнія','сосѣднія',
]);
// Adjectives in -чія; most words in -чія are nouns from -чіе (различія, красноречія).
const ADJECTIVES_CHIA=new Set(['горячія','могучія','колючія','летучія','текучія','кипучія','певучія','жгучія','висячія','лежачія','сидячія','ходячія','стоячія','зрячія']);
// Nouns in -гія and -хія, which keep -ия.
const NOUNS_IA=/(?:логія|энергія|магія|стратегія|литургія|хирургія|аллергія|теургія|демагогія|педагогія|архія)$/u;
const VOICELESS='кпстфхцчшщ';
const PREFIXES=['через','чрез','без','воз','раз','роз','из'];

function keepCase(model,word){
  if(model===model.toLocaleUpperCase('ru')&&model!==model.toLocaleLowerCase('ru')&&model.length>1)return word.toLocaleUpperCase('ru');
  if(model[0]!==model[0].toLocaleLowerCase('ru'))return word[0].toLocaleUpperCase('ru')+word.slice(1);
  return word;
}

/** One word in today's spelling; the word keeps its capitals. */
export function modernWord(word){
  const lower=word.toLocaleLowerCase('ru');
  if(WORDS.has(lower))return keepCase(word,WORDS.get(lower));
  let w=lower;
  // A hard sign closing the word goes; inside the word it stays a separator (подъѣздъ → подъезд).
  w=w.replace(/ъ$/u,'');
  // Endings, read before the letters change.
  if(/[жшчщ]аго$/u.test(w))w=w.slice(0,-3)+'его';
  else if(w.length>4&&/аго$/u.test(w))w=w.slice(0,-3)+'ого';
  else if(w.length>4&&/яго$/u.test(w))w=w.slice(0,-3)+'его';
  if(/ыя$/u.test(w)&&w.length>3)w=w.slice(0,-2)+'ые';
  // Participles in -щія and adjectives in -шія and -жія are adjectives, but not nouns from -душіе or оружіе.
  else if(ADJECTIVES_IA.has(w)||ADJECTIVES_CHIA.has(w)||/щія$/u.test(w)||(/шія$/u.test(w)&&!/душія$/u.test(w))||(/жія$/u.test(w)&&!/оружія$/u.test(w))||(/[кгх]ія$/u.test(w)&&!NOUNS_IA.test(w)&&word[0]===word[0].toLocaleLowerCase('ru')))w=w.slice(0,-2)+'ие';
  // Prefixes in -з before a voiceless consonant keep the -с the ear hears (разсказъ → рассказ).
  for(const prefix of PREFIXES){
    if(w.length>prefix.length+2&&w.startsWith(prefix)&&VOICELESS.includes(w[prefix.length])){w=prefix.slice(0,-1)+'с'+w.slice(prefix.length);break;}
  }
  // The letters themselves: ижица after а or е sounds as в (Еѵангеліе), elsewhere as и.
  w=w.replace(/([ае])ѵ/gu,'$1в').replace(/ѣ/gu,'е').replace(/і/gu,'и').replace(/ѳ/gu,'ф').replace(/ѵ/gu,'и');
  return w===lower?word:keepCase(word,w);
}

/** A text in today's spelling. Punctuation, spaces and line breaks stay as they are. */
export function modernRussian(text){
  if(!isPreReform(text))return text;
  let out='',word='';
  const flush=()=>{if(word){out+=modernWord(word);word='';}};
  for(const character of text){
    if(LETTER.test(character))word+=character;
    else{flush();out+=character;}
  }
  flush();
  return out;
}

/**
 * A text in today's spelling with the way back: `toSource[i]` is the offset in the source text (in UTF-16 units) of
 * the shown text's offset `i`, so that a selection made in the shown text names the source's own characters.
 * Inside a respelled word an offset keeps its place from the word's start, up to the source word's end.
 */
export function respell(text){
  const toSource=[];
  if(!isPreReform(text)){for(let index=0;index<=text.length;index++)toSource.push(index);return {text,toSource};}
  let out='',word='',wordStart=0,at=0;
  const flush=()=>{
    if(!word)return;
    const shown=modernWord(word);
    for(let index=0;index<shown.length;index++)toSource.push(wordStart+Math.min(index,word.length));
    out+=shown;word='';
  };
  for(const character of text){
    if(LETTER.test(character)){if(!word)wordStart=at;word+=character;}
    else{flush();for(let index=0;index<character.length;index++)toSource.push(at+index);out+=character;}
    at+=character.length;
  }
  flush();toSource.push(text.length);
  return {text:out,toSource};
}
