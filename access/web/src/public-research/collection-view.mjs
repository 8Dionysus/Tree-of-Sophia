const el=(tag,cls='',value)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
const localized=(value,language)=>typeof value==='string'?value:(value&&typeof value==='object'?(value[language]??value.en??value.ru??''):'');
const searchKey=value=>String(value??'').normalize('NFD').replace(/\p{M}/gu,'').toLocaleLowerCase().replaceAll('ς','σ');
const labels={
  ru:{overview:'Общий взгляд',era:'Эпохи',people:'Авторы и школы',work:'Произведения',concept:'Понятия',translation:'Переводы',all:'Все виды',period:'Все эпохи',search:'Искать в библиотеке…',none:'Совпадений нет.',previous:'Назад',next:'Далее',back:'К библиотеке',related:'Связанные материалы',add:'Добавить в пространство',added:'Открыть в пространстве',map:'Карта произведения',steps:'Шаги чтения',argument:'Ход мысли',question:'Вопрос',sources:'Указатели на источник',missing:'Текст здесь не представлен; доступна запись и её указатели.',kind:{era:'Эпоха',author:'Автор',school:'Школа',work:'Произведение',concept:'Понятие',translation:'Перевод',original:'Оригинал',edition:'Издание',passage:'Фрагмент',question:'Вопрос',fragment:'Фрагмент',chapter:'Глава',part:'Часть',figure:'Мыслитель',interpretation:'Прочтение',dossier:'Досье',symbol:'Образ',tradition:'Традиция',note:'Заметка'}},
  en:{overview:'Overview',era:'Eras',people:'Authors and schools',work:'Works',concept:'Concepts',translation:'Translations',all:'All kinds',period:'All eras',search:'Search the library…',none:'No matches.',previous:'Back',next:'Next',back:'Back to library',related:'Related materials',add:'Add to the space',added:'Open in the space',map:'Work map',steps:'Reading steps',argument:'The argument',question:'Question',sources:'Source references',missing:'The text is not supplied here; the record and its references are available.',kind:{era:'Era',author:'Author',school:'School',work:'Work',concept:'Concept',translation:'Translation',original:'Original',edition:'Edition',passage:'Passage',question:'Question',fragment:'Passage',chapter:'Chapter',part:'Part',figure:'Thinker',interpretation:'Reading',dossier:'Dossier',symbol:'Image',tradition:'Tradition',note:'Note'}}
};
const groups={overview:null,era:['era'],people:['author','school','figure','tradition','translator'],work:['work'],concept:['concept'],translation:['translation','original','edition'],words:['concept']};
export const matchesCollectionKind=(filter,kind)=>!filter||(filter==='@passages'?kind==='fragment'||kind==='passage':kind===filter);
labels.ru.words='Слова и значения';labels.en.words='Words and meanings';labels.ru.fullTexts='Полные тексты';labels.en.fullTexts='Full texts';
labels.ru.kind.translator='Переводчик';labels.en.kind.translator='Translator';
labels.ru.kind.character='Действующее лицо';labels.en.kind.character='Character';labels.ru.kind.excerpt='Выдержка';labels.en.kind.excerpt='Excerpt';
labels.ru.passages='Фрагменты и места';labels.en.passages='Fragments and passages';
const roleNames={premise:['Основание','Premise'],conclusion:['Вывод','Conclusion'],objection:['Возражение','Objection'],reply:['Ответ','Reply'],distinction:['Различение','Distinction'],example:['Пример','Example'],question:['Вопрос','Question'],inference:['Переход','Inference'],claim:['Тезис','Claim'],condition:['Условие','Condition'],definition:['Определение','Definition'],problem:['Затруднение','Problem'],explanation:['Объяснение','Explanation'],consequence:['Следствие','Consequence'],qualification:['Уточнение','Qualification'],test:['Проверка','Test']};

