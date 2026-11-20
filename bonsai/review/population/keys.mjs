// The name keys the identity resolution compares, and the slug of a member's Tree proposal.
//
// A key is a fold of one name form: compatibility-normalized, lower-cased, punctuation turned to
// spaces. Marks are removed only where they do not tell letters apart: in Latin, Greek, Arabic and
// Hebrew script (Ḥunayn = Hunayn); Cyrillic keeps й and folds only ё to е; every other script keeps
// its marks. A key is `strong` when it carries two name words besides particles (or three Han,
// kana or Hangul characters) and `short` otherwise. A form's base (without a parenthesis, a
// descriptor after a comma or a dash) is always short: the register added the descriptor to tell
// people apart.

export const PARTICLES=new Set([
  'ibn','ben','bar','bat','bin','bint','al','el','ad','ar','as','at','az','an','ash','ath','ul',
  'de','del','della','dei','degli','di','da','do','dos','das','du','des','la','le','les','lo',
  'van','von','der','den','ten','ter','zu','zur','of','the','and','y','e','und','et',
  'и','фон','де','ван','дер','ибн','аль','ал','бен','бар','бин','ди','да','дю','ла','ле','из','о'
]);
const MARKLESS=new Set(['Latn','Grek','Arab','Hebr']);
const IDEOGRAPHIC=new Set(['Hani','Jpan','Kore']);
const SCRIPTS=[
  ['Latn',/\p{Script=Latin}/u],['Cyrl',/\p{Script=Cyrillic}/u],['Grek',/\p{Script=Greek}/u],
  ['Arab',/\p{Script=Arabic}/u],['Hebr',/\p{Script=Hebrew}/u],['Hani',/\p{Script=Han}/u],
  ['Jpan',/[\p{Script=Hiragana}\p{Script=Katakana}]/u],['Kore',/\p{Script=Hangul}/u]
];
const LATIN_LETTERS={'ø':'o','Ø':'o','đ':'d','Đ':'d','ł':'l','Ł':'l','ß':'ss','æ':'ae','Æ':'ae','œ':'oe','Œ':'oe','ı':'i','ð':'d','Ð':'d','þ':'th','Þ':'th','ħ':'h','Ħ':'h'};
const APOSTROPHES=/[ʹʺʻʼʽʾʿ‘’‛'`´]/g;

/** The script most letters of a text are written in, among those the keys treat apart; else 'Other'. */
export function scriptOf(text){
  const counts=new Map();
  for(const ch of text){
    if(!/\p{L}/u.test(ch))continue;
    const script=SCRIPTS.find(([,pattern])=>pattern.test(ch))?.[0]??'Other';
    counts.set(script,(counts.get(script)??0)+1);
  }
  let best=null;
  for(const [script,count] of [...counts].sort(([a],[b])=>a<b?-1:a>b?1:0))if(!best||count>counts.get(best))best=script;
  return best??'Other';
}

export const ideographic=script=>IDEOGRAPHIC.has(script);

/** The fold of one name form. */
export function fold(text){
  const script=scriptOf(text);
  let value=text.normalize('NFKC').replace(APOSTROPHES,'');
  if(script==='Cyrl')value=value.toLowerCase().replace(/ё/g,'е');
  else if(MARKLESS.has(script)){
    value=value.replace(/[øØđĐłŁßæÆœŒıðÐþÞħĦ]/g,ch=>LATIN_LETTERS[ch]);
    value=value.normalize('NFKD').replace(/\p{M}/gu,'').normalize('NFC').toLowerCase().replace(/ς/g,'σ');
  }else value=value.toLowerCase();
  return value.replace(/[^\p{L}\p{N}\p{M}]+/gu,' ').trim();
}

/** Whether a key carries enough of a name to stand without a date. */
export function strength(key,script){
  if(IDEOGRAPHIC.has(script))return [...key.replace(/\s/g,'')].length>=3?'strong':'short';
  return key.split(' ').filter(token=>[...token].length>1&&!PARTICLES.has(token)).length>=2?'strong':'short';
}

/** The keys of one form: the whole form, its inversion ("Leibniz, Gottfried" → "Gottfried Leibniz") and its base. */
export function formKeys(text){
  const out=new Map();
  const add=(value,how)=>{
    const key=fold(value);
    if(!key)return;
    const script=scriptOf(value);
    const level=how==='base'?'short':strength(key,script);
    const known=out.get(key);
    if(!known||(known.strength==='short'&&level==='strong'))out.set(key,{key,script,how,strength:level});
  };
  add(text,'full');
  let base=text.replace(/\s*\([^()]*\)/g,' ').replace(/\s+[—–]\s+.*$/,'').replace(/\s+/g,' ').trim();
  const comma=base.indexOf(', ');
  if(comma>0){
    const left=base.slice(0,comma).trim(),right=base.slice(comma+2).trim();
    if(left&&right&&!right.includes(','))add(`${right} ${left}`,'inverted');
    base=left;
  }
  if(base&&base!==text)add(base,'base');
  return [...out.values()];
}

const OF=new Set(['of','from','из']);
const KIN=new Set(['son','daughter','father','mother','wife','brother','sister','сын','дочь','отец','мать','жена','брат','сестра']);

/**
 * The name a form qualifies by its origin ("Hierocles of Alexandria" → "hierocles"), or null. It
 * shares no key with the same name standing alone, so resolve.mjs asks a reader about the two.
 */
export function formHead(text){
  const base=text.replace(/\s*\([^()]*\)/g,' ').replace(/\s+[—–]\s+.*$/,'').split(', ')[0];
  const words=fold(base).split(' ');
  const at=words.findIndex((word,i)=>i>0&&OF.has(word));
  if(at<1||words.slice(0,at).some(word=>KIN.has(word)))return null;
  return words.slice(0,at).join(' ');
}

const CYRILLIC={'а':'a','б':'b','в':'v','г':'g','д':'d','е':'e','ё':'e','ж':'zh','з':'z','и':'i','й':'i','к':'k','л':'l','м':'m','н':'n','о':'o','п':'p','р':'r','с':'s','т':'t','у':'u','ф':'f','х':'kh','ц':'ts','ч':'ch','ш':'sh','щ':'shch','ъ':'','ы':'y','ь':'','э':'e','ю':'yu','я':'ya','і':'i','ї':'yi','є':'ye','ґ':'g','ў':'u'};
const SLUG_LETTERS={'ä':'ae','ö':'oe','ü':'ue','Ä':'ae','Ö':'oe','Ü':'ue',...LATIN_LETTERS};

/** A slug in the Tree's habit (ue, oe, ae for German letters), at most 48 characters, cut at a word. */
export function slugify(text){
  let value=text.normalize('NFC').replace(APOSTROPHES,'').replace(/[äöüÄÖÜøØđĐłŁßæÆœŒıðÐþÞħĦ]/g,ch=>SLUG_LETTERS[ch]);
  value=[...value.toLowerCase()].map(ch=>CYRILLIC[ch]??ch).join('');
  value=value.normalize('NFKD').replace(/\p{M}/gu,'').replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'');
  if(value.length>48){
    value=value.slice(0,49);
    value=value.includes('-')?value.slice(0,value.lastIndexOf('-')):value.slice(0,48);
  }
  return value;
}
