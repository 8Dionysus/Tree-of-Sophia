// The site's own address: the Tree greets the reader with its bonsai to choose from. It loads only what
// it shows (the copy below, the theme and the bonsai drawn here), so it opens at once; the stands load
// their catalogs and their sky when the reader enters one.
import {BONSAI_LOCALES,initialLocale,resolveLocale} from '../../../../bonsai/lib/locale-choice.mjs';
import {SYSTEM_NAME,SYSTEM_THEME,THEMES,applyTheme,currentTheme,themeChoice} from '../public-research/site-theme.mjs';
import {STANDS} from '../public-research/stands.mjs';
import {bonsaiTree} from './home-bonsai.mjs';
import './home.css';

const LOCALE_KEY='tos.public-research.locale.v1';
const LANGUAGE_NAMES={en:'English',ru:'Русский',es:'Español',de:'Deutsch','zh-Hans':'简体中文'};
const COPY={
  en:{site:'Tree of Sophia',tagline:'Thought grown from the texts themselves. Each bonsai is a work or a whole world: in the original, in translation and on a map of its links.',choose:'Choose a bonsai',language:'Language',theme:'Theme'},
  ru:{site:'Древо Софии',tagline:'Мысль, выращенная из самих текстов. Каждый бонсай — произведение или целый мир: в оригинале, в переводах и на карте его связей.',choose:'Выберите бонсай',language:'Язык',theme:'Тема'},
  es:{site:'Árbol de Sofía',tagline:'Pensamiento que crece de los propios textos. Cada bonsái es una obra o un mundo entero: en el original, en traducción y en un mapa de sus vínculos.',choose:'Elige un bonsái',language:'Idioma',theme:'Tema'},
  de:{site:'Tree of Sophia',tagline:'Denken, aus den Texten selbst gewachsen. Jeder Bonsai ist ein Werk oder eine ganze Welt: im Original, in Übersetzung und auf einer Karte seiner Verbindungen.',choose:'Wählen Sie einen Bonsai',language:'Sprache',theme:'Design'},
  'zh-Hans':{site:'智慧之树',tagline:'从文本本身生长出的思想。每一株盆景都是一部作品或一个完整的世界：原文、译本，以及它的关联地图。',choose:'选择一株盆景',language:'语言',theme:'主题'},
};
const THEME_NAMES={es:{system:'Sistema',midnight:'Medianoche',noon:'Mediodía'},de:{system:'System',midnight:'Mitternacht',noon:'Mittag'},'zh-Hans':{system:'跟随系统',midnight:'午夜',noon:'正午'}};
// What each bonsai holds. A stand without its own words here still shows, by its title.
const BONSAI={
  zarathustra:{
    title:{es:'Zaratustra',de:'Zarathustra','zh-Hans':'查拉图斯特拉'},
    meta:{ru:'Фридрих Ницше · 1883–1885',en:'Friedrich Nietzsche · 1883–1885',es:'Friedrich Nietzsche · 1883–1885',de:'Friedrich Nietzsche · 1883–1885','zh-Hans':'弗里德里希·尼采 · 1883–1885'},
    description:{
      ru:'Немецкий оригинал, переводы Антоновского и Коммона и карта книги: её понятия, фигуры, связи и маршруты чтения.',
      en:'The German original, Antonovsky’s and Common’s translations, and a map of the book: its concepts, figures, links and reading routes.',
      es:'El original alemán, las traducciones de Antonovski y Common y un mapa del libro: sus conceptos, figuras, vínculos y rutas de lectura.',
      de:'Das deutsche Original, die Übersetzungen von Antonowski und Common und eine Karte des Buches: seine Begriffe, Gestalten, Verbindungen und Lesewege.',
      'zh-Hans':'德文原文、安东诺夫斯基与科蒙的译本，以及全书的地图：它的概念、形象、关联与阅读路线。',
    },
  },
  antiquity:{
    title:{es:'Antigüedad',de:'Antike','zh-Hans':'古代'},
    meta:{ru:'Платон, Аристотель, стоики, Эпикур, Цицерон, Сенека, Секст',en:'Plato, Aristotle, the Stoics, Epicurus, Cicero, Seneca, Sextus',es:'Platón, Aristóteles, los estoicos, Epicuro, Cicerón, Séneca, Sexto',de:'Platon, Aristoteles, die Stoiker, Epikur, Cicero, Seneca, Sextus','zh-Hans':'柏拉图、亚里士多德、斯多亚派、伊壁鸠鲁、西塞罗、塞涅卡、塞克斯都'},
    description:{
      ru:'Вопросы, которые проходят сквозь тексты и голоса: места в оригинале, их переводы и прочтения.',
      en:'Questions that run through texts and voices: places in the original, their translations and readings.',
      es:'Preguntas que atraviesan textos y voces: pasajes en el original, sus traducciones y lecturas.',
      de:'Fragen, die durch Texte und Stimmen gehen: Stellen im Original, ihre Übersetzungen und Lesarten.',
      'zh-Hans':'贯穿诸多文本与声音的问题：原文段落、译本及其解读。',
    },
  },
};

