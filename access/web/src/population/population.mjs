// The Population stand: the authors of the Tree's corpus as its research registers of persons name them. A page of
// its own, light like the site's choice: no reader and no corpus. It loads the bonsai's population and chronology,
// and lets a reader find a member by any of its names, see when they lived, and open the register rows they grew from.
import {BONSAI_LOCALES,initialLocale,resolveLocale} from '../../../../bonsai/lib/locale-choice.mjs';
import {SYSTEM_NAME,SYSTEM_THEME,THEMES,applyTheme,currentTheme,themeChoice} from '../public-research/site-theme.mjs';
import {STANDS} from '../public-research/stands.mjs';
import {loadBonsai} from '../public-research/bonsai.mjs';
import {centuryText,displayName,filterMembers,findMember,populationModel,rowsByBatch,sortMembers,spanOf,spanText,tally} from './population-model.mjs';
import './population.css';

const LOCALE_KEY='tos.public-research.locale.v1';
const LANGUAGE_NAMES={en:'English',ru:'Русский',es:'Español',de:'Deutsch','zh-Hans':'简体中文'};
const THEME_NAMES={es:{system:'Sistema',midnight:'Medianoche',noon:'Mediodía'},de:{system:'System',midnight:'Mitternacht',noon:'Mittag'},'zh-Hans':{system:'跟随系统',midnight:'午夜',noon:'正午'}};
const PAGE=150;
const ru=new Intl.PluralRules('ru');
const ruCount=(n,[one,few,many])=>({one,few,many}[ru.select(n)]??many);

