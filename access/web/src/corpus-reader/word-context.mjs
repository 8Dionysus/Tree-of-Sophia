// The word in its sentence, as the stand's word layer reads it: lemma, part of
// speech, grammatical features, today's spelling and the word it depends on.
import {copyLocale,localeText} from '../public-research/locale.mjs';

const RU={
  NOUN:'существительное',PROPN:'имя собственное',VERB:'глагол',AUX:'вспомогательный глагол',ADJ:'прилагательное',ADV:'наречие',
  PRON:'местоимение',DET:'артикль или определитель',ADP:'предлог',CCONJ:'сочинительный союз',SCONJ:'подчинительный союз',PART:'частица',
  INTJ:'междометие',NUM:'числительное',SYM:'символ',X:'иное',
  'Case=Nom':'именительный падеж','Case=Gen':'родительный падеж','Case=Dat':'дательный падеж','Case=Acc':'винительный падеж',
  'Case=Ins':'творительный падеж','Case=Loc':'предложный падеж','Case=Voc':'звательный падеж','Case=Par':'партитив',
  'Number=Sing':'единственное число','Number=Plur':'множественное число',
  'Gender=Masc':'мужской род','Gender=Fem':'женский род','Gender=Neut':'средний род',
  'Person=1':'1-е лицо','Person=2':'2-е лицо','Person=3':'3-е лицо',
  'Tense=Pres':'настоящее время','Tense=Past':'прошедшее время','Tense=Fut':'будущее время',
  'Mood=Ind':'изъявительное наклонение','Mood=Imp':'повелительное наклонение','Mood=Sub':'сослагательное наклонение','Mood=Cnd':'условное наклонение',
  'VerbForm=Inf':'инфинитив','VerbForm=Part':'причастие','VerbForm=Conv':'деепричастие','VerbForm=Ger':'герундий',
  'Voice=Act':'действительный залог','Voice=Pass':'страдательный залог','Voice=Mid':'средний залог',
  'Aspect=Imp':'несовершенный вид','Aspect=Perf':'совершенный вид',
  'Degree=Cmp':'сравнительная степень','Degree=Sup':'превосходная степень',
  'Definite=Def':'определённый','Definite=Ind':'неопределённый',
  'PronType=Prs':'личное','PronType=Dem':'указательное','PronType=Art':'артикль','PronType=Rel':'относительное','PronType=Int':'вопросительное',
  'PronType=Ind':'неопределённое','PronType=Neg':'отрицательное','PronType=Tot':'определительное',
  'Animacy=Anim':'одушевлённое','Animacy=Inan':'неодушевлённое','Polarity=Neg':'отрицание','Poss=Yes':'притяжательное',
  'Reflex=Yes':'возвратное','Variant=Short':'краткая форма',
  nsubj:'подлежащее','nsubj:pass':'подлежащее страдательного оборота',obj:'прямое дополнение',iobj:'косвенное дополнение',
  obl:'обстоятельство или косвенное дополнение',nmod:'несогласованное определение',amod:'определение',advmod:'обстоятельство',
  det:'определитель',case:'предлог',mark:'союз придаточного',cc:'сочинительный союз',conj:'однородный член',cop:'связка',
  aux:'вспомогательный глагол','aux:pass':'вспомогательный глагол страдательного залога',acl:'определительное придаточное',
  'acl:relcl':'относительное придаточное',advcl:'обстоятельственное придаточное',ccomp:'дополнительное придаточное',
  xcomp:'зависимый инфинитив',appos:'приложение','compound:prt':'отделяемая приставка',flat:'часть имени',vocative:'обращение',
  expl:'формальное слово',parataxis:'вставка',nummod:'числительное при слове','obl:arg':'управляемое дополнение',
  root:'вершина предложения',
};
const EN={
  NOUN:'noun',PROPN:'proper noun',VERB:'verb',AUX:'auxiliary',ADJ:'adjective',ADV:'adverb',PRON:'pronoun',DET:'article or determiner',
  ADP:'preposition',CCONJ:'coordinating conjunction',SCONJ:'subordinating conjunction',PART:'particle',INTJ:'interjection',
  NUM:'numeral',SYM:'symbol',X:'other',
  'Case=Nom':'nominative','Case=Gen':'genitive','Case=Dat':'dative','Case=Acc':'accusative','Case=Ins':'instrumental',
  'Case=Loc':'prepositional','Case=Voc':'vocative','Case=Par':'partitive','Number=Sing':'singular','Number=Plur':'plural',
  'Gender=Masc':'masculine','Gender=Fem':'feminine','Gender=Neut':'neuter','Person=1':'first person','Person=2':'second person',
  'Person=3':'third person','Tense=Pres':'present','Tense=Past':'past','Tense=Fut':'future','Mood=Ind':'indicative',
  'Mood=Imp':'imperative','Mood=Sub':'subjunctive','Mood=Cnd':'conditional','VerbForm=Inf':'infinitive','VerbForm=Part':'participle',
  'VerbForm=Conv':'converb','VerbForm=Ger':'gerund','Voice=Act':'active','Voice=Pass':'passive','Voice=Mid':'middle',
  'Aspect=Imp':'imperfective','Aspect=Perf':'perfective','Degree=Cmp':'comparative','Degree=Sup':'superlative',
  'Definite=Def':'definite','Definite=Ind':'indefinite','PronType=Prs':'personal','PronType=Dem':'demonstrative','PronType=Art':'article',
  'PronType=Rel':'relative','PronType=Int':'interrogative','PronType=Ind':'indefinite','PronType=Neg':'negative','PronType=Tot':'total',
  'Animacy=Anim':'animate','Animacy=Inan':'inanimate','Polarity=Neg':'negation','Poss=Yes':'possessive','Reflex=Yes':'reflexive',
  'Variant=Short':'short form',
  nsubj:'subject','nsubj:pass':'passive subject',obj:'direct object',iobj:'indirect object',obl:'oblique',nmod:'nominal modifier',
  amod:'attribute',advmod:'adverbial',det:'determiner',case:'preposition',mark:'subordinator',cc:'coordinator',conj:'conjunct',cop:'copula',
  aux:'auxiliary','aux:pass':'passive auxiliary',acl:'clausal attribute','acl:relcl':'relative clause',advcl:'adverbial clause',
  ccomp:'complement clause',xcomp:'open complement',appos:'apposition','compound:prt':'separable particle',flat:'part of a name',
  vocative:'address',expl:'expletive',parataxis:'parenthetical',nummod:'numeral modifier','obl:arg':'governed oblique',root:'root of the sentence',
};
const COPY={
  ru:{here:'В этом месте',relation:'В предложении',to:'при слове',today:'Сегодня пишется',origin:'Происхождение слова'},
  en:{here:'In this place',relation:'In the sentence',to:'of',today:'Today written',origin:'Origin of the word'},
};

