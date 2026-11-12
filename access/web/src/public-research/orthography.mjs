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
//
// A transcription already in today's spelling may keep a few old forms: an
// adjective in -ия (долгия сумерки), a prefix in -з before a voiceless consonant
// (неизследованным). Those words are respelled by the same rules; the endings'
// rules there read only words in lower case, since a name keeps its own form.

const OLD_LETTERS=/[ѣѢіІѳѲѵѴ]/u;
const LETTER=/\p{L}/u;
// The old forms a transcription in today's spelling may keep.
const RESIDUAL=/з[кпстфхцчшщ]|[ыи]я(?!\p{L})|[ая]го(?!\p{L})|(?<!\p{L})н?ея(?!\p{L})/iu;

/** Whether a Russian text is in the spelling before 1918. */
export function isPreReform(text){
  if(typeof text!=='string'||!text)return false;
  return OLD_LETTERS.test(text)||/[бвгджзклмнпрстфхцчшщ]ъ(?![\p{L}])/iu.test(text);
}
/** Whether a Russian text shows any word in another spelling than today's: the old spelling, or old forms it kept. */
export function needsRespelling(text){
  return isPreReform(text)||(typeof text==='string'&&RESIDUAL.test(text));
}

// Whole words the endings' rules would get wrong, or that the reform respelled outright.
const WORDS=new Map(Object.entries({
  'онѣ':'они','однѣ':'одни','однѣхъ':'одних','однѣмъ':'одним','однѣми':'одними',
  'ея':'её','нея':'неё','божія':'божия',
  'благо':'благо','наго':'наго','саго':'саго',
  'большаго':'большого','чужаго':'чужого','дорогаго':'дорогого','другаго':'другого','всякаго':'всякого',
}));
// A word of a list in its old spelling and in today's letters before its ending (послѣднія, последнія).
const listed=words=>new Set(words.flatMap(word=>[word,word.replace(/ѣ/gu,'е')]));
// Adjectives in -нія and -кія/-гія/-хія: their plural ending is -ие, while nouns in -ія keep -ия.
const ADJECTIVES_IA=listed([
  'древнія','послѣднія','синія','искреннія','нижнія','верхнія','внутреннія','внѣшнія','здѣшнія','тамошнія',
  'тогдашнія','нынѣшнія','вчерашнія','сегодняшнія','утреннія','вечернія','лѣтнія','зимнія','осеннія','весеннія',
  'домашнія','среднія','крайнія','прежнія','давнія','позднія','раннія','излишнія','кромѣшнія','сосѣднія',
]);
// Adjectives in -чія; most words in -чія are nouns from -чіе (различія, красноречія).
const ADJECTIVES_CHIA=listed(['горячія','могучія','колючія','летучія','текучія','кипучія','певучія','жгучія','висячія','лежачія','сидячія','ходячія','стоячія','зрячія','скрипучія','гремучія','дремучія','трескучія','шипучія','плакучія','ползучія','живучія','вонючія','блестящія']);
// Nouns in -гія and -хія, which keep -ия.
const NOUNS_IA=/(?:логія|энергія|магія|стратегія|ургія|аллергія|гогія|архія|стихія|легія|льгія|алгія|оргія)$/u;
const VOICELESS='кпстфхцчшщ';
const PREFIXES=['через','чрез','без','воз','раз','роз','из'];

function keepCase(model,word){
  if(model===model.toLocaleUpperCase('ru')&&model!==model.toLocaleLowerCase('ru')&&model.length>1)return word.toLocaleUpperCase('ru');
  if(model[0]!==model[0].toLocaleLowerCase('ru'))return word[0].toLocaleUpperCase('ru')+word.slice(1);
  return word;
}