const COPY={
  ru:{site:'Древо Софии',spaces:'Бонсаи',language:'Язык',theme:'Тема',title:'Популяция',
    intro:'Авторы корпуса Древа Софии — люди, традиции и голоса, какими их называют исследовательские реестры лиц.',
    stats:({members,dated,rows,books,n})=>`${n(members)} ${ruCount(members,['автор','автора','авторов'])} корпуса · у ${n(dated)} известны годы · ${n(rows)} ${ruCount(rows,['строка','строки','строк'])} из ${n(books)} реестров`,
    posture:'Каждая запись держится слов реестров. Ни одно лицо ещё не принято в Древо: вердикт читателя не вынесен.',
    find:'Найти автора',search:'Имя на любом языке…',all:'Все',sort:'Порядок',byName:'По имени',byTime:'По времени',
    centuries:'Века',allCenturies:'все века',found:n=>`Найдено: ${n}`,more:'Показать ещё',none:'Никого не нашлось.',
    loading:'Популяция загружается…',failed:'Не удалось загрузить популяцию.',
    close:'Закрыть',life:'Годы',names:'Имена',kind:'Вид записи',verdict:'Вердикт читателя',undecided:'не вынесен',
    statuses:'Реестры отмечают',grew:'Вырос из строк реестров',register:batch=>`реестр ${batch}`,words:'Слова реестра',gaps:'Пробелы',
    phase:{lifetime:'годы жизни',activity:'деятельность'},rows:n=>`${n} ${ruCount(n,['строка','строки','строк'])}`,
    status:{verified:'проверено',provisional:'предварительно',disputed:'спорно'},
    kinds:{person:'Лица',pseudonymous:'Псевдоавторы',conventional:'Традиционные авторы','attributed-voice':'Голоса','unnamed-hand':'Безымянные',collective:'Коллективы'},
    gap:{'name:latin':'нет латинской формы имени','life:unchecked':'годы в реестре не проверены','life:unread':'запись годов ещё не прочитана','life:none':'реестр не даёт годов','life:rows-differ':'строки расходятся в годах','kind:signal':'вид записи под вопросом','identity:queue':'открыт вопрос тождества','core:ambiguous':'привязка к таблице авторов неоднозначна'}},
  en:{site:'Tree of Sophia',spaces:'Bonsai',language:'Language',theme:'Theme',title:'Population',
    intro:'The authors of the Tree of Sophia’s corpus: people, traditions and voices, as the research registers of persons name them.',
    stats:({members,dated,rows,books,n})=>`${n(members)} corpus authors · ${n(dated)} with known years · ${n(rows)} rows from ${n(books)} registers`,
    posture:'Every entry keeps to the registers’ words. No one is admitted to the Tree yet: no reader has given a verdict.',
    find:'Find an author',search:'A name in any language…',all:'All',sort:'Order',byName:'By name',byTime:'By time',
    centuries:'Centuries',allCenturies:'all centuries',found:n=>`${n} found`,more:'Show more',none:'No one found.',
    loading:'The population is loading…',failed:'The population could not be loaded.',
    close:'Close',life:'Years',names:'Names',kind:'Kind of entry',verdict:'Reader’s verdict',undecided:'not given',
    statuses:'The registers mark',grew:'Grown from register rows',register:batch=>`register ${batch}`,words:'The register’s words',gaps:'Gaps',
    phase:{lifetime:'lifetime',activity:'activity'},rows:n=>`${n} ${n===1?'row':'rows'}`,
    status:{verified:'verified',provisional:'provisional',disputed:'disputed'},
    kinds:{person:'People',pseudonymous:'Pseudonymous',conventional:'Traditional authors','attributed-voice':'Voices','unnamed-hand':'Unnamed hands',collective:'Collectives'},
    gap:{'name:latin':'no Latin form of the name','life:unchecked':'the register has not checked the years','life:unread':'the years are not read yet','life:none':'the register gives no years','life:rows-differ':'the rows differ in the years','kind:signal':'the kind of entry is in question','identity:queue':'an identity question is open','core:ambiguous':'the binding to the authors table is ambiguous'}},
  es:{site:'Árbol de Sofía',spaces:'Bonsáis',language:'Idioma',theme:'Tema',title:'Población',
    intro:'Los autores del corpus del Árbol de Sofía: personas, tradiciones y voces, tal como los nombran los registros de investigación de personas.',
    stats:({members,dated,rows,books,n})=>`${n(members)} autores del corpus · ${n(dated)} con años conocidos · ${n(rows)} filas de ${n(books)} registros`,
    posture:'Cada entrada se atiene a las palabras de los registros. Nadie ha sido admitido aún en el Árbol: ningún lector ha dado su veredicto.',
    find:'Buscar un autor',search:'Un nombre en cualquier idioma…',all:'Todos',sort:'Orden',byName:'Por nombre',byTime:'Por época',
    centuries:'Siglos',allCenturies:'todos los siglos',found:n=>`${n} encontrados`,more:'Mostrar más',none:'No se encontró a nadie.',
    loading:'La población se está cargando…',failed:'No se pudo cargar la población.',
    close:'Cerrar',life:'Años',names:'Nombres',kind:'Tipo de entrada',verdict:'Veredicto del lector',undecided:'sin dictar',
    statuses:'Los registros marcan',grew:'Surgido de filas de registros',register:batch=>`registro ${batch}`,words:'Palabras del registro',gaps:'Lagunas',
    phase:{lifetime:'vida',activity:'actividad'},rows:n=>`${n} ${n===1?'fila':'filas'}`,
    status:{verified:'verificado',provisional:'provisional',disputed:'discutido'},
    kinds:{person:'Personas',pseudonymous:'Seudónimos',conventional:'Autores tradicionales','attributed-voice':'Voces','unnamed-hand':'Manos anónimas',collective:'Colectivos'},
    gap:{'name:latin':'sin forma latina del nombre','life:unchecked':'el registro no ha comprobado los años','life:unread':'los años aún no se han leído','life:none':'el registro no da años','life:rows-differ':'las filas difieren en los años','kind:signal':'el tipo de entrada está en duda','identity:queue':'hay una cuestión de identidad abierta','core:ambiguous':'el vínculo con la tabla de autores es ambiguo'}},
  de:{site:'Tree of Sophia',spaces:'Bonsai',language:'Sprache',theme:'Design',title:'Population',
    intro:'Die Autoren des Korpus des Tree of Sophia: Menschen, Traditionen und Stimmen, wie die Forschungsregister der Personen sie nennen.',
    stats:({members,dated,rows,books,n})=>`${n(members)} Autoren des Korpus · ${n(dated)} mit bekannten Lebensdaten · ${n(rows)} Zeilen aus ${n(books)} Registern`,
    posture:'Jeder Eintrag hält sich an die Worte der Register. Noch ist niemand in den Baum aufgenommen: kein Leser hat ein Urteil gefällt.',
    find:'Einen Autor finden',search:'Ein Name in beliebiger Sprache…',all:'Alle',sort:'Reihenfolge',byName:'Nach Name',byTime:'Nach Zeit',
    centuries:'Jahrhunderte',allCenturies:'alle Jahrhunderte',found:n=>`${n} gefunden`,more:'Mehr zeigen',none:'Niemand gefunden.',
    loading:'Die Population wird geladen…',failed:'Die Population konnte nicht geladen werden.',
    close:'Schließen',life:'Lebensdaten',names:'Namen',kind:'Art des Eintrags',verdict:'Urteil des Lesers',undecided:'noch keines',
    statuses:'Die Register vermerken',grew:'Gewachsen aus Registerzeilen',register:batch=>`Register ${batch}`,words:'Worte des Registers',gaps:'Lücken',
    phase:{lifetime:'Lebenszeit',activity:'Wirken'},rows:n=>`${n} ${n===1?'Zeile':'Zeilen'}`,
    status:{verified:'geprüft',provisional:'vorläufig',disputed:'umstritten'},
    kinds:{person:'Personen',pseudonymous:'Pseudoautoren',conventional:'Traditionelle Autoren','attributed-voice':'Stimmen','unnamed-hand':'Ungenannte Hände',collective:'Kollektive'},
    gap:{'name:latin':'keine lateinische Namensform','life:unchecked':'das Register hat die Lebensdaten nicht geprüft','life:unread':'die Lebensdaten sind noch nicht gelesen','life:none':'das Register nennt keine Lebensdaten','life:rows-differ':'die Zeilen weichen in den Lebensdaten ab','kind:signal':'die Art des Eintrags ist fraglich','identity:queue':'eine Identitätsfrage ist offen','core:ambiguous':'die Zuordnung zur Autorentabelle ist mehrdeutig'}},
  'zh-Hans':{site:'智慧之树',spaces:'盆景',language:'语言',theme:'主题',title:'人物',
    intro:'智慧之树语料中的作者：研究性人物登记册所记载的人、传统与声音。',
    stats:({members,dated,rows,books,n})=>`语料作者${n(members)}位 · 已知年代者${n(dated)}位 · 来自${n(books)}份登记册的${n(rows)}行记录`,
    posture:'每条记录都以登记册的原话为准。尚无人被收入智慧之树：读者尚未作出裁定。',
    find:'查找作者',search:'任意语言的名字…',all:'全部',sort:'排序',byName:'按姓名',byTime:'按时间',
    centuries:'世纪',allCenturies:'全部世纪',found:n=>`找到${n}位`,more:'显示更多',none:'未找到。',
    loading:'正在加载人物…',failed:'无法加载人物。',
    close:'关闭',life:'年代',names:'名字',kind:'条目类型',verdict:'读者裁定',undecided:'尚未作出',
    statuses:'登记册标记',grew:'源自登记册条目',register:batch=>`登记册${batch}`,words:'登记册原文',gaps:'缺漏',
    phase:{lifetime:'生卒',activity:'活动'},rows:n=>`${n}行`,
    status:{verified:'已核实',provisional:'暂定',disputed:'有争议'},
    kinds:{person:'人物',pseudonymous:'托名作者',conventional:'传统作者','attributed-voice':'归属之声','unnamed-hand':'佚名之手',collective:'集体'},
    gap:{'name:latin':'缺少拉丁字母名','life:unchecked':'登记册未核对年代','life:unread':'年代尚未解读','life:none':'登记册未给出年代','life:rows-differ':'各行年代不一','kind:signal':'条目类型存疑','identity:queue':'身份问题尚待裁定','core:ambiguous':'与作者表的对应不明确'}},
};