const el=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!=null)node.textContent=text;return node;};
const words=locale=>{const base=COPY[copyLocale(locale)];return Object.fromEntries(Object.entries(base).map(([key,value])=>[key,copyLocale(locale)==='en'?localeText(value,locale):value]));};
const label=(key,locale)=>{const en=copyLocale(locale)==='en';const value=(en?EN:RU)[key];return value?(en?localeText(value,locale):value):null;};

/** Human words for a part of speech and UD features: "существительное · винительный падеж, единственное число". */
export function grammarLabel(upos,feats,locale){
  const parts=String(feats||'').split('|').filter(Boolean).map(item=>label(item,locale)).filter(Boolean);
  return [label(upos,locale)||upos,parts.join(', ')].filter(Boolean).join(' · ');
}

export function relationLabel(deprel,locale){
  return label(deprel,locale)||label(String(deprel).split(':')[0],locale)||deprel;
}

/** Render the word layer's reading of the selected word; `onOrigin(word)` opens its origin. */
export function renderWordContext(parent,context,{locale='ru',onOrigin}={}){
  if(!context?.words?.length)return;
  const copy=words(locale);
  const section=el('section','cr-word-context');
  section.append(el('h3','cr-philology-heading',copy.here));
  for(const word of context.words){
    const card=el('article','cr-word-context-word');
    const head=el('p','cr-word-context-lemma');
    head.append(el('strong','',word.lemma),document.createTextNode(` · ${grammarLabel(word.upos,word.feats,locale)}`));
    card.append(head);
    if(word.spelling)card.append(el('p','',`${copy.today}: ${word.spelling.modern}`));
    if(word.deprel&&word.deprel!=='punct'){
      const relation=relationLabel(word.deprel,locale);
      card.append(el('p','',word.headText&&word.deprel!=='root'?`${copy.relation}: ${relation} ${copy.to} «${word.headText}»`:`${copy.relation}: ${relation}`));
    }
    if(typeof onOrigin==='function'){
      const button=el('button','cr-word-origin-open',copy.origin);button.type='button';
      button.addEventListener('click',()=>onOrigin(word,context));card.append(button);
    }
    section.append(card);
  }
  parent.append(section);
}