/** Read-only browsing; graph mutation and reader access are explicit owner callbacks. */
export function mountCollectionView({root,entries,locale,onOpenSource,onReadCatalog,onOpenMaterial,hasMaterial,materialInGraph}={}){
  const items=Array.isArray(entries)?entries:[];
  let group='overview',kind='',period='',query='',page=0,selected=null;
  const lang=()=>String(locale?.()??'ru').startsWith('en')?'en':'ru';
  const txt=value=>localized(value,lang());
  const named=value=>typeof value==='string'?value:txt(value?.title??value?.name??value?.label??value);
  const matchEntry=value=>items.find(item=>item.id===value||item.legacyId===value||item.materialId===value)||items.find(item=>txt(item.title)===named(value));
  const addText=(parent,value,cls='')=>{const text=txt(value);if(text)parent.append(el('p',cls,text));};
  function renderStructured(parent,value){
    const values=Array.isArray(value)?value:[value];
    for(const item of values){
      if(!item)continue;
      if(typeof item==='string'||item.ru||item.en){addText(parent,item);continue;}
      if(typeof item!=='object')continue;
      const section=el('section','ps-collection-structured');
      if(item.role)section.append(el('small','',roleNames[item.role]?.[lang()==='en'?1:0]??item.role));
      const title=named(item.title??item.name??item.label);if(title)section.append(el('h4','',title));
      for(const key of ['body','reading','description','summary','text','claim','content'])if(item[key])addText(section,item[key]);
      if(txt(item.question)){section.append(el('small','',labels[lang()].question));addText(section,item.question);}
      for(const key of ['stages','steps','readings','items','links','returns','continuations'])if(item[key])renderStructured(section,item[key]);
      for(const id of new Set([...(item.parts??[]),...(item.part?[item.part]:[]),...(typeof item.from==='string'?[item.from]:[]),...(typeof item.to==='string'?[item.to]:[])]))linked(section,id);
      if(section.childNodes.length)parent.append(section);
    }
  }
  function linked(parent,value){
    const target=matchEntry(value);if(!target||target===selected)return;
    const b=button(txt(target.title),'text-button');b.addEventListener('click',()=>{selected=target;render();});parent.append(b);
  }
  function details(item){
    const l=labels[lang()],back=button(`← ${l.back}`,'text-button');back.addEventListener('click',()=>{selected=null;render();});root.append(back);
    root.append(el('p','eyebrow',l.kind[item.kind]??item.kind??''),el('h2','ps-collection-title',txt(item.title)));
    addText(root,item.meta,'ps-collection-meta');addText(root,item.body,'ps-collection-body');
    if(txt(item.voice)){root.append(el('h3','minor-title',lang()==='en'?'Voice':'Голос'));addText(root,item.voice);}
    if(txt(item.question)){root.append(el('h3','minor-title',l.question));addText(root,item.question);}
    if(item.positions)renderStructured(root,item.positions.map(position=>({...position,parts:[position.passage]})));
    for(const [key,title] of [['workMap',l.map],['readingSteps',l.steps],['argument',l.argument]])if(item[key]){root.append(el('h3','minor-title',title));renderStructured(root,item[key]);}
    const source=onOpenSource?.(item);if(source)root.append(source);else if(item.kind==='work'||item.kind==='passage'||item.sourceRefs?.length)root.append(el('p','ps-collection-meta',l.missing));
    if(item.sourceRefs?.length){root.append(el('h3','minor-title',l.sources));for(const ref of item.sourceRefs){const url=typeof ref==='string'?ref:ref.url??ref.ref;const name=typeof ref==='string'?ref:txt(ref.label??ref.title) || url;if(typeof url==='string'&&/^https?:\/\//.test(url)){const a=el('a','source-link',`${name} ↗`);a.href=url;a.target='_blank';a.rel='noopener noreferrer';root.append(a);}else if(name)root.append(el('p','ps-collection-meta',name));}}
    const relations=el('div','ps-collection-links');for(const value of [item.author,item.school,item.workId,...(item.concepts??[]),...(item.relatedIds??[])])if(value)linked(relations,value);
    if(relations.childElementCount){root.append(el('h3','minor-title',l.related),relations);}
    const materialId=item.materialId??item.id;if(hasMaterial?.(materialId)){const add=button(materialInGraph?.(materialId)?l.added:l.add,'primary');add.addEventListener('click',()=>onOpenMaterial?.(materialId));root.append(add);}
  }
  function render(){
    root.replaceChildren();const l=labels[lang()];if(selected){details(selected);return;}
    const tabs=el('nav','ps-collection-tabs');tabs.setAttribute('aria-label',l.overview);
    for(const key of Object.keys(groups)){if(key==='words'&&!items.some(item=>item.lexical))continue;const b=button(l[key],'text-button');b.setAttribute('aria-pressed',String(group===key));b.addEventListener('click',()=>{group=key;kind='';page=0;render();});tabs.append(b);}if(onReadCatalog){const read=button(l.fullTexts,'primary');read.addEventListener('click',onReadCatalog);tabs.append(read);}root.append(tabs);
    const controls=el('div','ps-collection-controls');const input=document.createElement('input');input.type='search';input.placeholder=l.search;input.setAttribute('aria-label',l.search);input.value=query;input.addEventListener('input',()=>{query=input.value;page=0;renderResults();});
    const kindSelect=document.createElement('select');kindSelect.setAttribute('aria-label',l.all);kindSelect.append(new Option(l.all,''));const kinds=[...new Set(items.map(item=>item.kind).filter(Boolean))].sort();const bothPassages=kinds.includes('passage')&&kinds.includes('fragment');for(const value of kinds){if(bothPassages&&value==='fragment'){kindSelect.append(new Option(l.passages,'@passages'));continue;}if(bothPassages&&value==='passage')continue;kindSelect.append(new Option(l.kind[value]??value,value));}kindSelect.value=kind;kindSelect.addEventListener('change',()=>{kind=kindSelect.value;page=0;renderResults();});
    const periodSelect=document.createElement('select');periodSelect.setAttribute('aria-label',l.period);periodSelect.append(new Option(l.period,''));const periods=new Map();for(const item of items){const value=txt(item.period);if(value)periods.set(value,value);}for(const value of [...periods.keys()].sort())periodSelect.append(new Option(value,value));periodSelect.value=period;periodSelect.addEventListener('change',()=>{period=periodSelect.value;page=0;renderResults();});controls.append(input,kindSelect,periodSelect);root.append(controls);
    const results=el('div','ps-collection-results');root.append(results);
    function renderResults(){
      results.replaceChildren();const q=searchKey(query.trim());const filtered=items.filter(item=>(group==='words'?item.lexical:!groups[group]||groups[group].includes(item.kind))&&matchesCollectionKind(kind,item.kind)&&(!period||txt(item.period)===period)&&(!q||[item.id,item.lemma,item.title,item.body,item.meta,item.period,item.author,item.school,item.workId,...(item.concepts??[]),...(item.aliases??[])].some(value=>searchKey(named(value)).includes(q))));
      const pages=Math.max(1,Math.ceil(filtered.length/24));page=Math.min(page,pages-1);const grid=el('div','ps-collection-grid');
      for(const item of filtered.slice(page*24,page*24+24)){const b=button('','ps-collection-card');b.append(el('small','',l.kind[item.kind]??item.kind??''),el('strong','',txt(item.title)));if(item.meta)b.append(el('span','',txt(item.meta)));b.addEventListener('click',()=>{selected=item;render();root.scrollTop=0;});grid.append(b);}if(!filtered.length)grid.append(el('p','ps-collection-meta',l.none));results.append(grid);
      if(pages>1){const nav=el('div','ps-collection-pagination'),previous=button(`← ${l.previous}`,'text-button'),next=button(`${l.next} →`,'text-button');previous.disabled=page===0;next.disabled=page===pages-1;previous.addEventListener('click',()=>{page--;renderResults();});next.addEventListener('click',()=>{page++;renderResults();});nav.append(previous,el('span','',`${page+1} / ${pages}`),next);results.append(nav);}
    }renderResults();
  }
  render();return {refresh:render};
}