const KIND_NAMES={
  es:{person:'persona',pseudonymous:'autor seudónimo',conventional:'identidad autoral convencional o tradicional','attributed-voice':'voz a la que una tradición atribuye un texto','unnamed-hand':'mano anónima',collective:'colectivo'},
  de:{person:'Person',pseudonymous:'Pseudoautor',conventional:'konventionelle oder traditionelle Autoridentität','attributed-voice':'Stimme, der eine Tradition einen Text zuschreibt','unnamed-hand':'ungenannte Hand',collective:'Kollektiv'},
  'zh-Hans':{person:'人物',pseudonymous:'托名作者',conventional:'约定或传统的作者身份','attributed-voice':'传统归之以文本的声音','unnamed-hand':'佚名之手',collective:'集体'},
};

const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};
const query=new URLSearchParams(location.search);
let saved;try{saved=localStorage.getItem(LOCALE_KEY);}catch{}
let locale=initialLocale({requested:query.get('lang'),saved,system:navigator.languages??[navigator.language]});
const copy=()=>COPY[locale]??COPY.en;
const number=value=>new Intl.NumberFormat(locale==='zh-Hans'?'zh':locale).format(value);
// In development the stands live under the build's base; the site serves them at its root.
const base=location.pathname.replace(/population\/?$/,'');
const homeUrl=base==='/'?'/':`${base}home/`;
const standTitle=stand=>stand.title[locale]??stand.title.en;