function modernEnding(w,word){
  if(/[жшчщ]аго$/u.test(w))w=w.slice(0,-3)+'его';
  else if(w.length>4&&/аго$/u.test(w))w=w.slice(0,-3)+'ого';
  else if(w.length>4&&/яго$/u.test(w))w=w.slice(0,-3)+'его';
  if(/ыя$/u.test(w)&&w.length>3)return w.slice(0,-2)+'ые';
  // Participles in -щія and adjectives in -шія and -жія are adjectives, but not nouns from -душіе or оружіе.
  if(ADJECTIVES_IA.has(w)||ADJECTIVES_CHIA.has(w)||/щія$/u.test(w)||(/шія$/u.test(w)&&!/душія$/u.test(w))||(/жія$/u.test(w)&&!/оружія$/u.test(w))||(/[кгх]ія$/u.test(w)&&!NOUNS_IA.test(w)&&word[0]===word[0].toLocaleLowerCase('ru')))return w.slice(0,-2)+'ие';
  return w;
}

/** One word in today's spelling; the word keeps its capitals. `kept` reads a word of a text in today's spelling. */
export function modernWord(word,{kept=false}={}){
  const lower=word.toLocaleLowerCase('ru');
  if(WORDS.has(lower))return keepCase(word,WORDS.get(lower));
  // In today's letters the ending -ия is read as the old -ія, so that one set of rules serves both texts.
  let w=kept?lower.replace(/ия$/u,'ія'):lower;
  const endings=!kept||word[0]===lower[0];
  // A hard sign closing the word goes; inside the word it stays a separator (подъѣздъ → подъезд).
  w=w.replace(/ъ$/u,'');
  // Endings, read before the letters change.
  if(endings)w=modernEnding(w,word);
  // Prefixes in -з before a voiceless consonant keep the -с the ear hears (разсказъ → рассказ, неизслѣдованный →
  // неисследованный).
  const negated=w.startsWith('не')?2:0;
  for(const prefix of PREFIXES){
    const at=negated+prefix.length;
    if(w.length>at+2&&w.startsWith(prefix,negated)&&VOICELESS.includes(w[at])){w=w.slice(0,negated)+prefix.slice(0,-1)+'с'+w.slice(at);break;}
  }
  // The letters themselves: ижица after а or е sounds as в (Еѵангеліе), elsewhere as и.
  w=w.replace(/([ае])ѵ/gu,'$1в').replace(/ѣ/gu,'е').replace(/і/gu,'и').replace(/ѳ/gu,'ф').replace(/ѵ/gu,'и');
  return w===lower?word:keepCase(word,w);
}

/** A text in today's spelling. Punctuation, spaces and line breaks stay as they are. */
export function modernRussian(text){
  if(!needsRespelling(text))return text;
  const kept=!isPreReform(text);
  let out='',word='';
  const flush=()=>{if(word){out+=modernWord(word,{kept});word='';}};
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
 * `toDisplay[i]` maps a source UTF-16 boundary into the shown text. Inside a respelled word an offset keeps its
 * place from the word's start, up to the source word's end.
 */
export function respell(text){
  const toSource=[];
  const result=shown=>{
    const toDisplay=[];let at=0;
    for(let index=0;index<=text.length;index++){
      while(at<toSource.length-1&&toSource[at]<index)at++;
      toDisplay.push(at);
    }
    return {text:shown,toSource,toDisplay};
  };
  if(!needsRespelling(text)){for(let index=0;index<=text.length;index++)toSource.push(index);return result(text);}
  const kept=!isPreReform(text);
  let out='',word='',wordStart=0,at=0;
  const flush=()=>{
    if(!word)return;
    const shown=modernWord(word,{kept});
    for(let index=0;index<shown.length;index++)toSource.push(wordStart+Math.min(index,word.length));
    out+=shown;word='';
  };
  for(const character of text){
    if(LETTER.test(character)){if(!word)wordStart=at;word+=character;}
    else{flush();for(let index=0;index<character.length;index++)toSource.push(at+index);out+=character;}
    at+=character.length;
  }
  flush();toSource.push(text.length);
  return result(out);
}
