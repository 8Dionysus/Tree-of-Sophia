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
  en:{site:'Tree of Sophia',tagline:'Thought grown from the sources. Each bonsai cultivates its own direction.',choose:'Choose a bonsai',language:'Language',theme:'Theme'},
  ru:{site:'Древо Софии',tagline:'Мысль, выращенная из источников. Каждый бонсай культивирует своё направление.',choose:'Выберите бонсай',language:'Язык',theme:'Тема'},
  es:{site:'Árbol de Sofía',tagline:'Pensamiento que crece de las fuentes. Cada bonsái cultiva su propia corriente.',choose:'Elige un bonsái',language:'Idioma',theme:'Tema'},
  de:{site:'Tree of Sophia',tagline:'Denken, aus den Quellen gewachsen. Jeder Bonsai kultiviert seine eigene Richtung.',choose:'Wählen Sie einen Bonsai',language:'Sprache',theme:'Design'},
  'zh-Hans':{site:'智慧之树',tagline:'从原典中生长出的思想。每一株盆景都培育着自己的方向。',choose:'选择一株盆景',language:'语言',theme:'主题'},
};
const THEME_NAMES={es:{system:'Sistema',midnight:'Medianoche',noon:'Mediodía'},de:{system:'System',midnight:'Mitternacht',noon:'Mittag'},'zh-Hans':{system:'跟随系统',midnight:'午夜',noon:'正午'}};
// What each bonsai is: the work it grows, or the era. A stand without its own words here still shows, by its title.
const BONSAI={
  zarathustra:{
    title:{es:'Zaratustra',de:'Zarathustra','zh-Hans':'查拉图斯特拉'},
    description:{ru:'«Так говорил Заратустра» Фридриха Ницше',en:'Thus Spoke Zarathustra by Friedrich Nietzsche',es:'Así habló Zaratustra, de Friedrich Nietzsche',de:'Also sprach Zarathustra von Friedrich Nietzsche','zh-Hans':'弗里德里希·尼采的《查拉图斯特拉如是说》'},
  },
  aesthetics:{
    title:{es:'Estética',de:'Ästhetik','zh-Hans':'美学'},
    description:{ru:'Философия прекрасного, искусства и чувственного опыта',en:'The philosophy of beauty, art and sensory experience',es:'La filosofía de la belleza, el arte y la experiencia sensible',de:'Die Philosophie des Schönen, der Kunst und der sinnlichen Erfahrung','zh-Hans':'关于美、艺术与感性经验的哲学'},
  },
  antiquity:{
    title:{es:'Antigüedad',de:'Antike','zh-Hans':'古代'},
    description:{ru:'Философия Древней Греции и Рима',en:'The philosophy of ancient Greece and Rome',es:'La filosofía de la Grecia y la Roma antiguas',de:'Die Philosophie des alten Griechenlands und Roms','zh-Hans':'古希腊与古罗马的哲学'},
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
  item.style.setProperty('--i',index);link.href=`${base}${stand.id}/`;link.append(...(stand.id==='aesthetics'?[]:[bonsaiTree(stand.id,index)]),title,meta,description);item.append(link);list.append(item);
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
  mark.textContent=words.site;tagline.replaceChildren(...words.tagline.split(/(?<=[.。])\s*(?=\S)/).flatMap((line,index)=>index?[' ',make('span','',line)]:[make('span','',line)]));choose.textContent=words.choose;
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