// The state a reader can share: the search, the kind, the century, the order and the member open.
const state={query:query.get('q')??'',kind:query.get('kind'),century:query.has('c')?Number(query.get('c')):null,by:query.get('sort')==='time'?'time':'name',open:query.get('m'),shown:PAGE};
if(!Number.isInteger(state.century))state.century=null;

// The page: the bar with the site's other bonsai, the stand's head, its tools, the list and the open member.
const root=document.querySelector('#population');
const bar=make('header','pop-bar'),wordmark=make('a','pop-wordmark'),spaces=make('nav','pop-spaces'),choices=make('div','pop-choices');
wordmark.href=homeUrl;
const language=make('select','pop-select'),theme=make('select','pop-select');
for(const value of BONSAI_LOCALES){const option=make('option','',LANGUAGE_NAMES[value]);option.value=value;option.lang=value;language.append(option);}
for(const value of [{id:SYSTEM_THEME},...THEMES]){const option=make('option');option.value=value.id;theme.append(option);}
choices.append(language,theme);
const spaceLinks=STANDS.map(stand=>{
  const node=stand.id==='population'?make('span','pop-space-current'):make('a','pop-space');
  if(stand.id==='population')node.setAttribute('aria-current','page');
  else node.addEventListener('click',event=>{
    if(event.defaultPrevented||event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||event.altKey||!window.tosCosmos)return;
    event.preventDefault();
    void window.tosCosmos.show({stand:stand.id,title:node.textContent}).then(()=>location.assign(node.href));
  });
  spaces.append(node);
  return {stand,node};
});
bar.append(wordmark,spaces,choices);