const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};

const query=new URLSearchParams(location.search);
let saved;try{saved=localStorage.getItem(LOCALE_KEY);}catch{}
let locale=initialLocale({requested:query.get('lang'),saved,system:navigator.languages??[navigator.language]});
// In development the stands live under the build's base; the site serves them at its root.
const base=location.pathname.endsWith('/home/')?location.pathname.slice(0,-'home/'.length):'/';
const copy=()=>COPY[locale]??COPY.en;
const say=(value,fallback='')=>value?.[locale]??value?.en??fallback;
const standTitle=stand=>BONSAI[stand.id]?.title?.[locale]??stand.title[locale==='ru'?'ru':'en'];

const bar=document.querySelector('.home-bar'),head=document.querySelector('.home-head'),choice=document.querySelector('.home-choice');
const language=make('select','home-select'),theme=make('select','home-select');
for(const value of BONSAI_LOCALES){const option=make('option','',LANGUAGE_NAMES[value]);option.value=value;option.lang=value;language.append(option);}
for(const value of [{id:SYSTEM_THEME},...THEMES]){const option=make('option');option.value=value.id;theme.append(option);}
bar.replaceChildren(language,theme);
const mark=make('h1','home-mark'),tagline=make('p','home-tagline'),choose=make('h2','home-choose');choose.id='home-choose';
head.replaceChildren(mark,tagline);
const list=make('ul','home-stands');
const cards=STANDS.map((stand,index)=>{
  const item=make('li','home-stand'),link=make('a'),title=make('span','home-title'),meta=make('span','home-meta'),description=make('span','home-desc');
  item.style.setProperty('--i',index);link.href=`${base}${stand.id}/`;link.append(bonsaiTree(stand.id,index),title,meta,description);item.append(link);list.append(item);
  // The sky between pages (public/cosmos.js) comes up before the bonsai is reached, so the way there leads through it.
  link.addEventListener('click',event=>{
    if(event.defaultPrevented||event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||event.altKey||!window.tosCosmos)return;
    event.preventDefault();
    void window.tosCosmos.show({stand:stand.id,title:title.textContent}).then(()=>location.assign(link.href));
  });
  return {stand,title,meta,description};
});
choice.replaceChildren(choose,list);

function render(){
  const words=copy();
  document.documentElement.lang=locale==='zh-Hans'?'zh':locale;document.title=words.site;
  mark.textContent=words.site;tagline.textContent=words.tagline;choose.textContent=words.choose;
  language.value=locale;language.setAttribute('aria-label',words.language);
  theme.setAttribute('aria-label',words.theme);
  for(const option of theme.options){const named=THEMES.find(item=>item.id===option.value)??{id:SYSTEM_THEME,name:SYSTEM_NAME};option.textContent=THEME_NAMES[locale]?.[named.id]??named.name[locale==='ru'?'ru':'en'];}
  theme.value=themeChoice();
  for(const {stand,title,meta,description} of cards){title.textContent=standTitle(stand);meta.textContent=say(BONSAI[stand.id]?.meta);meta.hidden=!meta.textContent;description.textContent=say(BONSAI[stand.id]?.description);description.hidden=!description.textContent;}
}
language.addEventListener('change',()=>{locale=resolveLocale(language.value);try{localStorage.setItem(LOCALE_KEY,locale);}catch{}render();});
theme.addEventListener('change',()=>{applyTheme(theme.value);});
applyTheme(currentTheme(),{save:false});
render();
document.documentElement.dataset.ready='true';