const main=make('main','pop');
const head=make('section','pop-head'),title=make('h1','pop-title'),intro=make('p','pop-intro'),stats=make('p','pop-stats'),posture=make('p','pop-posture');
head.append(title,intro,stats,posture);
const tools=make('section','pop-tools'),searchLabel=make('label','pop-find'),searchCaption=make('span','pop-find-caption'),search=make('input','pop-search');
search.type='search';search.autocomplete='off';search.spellcheck=false;search.value=state.query;
searchLabel.append(searchCaption,search);
const kinds=make('div','pop-kinds'),sortLabel=make('label','pop-sort'),sortCaption=make('span','pop-sort-caption'),sort=make('select','pop-select');
for(const value of ['name','time']){const option=make('option');option.value=value;sort.append(option);}
sort.value=state.by;sortLabel.append(sortCaption,sort);
const time=make('figure','pop-time'),timeCaption=make('figcaption','pop-time-caption'),timeLabel=make('span','pop-time-label'),timeBars=make('div','pop-centuries'),timeClear=make('button','pop-time-clear');
timeClear.type='button';timeCaption.append(timeLabel,timeClear);time.append(timeCaption,timeBars);
tools.append(searchLabel,kinds,sortLabel,time);
const body=make('div','pop-body'),listSection=make('section','pop-results'),count=make('p','pop-count'),list=make('ul','pop-list'),more=make('button','pop-more');
count.setAttribute('role','status');count.setAttribute('aria-live','polite');more.type='button';
listSection.append(count,list,more);
const detail=make('aside','pop-detail');detail.hidden=true;detail.tabIndex=-1;
body.append(listSection,detail);
main.append(head,tools,body);
root.replaceWith(bar,main);

let model=null,bonsai=null,shown=[];

function setUrl(){
  const url=new URL(location.href);
  const put=(key,value)=>{if(value===null||value===undefined||value==='')url.searchParams.delete(key);else url.searchParams.set(key,String(value));};
  put('q',state.query.trim());put('kind',state.kind);put('c',state.century);put('sort',state.by==='time'?'time':null);put('m',state.open);
  history.replaceState(null,'',url);
}

function renderShell(){
  const words=copy();
  document.documentElement.lang=locale==='zh-Hans'?'zh':locale;
  document.title=`${words.title} · ${words.site}`;
  wordmark.textContent=words.site;
  spaces.setAttribute('aria-label',words.spaces);
  for(const {stand,node} of spaceLinks){node.textContent=standTitle(stand);if(node.tagName==='A')node.href=`${base}${stand.id}/?lang=${locale}`;}
  language.value=locale;language.setAttribute('aria-label',words.language);
  theme.setAttribute('aria-label',words.theme);
  for(const option of theme.options){const named=THEMES.find(item=>item.id===option.value)??{id:SYSTEM_THEME,name:SYSTEM_NAME};option.textContent=THEME_NAMES[locale]?.[named.id]??named.name[locale==='ru'?'ru':'en'];}
  theme.value=themeChoice();
  title.textContent=words.title;intro.textContent=words.intro;posture.textContent=words.posture;
  searchCaption.textContent=words.find;search.placeholder=words.search;
  sortCaption.textContent=words.sort;sort.options[0].textContent=words.byName;sort.options[1].textContent=words.byTime;
  more.textContent=words.more;
}

function renderStats(){
  if(!model){stats.textContent='';return;}
  const rows=model.members.reduce((sum,entry)=>sum+entry.member.grew_from.length,0);
  stats.textContent=copy().stats({members:model.members.length,dated:model.members.filter(entry=>entry.span).length,rows,books:Object.keys(bonsai.population.batches??{}).length,n:number});
}

function renderKinds(){
  // The chips count what the search and the century leave, as the bars of the centuries count what the search and the kind leave.
  const words=copy(),within=filterMembers(model,{query:state.query,century:state.century}),{kinds:counts}=tally(within);
  for(const kind of tally(model.members).kinds.keys())if(!counts.has(kind))counts.set(kind,0);
  const chip=(value,label,amount)=>{
    const node=make('button','pop-kind');node.type='button';node.setAttribute('aria-pressed',String(state.kind===value));
    node.append(make('span','',label),make('span','pop-kind-count',number(amount)));
    node.addEventListener('click',()=>{state.kind=state.kind===value||value===null?null:value;state.shown=PAGE;update();});
    return node;
  };
  // The chips keep the order of the whole population, so they do not jump while the reader types.
  const order=[...tally(model.members).kinds].sort((a,b)=>b[1]-a[1]).map(([kind])=>kind);
  kinds.replaceChildren(chip(null,words.all,within.length),...order.map(kind=>chip(kind,words.kinds[kind]??kind,counts.get(kind))));
}

function renderCenturies(){
  const words=copy();
  // The bars count the members the search and the kind leave; the century itself stays open to choose.
  const {centuries}=tally(filterMembers(model,{query:state.query,kind:state.kind}));
  const all=[...tally(model.members).centuries.keys()];
  const first=Math.min(...all),last=Math.max(...all),peak=Math.max(1,...centuries.values());
  const bars=[];
  for(let century=first;century<=last;century++){
    if(century===0)continue;
    const amount=centuries.get(century)??0,label=`${centuryText(century,locale)}: ${number(amount)}`;
    const node=make('button','pop-century');node.type='button';node.title=label;node.setAttribute('aria-label',label);
    node.setAttribute('aria-pressed',String(state.century===century));
    node.style.setProperty('--h',String(amount?Math.max(.06,Math.sqrt(amount/peak)):0));
    node.disabled=!amount&&state.century!==century;
    node.addEventListener('click',()=>{state.century=state.century===century?null:century;state.shown=PAGE;update();});
    bars.push(node);
  }
  timeBars.replaceChildren(...bars);
  timeLabel.textContent=state.century===null?words.centuries:centuryText(state.century,locale);
  timeClear.textContent=words.allCenturies;
  timeClear.hidden=state.century===null;
}

function rowFor(entry){
  const words=copy(),item=make('li'),row=make('button','pop-row');row.type='button';row.dataset.id=entry.id;
  const name=make('span','pop-name',displayName(entry.member,locale));
  const other=locale==='ru'?entry.member.name.en:entry.member.name.ru;
  const second=make('span','pop-other',other&&other!==name.textContent?other:'');
  const years=make('span','pop-years',spanText(entry.span,locale));
  const meta=make('span','pop-meta',[entry.member.kind!=='person'?words.kinds[entry.member.kind]:null,words.rows(entry.member.grew_from.length)].filter(Boolean).join(' · '));
  row.append(name,second,years,meta);
  if(entry.id===state.open)row.setAttribute('aria-current','true');
  row.addEventListener('click',()=>open(entry.id));
  item.append(row);
  return item;
}

function renderList(){
  const words=copy();
  shown=sortMembers(filterMembers(model,{query:state.query,kind:state.kind,century:state.century}),{by:state.by,locale});
  count.textContent=shown.length?words.found(number(shown.length)):words.none;
  list.replaceChildren(...shown.slice(0,state.shown).map(rowFor));
  more.hidden=shown.length<=state.shown;
}

function section(label,...content){const node=make('section','pop-detail-part');node.append(make('h3','',label),...content);return node;}

function renderDetail(){
  const words=copy(),entry=state.open?findMember(model,state.open):null;
  if(!entry){detail.hidden=true;detail.replaceChildren();return;}
  const {member}=entry,chronology=bonsai.chronology;
  const close=make('button','pop-close',words.close);close.type='button';close.addEventListener('click',()=>open(null));
  const names=make('ul','pop-names');
  for(const value of new Set([member.name.ru,member.name.en,member.name.original].filter(Boolean))){const item=make('li','',value);names.append(item);}
  const life=make('ul','pop-phases');
  for(const id of member.phases){
    const situation=chronology.situations[id];if(!situation)continue;
    const phase={id,...situation},item=make('li');
    item.append(make('span','pop-phase-name',words.phase[id.endsWith('.activity')?'activity':'lifetime']),make('span','pop-phase-years',spanText(spanOf(phase),locale)));
    const wording=situation.dating.source_wording?.text;
    if(wording){const quote=make('q','pop-wording',wording);quote.lang=situation.dating.source_wording.language??'ru';item.append(make('span','pop-wording-label',words.words),quote);}
    life.append(item);
  }
  if(!life.childElementCount)life.append(make('li','',spanText(null,locale)));
  const statuses=make('p','',member.register.statuses.map(status=>words.status[status]??status).join(', '));
  const grew=make('ul','pop-batches');
  for(const {batch,rows,register} of rowsByBatch(member,bonsai.population.batches)){
    const item=make('li');
    const link=register?make('a','pop-register',words.register(batch)):make('span','pop-register',words.register(batch));
    if(register){link.href=register;link.target='_blank';link.rel='noopener';}
    item.append(link,make('span','pop-row-ids',rows.join(', ')));
    grew.append(item);
  }
  // The kind in the reader's language: the bonsai's own label in Russian and English, the stand's in the others.
  const kindName=KIND_NAMES[locale]?.[member.kind]??bonsai.population.kinds?.[member.kind]?.label?.[locale==='ru'?'ru':'en']??member.kind;
  detail.setAttribute('aria-label',displayName(member,locale));
  const parts=[
    make('h2','pop-detail-name',displayName(member,locale)),
    section(words.names,names),
    section(words.life,life),
    section(words.kind,make('p','',kindName)),
    section(words.verdict,make('p','',words.undecided)),
    section(words.statuses,statuses),
    section(`${words.grew} · ${words.rows(member.grew_from.length)}`,grew),
  ];
  if(member.gaps.length){const gaps=make('ul','pop-gaps');for(const gap of member.gaps)gaps.append(make('li','',words.gap[gap]??gap));parts.push(section(words.gaps,gaps));}
  parts.push(make('p','pop-id',entry.id));
  detail.replaceChildren(close,...parts);
  detail.hidden=false;
}

function open(id){
  const entry=id?findMember(model,id):null;
  state.open=entry?.id??null;
  for(const row of list.querySelectorAll('.pop-row'))row.toggleAttribute('aria-current',row.dataset.id===state.open);
  renderDetail();setUrl();
  if(entry){detail.focus({preventScroll:true});if(matchMedia('(max-width: 899px)').matches)detail.scrollIntoView({block:'start',behavior:'smooth'});}
}

function update(){
  if(!model)return;
  renderKinds();renderCenturies();renderList();renderDetail();setUrl();
}

function renderAll(){renderShell();if(model){renderStats();update();}}

let typing=0;
search.addEventListener('input',()=>{cancelAnimationFrame(typing);typing=requestAnimationFrame(()=>{state.query=search.value;state.shown=PAGE;update();});});
sort.addEventListener('change',()=>{state.by=sort.value;state.shown=PAGE;update();});
more.addEventListener('click',()=>{state.shown+=PAGE;renderList();});
timeClear.addEventListener('click',()=>{state.century=null;state.shown=PAGE;update();});
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&state.open)open(null);});
language.addEventListener('change',()=>{locale=resolveLocale(language.value);try{localStorage.setItem(LOCALE_KEY,locale);}catch{}renderAll();});
theme.addEventListener('change',()=>{applyTheme(theme.value);});
applyTheme(currentTheme(),{save:false});
renderShell();
count.textContent=copy().loading;

loadBonsai('population').then(loaded=>{
  bonsai=loaded;model=populationModel(loaded.population,loaded.chronology);
  // A link to a merged member's ID opens the member it joined.
  if(state.open)state.open=findMember(model,state.open)?.id??null;
  renderStats();update();
}).catch(error=>{
  console.error(error);count.textContent=copy().failed;
}).finally(()=>{
  document.documentElement.dataset.ready='true';
  requestAnimationFrame(()=>requestAnimationFrame(()=>window.tosCosmos?.done()));
});
