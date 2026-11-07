// The reference as a book of the tree.
//
// The book opens on its contents: the tree read from the crown to the roots beside a figure of the tree itself,
// with an index of every subject and a library of the texts. An article reads one subject: where it stands in the
// tree, what it says in the stand's own words with the places it rests on beside them, how it goes on, what it is
// linked with and by what kind of link, and the article before and after it. Every part shows only what the stand
// has authored; an empty part is left out.

import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {drawTree} from './reference-tree.mjs';
import {keepQuotesOnce} from './quote-once.mjs';
import './reference-book.css';

const W={
  contents:{ru:'Оглавление',en:'Contents'},index:{ru:'Указатель',en:'Index'},library:{ru:'Библиотека',en:'Library'},
  parts:{ru:'Части справочника',en:'Parts of the reference'},
  contentsNoteAntiquity:{ru:'Древо от кроны к корням: ветви вопросов, понятия, слова и тексты.',en:'The tree from the crown to the roots: branches of questions, concepts, words and texts.'},
  contentsNoteZarathustra:{ru:'Древо от кроны к корням: пути и вопросы, понятия, образы, фигуры и книга.',en:'The tree from the crown to the roots: paths and questions, concepts, images, figures and the book.'},
  indexNote:{ru:'Все предметы справочника по алфавиту.',en:'Every subject of the reference in alphabetical order.'},
  libraryNoteAntiquity:{ru:'Эпохи, их авторы, произведения и места, на которых стоит Древо.',en:'The eras, their authors, their works and the places the tree rests on.'},
  libraryNoteZarathustra:{ru:'Части и главы книги. Под главой стоят предметы, которые на неё опираются.',en:'The parts and chapters of the book. Under a chapter stand the subjects that rest on it.'},
  all:{ru:'Все',en:'All'},kinds:{ru:'Виды предметов',en:'Kinds of subjects'},
  openOnTree:{ru:'Открыть на Древе',en:'Open on the tree'},placeOnTree:{ru:'Место на Древе',en:'Place on the tree'},
  compare:{ru:'Сопоставить',en:'Compare'},continueWork:{ru:'Продолжить исследование',en:'Continue research'},
  moves:{ru:'Ходы ветви',en:'Moves of the branch'},steps:{ru:'Шаги пути',en:'Steps of the path'},
  reasoning:{ru:'Ход рассуждения',en:'The reasoning'},readingSteps:{ru:'Шаги чтения',en:'Reading steps'},
  openQuestion:{ru:'Остаётся открытым',en:'Left open'},
  conclusion:{ru:'К чему приходит ветвь',en:'Where the branch arrives'},openEnds:{ru:'Куда ветвь ведёт дальше',en:'Where the branch leads on'},
  branchQuestions:{ru:'Вопросы ветви',en:'Questions of the branch'},voices:{ru:'Ответы голосов',en:'Answers of the voices'},
  readingsOfPlace:{ru:'Прочтения этого места',en:'Readings of this passage'},passage:{ru:'Место',en:'The passage'},
  reading:{ru:'Прочтение',en:'Reading'},links:{ru:'Связи',en:'Links'},
  apparatus:{ru:'Источники и упоминания',en:'Sources and mentions'},sources:{ru:'Источники',en:'Sources'},mentions:{ru:'Где упоминается',en:'Where it is mentioned'},
  previous:{ru:'Предыдущая статья',en:'Previous article'},next:{ru:'Следующая статья',en:'Next article'},
  ofTotal:{ru:'{0} из {1}',en:'{0} of {1}'},
  uses:{ru:'Употребления',en:'Uses'},dictionary:{ru:'Словарь',en:'Dictionary'},otherForms:{ru:'Другие формы',en:'Other forms'},relatedWords:{ru:'Родственные слова',en:'Related words'},
  works:{ru:'Произведения',en:'Works'},places:{ru:'Места',en:'Places'},courseOfWork:{ru:'Ход произведения',en:'The course of the work'},
  concepts:{ru:'Понятия',en:'Concepts'},about:{ru:'О ком',en:'About whom'},
  argument:{ru:'Ход мысли',en:'The argument'},answer:{ru:'Ответ',en:'Answer'},otherReading:{ru:'Другое прочтение',en:'Another reading'},

  role_premise:{ru:'Посылка',en:'Premise'},role_example:{ru:'Пример',en:'Example'},role_conclusion:{ru:'Вывод',en:'Conclusion'},role_objection:{ru:'Возражение',en:'Objection'},
  role_explanation:{ru:'Объяснение',en:'Explanation'},role_claim:{ru:'Утверждение',en:'Claim'},role_definition:{ru:'Определение',en:'Definition'},role_condition:{ru:'Условие',en:'Condition'},
  role_distinction:{ru:'Различение',en:'Distinction'},role_test:{ru:'Проверка',en:'Test'},role_problem:{ru:'Трудность',en:'Problem'},role_consequence:{ru:'Следствие',en:'Consequence'},
  role_qualification:{ru:'Уточнение',en:'Qualification'},role_reply:{ru:'Ответ',en:'Reply'},role_inference:{ru:'Умозаключение',en:'Inference'},role_question:{ru:'Вопрос',en:'Question'},
};
const make=(tag,cls='',text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined&&text!==null)node.textContent=text;return node;};
// Names, paths and lists of links are read as names: the marks of terms belong to the prose.
const plainNames=node=>{node.dataset.terms='off';return node;};
const action=(text,run,cls)=>{const node=make('button',cls,text);node.type='button';node.addEventListener('click',run);return node;};
// A line that opens with the name it stands beside goes on from that name.
const afterName=(text,name)=>{if(!text||!name)return text??'';const rest=text.toLocaleLowerCase().startsWith(name.toLocaleLowerCase())?text.slice(name.length).replace(/^[\s:;,.—–-]+/u,''):text;return rest||text;};
const firstLetter=text=>String(text??'').replace(/^[\s«»“”„"'(\[]+/u,'').charAt(0).toLocaleUpperCase();

/**
 * `source` is what the stand gives the book: its outline (`book`), tree layout and index, its fragments and
 * core views, and the readers of its materials. The book returns `contents(target)` and `article(target, id)`.
 */
export function createReferenceBook(source){
  const {profile,index,book}=source;
  const locale=()=>source.locale();
  const w=(key,values=[])=>{const pair=W[key];const text=pair?(resolveLocale(locale())==='ru'?pair.ru:localeText(pair.en,locale())):key;return text.replace(/\{(\d+)\}/g,(match,at)=>at<values.length?String(values[at]):match);};
  const tr=value=>typeof value==='string'?value:localeValue(value,locale());
  const own=value=>{const text=tr(value);return typeof text==='string'?text.trim():'';};
  const title=id=>source.title(id)||'';
  const emit=(name,detail,target)=>target.dispatchEvent(new CustomEvent(name,{bubbles:true,cancelable:true,detail}));
  const link=(id,text,cls='rb-link')=>{
    const node=action(text??title(id),event=>emit('public-research-open',{id},event.currentTarget),cls);
    const line=source.glimpse(id);if(line)node.title=line;
    return node;
  };
  const sign=(id,node)=>{const mark=source.kind(id)?.sign?.();if(mark)node.prepend(mark);return node;};
  const claim=(parent,value,options={})=>value?source.fragments.render(parent,value,{grounds:true,...options}):null;
  function section(parent,key,label){
    const box=make('section','rb-section');box.dataset.part=key;box.append(make('h2','rb-section-title',label));
    const body=make('div','rb-section-body');box.append(body);parent.append(box);return body;
  }
  function field(parent,label,cls=''){
    const box=make('div',`rb-field${cls?` ${cls}`:''}`);if(label)box.append(make('p','rb-field-label',label));
    const body=make('div','rb-field-body');box.append(body);parent.append(box);return body;
  }
  const done=body=>{const box=body.parentElement;if(box&&!body.childElementCount)box.remove();};

  // ── The figure of the tree. The frontispiece draws it whole; an article draws it small with its own star lit.
  let figure=null;
  const treeFigure=(width,label)=>source.layout?drawTree(source.layout,{width,label,colorOf:source.lineColor,via:source.via}):null;
  const treeId=id=>source.treeId(id);
  function lightEntry(entry){
    if(!figure)return;
    if(!entry){figure.light([]);return;}
    const ids=book.addresses(entry).map(treeId).filter(id=>figure.has(id));
    const steps=entry.children?.filter(child=>Number.isInteger(child.n)).map(child=>treeId(child.id))??[];
    const links=steps.slice(1).map((id,at)=>[steps[at],id,'#dcc690']);
    figure.light(ids,{current:entry.id?treeId(entry.id):null,links});
  }

  // ── The contents, the index and the library.
  let tab=new URL(location.href).searchParams.get('book');if(!['contents','index','library'].includes(tab))tab='contents';
  function setTab(next,target){
    tab=next;const url=new URL(location.href);if(tab==='contents')url.searchParams.delete('book');else url.searchParams.set('book',tab);
    window.history.replaceState(window.history.state,'',url);contents(target);
  }
  let host=null;
  function contents(target,{focus=null}={}){
    host=target;target.replaceChildren();
    const root=make('div','rb-contents');root.dataset.tab=tab;target.append(root);
    const cover=make('header','rb-cover'),name=make('h1','rb-book-title',tr(source.intro?.title)),tabs=make('nav','rb-tabs');
    tabs.setAttribute('aria-label',w('parts'));
    for(const key of ['contents','index','library']){const button=action(w(key),()=>setTab(key,target),'rb-tab');button.setAttribute('aria-current',String(key===tab));tabs.append(button);}
    const note=tab==='index'?w('indexNote'):tab==='library'?w(profile==='antiquity'?'libraryNoteAntiquity':'libraryNoteZarathustra'):w(profile==='antiquity'?'contentsNoteAntiquity':'contentsNoteZarathustra');
    cover.append(name,make('p','rb-book-note',note),tabs);root.append(cover);
    const grid=make('div','rb-contents-grid'),main=make('div','rb-contents-main');grid.append(main);root.append(grid);
    // The tree stands beside the contents; an entry pointed at lights its stars in it.
    figure=null;
    if(tab!=='index'){
      figure=treeFigure(420,tr(source.intro?.title));
      if(figure){const frame=make('figure','rb-frontispiece');frame.append(figure.svg);grid.append(frame);}
    }
    if(tab==='index')renderIndex(main);else if(tab==='library')renderLibrary(main);else renderContents(main);
    main.addEventListener('pointerover',event=>{const entry=event.target.closest?.('[data-entry]');lightEntry(entry?entries.get(entry.dataset.entry):null);});
    main.addEventListener('focusin',event=>{const entry=event.target.closest?.('[data-entry]');lightEntry(entry?entries.get(entry.dataset.entry):null);});
    main.addEventListener('pointerleave',()=>lightEntry(null));
    if(focus)requestAnimationFrame(()=>main.querySelector(`[data-key="${CSS.escape(focus)}"]`)?.scrollIntoView({block:'start'}));
  }
  const entries=new Map();
  const remember=(node,entry)=>{entries.set(entry.key,entry);node.dataset.entry=entry.key;node.dataset.key=entry.key;return node;};
  function entryNode(entry,{numbered=false,brief=false}={}){
    const item=remember(make('li','rb-entry'),entry),head=make('p','rb-entry-head');
    if(numbered&&entry.n)head.append(make('span','rb-entry-n',String(entry.n)));
    head.append(entry.id?link(entry.id,tr(entry.title),'rb-entry-title'):make('span','rb-entry-title',tr(entry.title)));
    if(entry.meta&&!brief&&own(entry.meta)!==own(entry.line)&&own(entry.meta)!==own(entry.title))head.append(make('span','rb-entry-meta',tr(entry.meta)));
    item.append(head);
    if(!brief&&own(entry.line))item.append(make('p','rb-entry-line',own(entry.line)));
    return item;
  }
  function renderContents(main){
    for(const part of book.parts){
      const box=remember(make('section','rb-part'),part);box.dataset.stratum=part.stratum??'';
      box.append(make('h2','rb-part-title',tr(part.title)));
      if(part.library){
        // The roots of the tree are told in full in the library; the contents name who stands there.
        const list=make('div','rb-roots');
        for(const group of part.children){const row=make('p','rb-root');row.append(make('span','rb-root-title',tr(group.title)));const names=make('span','rb-root-names');for(const [at,item] of group.children.entries()){if(at)names.append(document.createTextNode(', '));names.append(item.id?link(item.id,tr(item.title)):make('span','',tr(item.title)));}if(group.children.length)row.append(names);list.append(row);}
        list.append(action(w('library'),()=>setTab('library',host),'rb-more'));
        box.append(list);main.append(box);continue;
      }
      if(part.flat){const list=make('ul','rb-names');for(const entry of part.children)list.append(entryNode(entry,{brief:true}));box.append(list);main.append(box);continue;}
      for(const group of part.children){
        // A region of the crown holds its branches; a path or a branch with numbered steps lists them in order.
        const isRegion=group.children?.some(child=>child.children||child.questions);
        if(isRegion){
          const region=remember(make('section','rb-region'),group);region.append(make('h3','rb-region-title',tr(group.title)));
          const list=make('ol','rb-entries');for(const entry of group.children)list.append(walkEntry(entry));region.append(list);box.append(region);
        }else if(group.children?.length&&!group.id&&!group.children.some(child=>Number.isInteger(child.n))){
          const region=remember(make('section','rb-region'),group);region.append(make('h3','rb-region-title',tr(group.title)));
          const list=make('ul','rb-names');for(const entry of group.children)list.append(entryNode(entry));region.append(list);box.append(region);
        }else{
          let list=box.querySelector(':scope > ol.rb-entries');if(!list){list=make('ol','rb-entries');box.append(list);}
          list.append(walkEntry(group));
        }
      }
      main.append(box);
    }
  }
  // A branch, a path or a subject with its own routes: its moves or steps stand under it, folded.
  function walkEntry(entry){
    const item=entryNode(entry),steps=entry.children??[],questions=entry.questions??[];
    if(!steps.length&&!questions.length)return item;
    const numbered=steps.some(step=>Number.isInteger(step.n));
    const fold=make('details','rb-entry-more'),summary=make('summary','',steps.length?(numbered?source.countSteps(steps.length):source.countRoutes(steps.length)):w('branchQuestions'));
    fold.append(summary);
    if(steps.length){const list=make('ol','rb-steps');for(const step of steps)list.append(entryNode(step,{numbered:true,brief:!step.n}));fold.append(list);}
    if(questions.length){fold.append(make('p','rb-steps-title',w('branchQuestions')));const list=make('ul','rb-steps');for(const question of questions)list.append(entryNode(question,{brief:true}));fold.append(list);}
    item.append(fold);return item;
  }
  function renderIndex(main){
    const seen=new Map();
    const take=entry=>{if(entry.id&&!seen.has(entry.id)){const name=title(entry.id)||tr(entry.title);if(name)seen.set(entry.id,{id:entry.id,name,kind:source.kind(entry.id)?.word??''});}for(const child of [...(entry.children??[]),...(entry.questions??[])])take(child);};
    for(const part of book.parts)if(!part.library)part.children.forEach(take);
    book.library.forEach(take);
    const collator=new Intl.Collator(resolveLocale(locale()));
    const all=[...seen.values()].sort((a,b)=>collator.compare(a.name.replace(/^[«“"„]/u,''),b.name.replace(/^[«“"„]/u,'')));
    const kinds=[...new Set(all.map(item=>item.kind).filter(Boolean))];
    let chosen='';
    const filter=make('div','rb-kinds');filter.setAttribute('role','group');filter.setAttribute('aria-label',w('kinds'));
    const letters=make('nav','rb-letters'),list=make('div','rb-index');
    const draw=()=>{
      list.replaceChildren();letters.replaceChildren();
      const shown=all.filter(item=>!chosen||item.kind===chosen),groups=new Map();
      for(const item of shown){const letter=firstLetter(item.name)||'·';if(!groups.has(letter))groups.set(letter,[]);groups.get(letter).push(item);}
      for(const [letter,items] of groups){
        const block=make('section','rb-index-letter');block.id=`rb-letter-${letter}`;block.append(make('h2','rb-index-mark',letter));
        const names=make('ul','rb-index-names');
        for(const item of items){const row=make('li','rb-index-row');row.append(sign(item.id,link(item.id,item.name,'rb-index-name')),make('span','rb-index-kind',item.kind));names.append(row);}
        block.append(names);list.append(block);
        const jump=action(letter,()=>block.scrollIntoView({block:'start',behavior:globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches?'auto':'smooth'}),'rb-letter');letters.append(jump);
      }
      for(const button of filter.children)button.setAttribute('aria-pressed',String((button.dataset.kind??'')===chosen));
    };
    for(const kind of ['',...kinds]){const button=action(kind||w('all'),()=>{chosen=kind;draw();},'rb-kind-chip');button.dataset.kind=kind;filter.append(button);}
    main.append(filter,letters,list);draw();
  }
  function renderLibrary(main){
    for(const group of book.library){
      const box=remember(make('section','rb-part rb-shelf'),group);
      box.append(make('h2','rb-part-title',tr(group.title)));
      if(own(group.meta))box.append(make('p','rb-part-meta',own(group.meta)));
      if(own(group.line))box.append(make('p','rb-part-line',own(group.line)));
      const list=make('ul','rb-shelf-items');
      for(const item of group.children){
        const row=remember(make('li','rb-shelf-item'),item),head=make('p','rb-entry-head');
        if(item.chapter){head.append(action(tr(item.title)?.replace(/\.$/,'')??item.chapter,()=>void source.showChapter(item.chapter),'rb-entry-title rb-chapter'));}
        else head.append(item.id?link(item.id,tr(item.title),'rb-entry-title'):make('span','rb-entry-title',tr(item.title)));
        if(own(item.meta))head.append(make('span','rb-entry-meta',own(item.meta)));
        row.append(head);
        const works=item.children??[];
        if(item.chapter&&works.length){const names=make('p','rb-shelf-subjects');for(const [at,subject] of works.entries()){if(at)names.append(document.createTextNode(', '));names.append(link(subject.id,tr(subject.title)));}row.append(names);}
        else if(works.length){
          const shelf=make('ul','rb-works');
          for(const work of works){
            const entry=remember(make('li','rb-work'),work),line=make('p','rb-entry-head');
            line.append(work.id?link(work.id,tr(work.title),'rb-work-title'):make('span','rb-work-title',tr(work.title)));if(own(work.meta))line.append(make('span','rb-entry-meta',own(work.meta)));entry.append(line);
            if(work.children?.length){const fold=make('details','rb-entry-more'),places=make('ol','rb-steps');fold.append(make('summary','',`${w('places')} · ${work.children.length}`));for(const place of work.children)places.append(entryNode(place,{brief:false}));fold.append(places);entry.append(fold);}
            shelf.append(entry);
          }
          row.append(shelf);
        }
        list.append(row);
      }
      box.append(list);main.append(box);
    }
  }

  // ── An article.
  function article(target,id){
    const item=index.get(id);if(!item)return false;
    target.classList.add('rb-article');target.dataset.kind=item.kind??'';
    trailOf(target,id);
    const head=make('header','rb-head'),text=make('div','rb-head-text'),place=make('aside','rb-place');head.append(text,place);target.append(head);
    const kind=source.kind(id),kindLine=plainNames(make('p','rb-kind'));
    sign(id,kindLine).append(make('span','rb-kind-word',kind?.word??''));
    const around=book.neighbours(id);
    if(around?.numbered&&around.total>1)kindLine.append(make('span','rb-kind-place',w('ofTotal',[around.index+1,around.total])));
    const status=source.status?.(id);if(status)kindLine.append(make('span','rb-status',status));
    text.append(kindLine,make('h1','rb-title',title(id)));
    const line=source.sourceLine(id);if(line)text.append(plainNames(make('p','rb-source',line)));
    const facts=make('div','rb-facts');source.facts(facts,id);if(facts.childElementCount)text.append(facts);
    text.append(actionsOf(id));
    // The opening — the question, the lead with its grounds, the book's own formula — runs the width of the text
    // and its margin under the head; the body follows it.
    const opening=make('div','rb-opening'),body=make('div','rb-body');
    source.body({head:opening,body,id,item,claim,section,field,link,done,w});
    text.append(opening);target.append(body);
    placeOnTree(place,id);
    // The places the lead rests on stand under the figure of the tree, in the same margin.
    const grounds=opening.querySelector('.rb-lead .rb-grounds');if(grounds)place.append(grounds);
    // A place of a source is quoted once on the page; a later ground leads back to it.
    keepQuotesOnce(target,{label:source=>`↑ ${source.split(' · ')[0]}`,later:'.rb-ground'});
    linksOf(target,id);
    apparatusOf(target,id);
    pagerOf(target,id);
    return true;
  }
  function trailOf(target,id){
    const trail=book.trail(id),nav=plainNames(make('nav','rb-trail'));nav.setAttribute('aria-label',w('contents'));
    const home=book.home(id),root=home?.source==='library'?'library':'contents';
    nav.append(action(w(root),event=>emit('public-research-contents',{tab:root},event.currentTarget),'rb-crumb'));
    for(const entry of trail){
      nav.append(make('span','rb-crumb-sep','›'));
      nav.append(entry.id?link(entry.id,tr(entry.title),'rb-crumb'):action(tr(entry.title),event=>emit('public-research-contents',{tab:root,key:entry.key},event.currentTarget),'rb-crumb'));
    }
    target.append(nav);
  }
  function placeOnTree(place,id){
    const mini=treeFigure(230,`${w('placeOnTree')}: ${title(id)}`);
    if(mini&&mini.has(treeId(id))){
      const links=source.relations(id).filter(relation=>mini.has(treeId(relation.other))).map(relation=>[treeId(id),treeId(relation.other),relation.color]);
      mini.light(links.map(link=>link[1]),{current:treeId(id),links});
      const frame=action('',event=>emit('public-research-place',{id},event.currentTarget),'rb-mini');
      frame.setAttribute('aria-label',`${w('openOnTree')}: ${title(id)}`);frame.append(mini.svg,make('span','rb-mini-caption',w('openOnTree')));place.append(frame);
    }
  }
  function actionsOf(id){
    const actions=make('div','rb-actions');
    actions.append(action(w('compare'),event=>emit('public-research-action',{action:'compare',recordId:id},event.currentTarget),'rb-action'));
    actions.append(action(w('continueWork'),event=>emit('public-research-action',{action:'continue',recordId:id},event.currentTarget),'rb-action'));
    return actions;
  }
  // Links of one kind stand together under the word, colour and line of that kind on the map.
  function linksOf(target,id){
    const relations=source.relations(id);if(!relations.length)return;
    const body=section(target,'links',w('links')),groups=new Map();
    for(const relation of relations){if(!groups.has(relation.group))groups.set(relation.group,{relation,rows:[]});groups.get(relation.group).rows.push(relation);}
    for(const {relation,rows} of groups.values()){
      const group=plainNames(make('div','rb-link-group'));group.style.setProperty('--link-color',relation.color??'#a5b7cf');group.dataset.dash=relation.dash??'solid';
      const label=make('p','rb-link-kind');label.append(make('span','rb-link-line'),make('span','',relation.word));if(relation.hint)label.title=relation.hint;group.append(label);
      const list=make('ul','rb-link-rows');
      for(const row of rows){
        const entry=make('li','rb-link-row');entry.dataset.direction=row.direction;
        entry.append(make('span','rb-link-dir',row.direction==='in'?'←':'→'),sign(row.other,link(row.other,title(row.other)||row.otherTitle,'rb-link-end')));
        const verb=afterName(row.verb,title(row.other)||row.otherTitle);
        if(verb&&verb!==relation.word)entry.append(row.relationId?action(verb,event=>emit('public-research-open',{id:row.relationId},event.currentTarget),'rb-link-verb'):make('span','rb-link-verb',verb));
        list.append(entry);
      }
      group.append(list);body.append(group);
    }
  }
  function apparatusOf(target,id){
    const box=make('div','rb-apparatus');
    source.apparatus?.(box,id);
    const body=section(box,'mentions',w('apparatus'));
    // The apparatus names what the page does not: the sources it can open, and where the subject is mentioned
    // beyond its own links.
    const refs=(source.sources?.(id)??[]).filter(ref=>ref.url);
    if(refs.length){const list=field(body,w('sources'));for(const ref of refs){const anchor=make('a','rb-source-link',ref.label);anchor.href=ref.url;anchor.target='_blank';anchor.rel='noopener noreferrer';list.append(anchor);}}
    const linked=new Set(source.relations(id).map(relation=>relation.other)),mentions=source.mentions(id).filter(other=>!linked.has(other));
    if(mentions.length){const list=field(body,w('mentions')),names=plainNames(make('p','rb-mention-list'));for(const [at,other] of mentions.entries()){if(at)names.append(document.createTextNode(', '));names.append(link(other));}list.append(names);}
    done(body);
    if(box.childElementCount)target.append(box);
  }
  function pagerOf(target,id){
    const around=book.neighbours(id);if(!around||(!around.previous&&!around.next))return;
    const nav=plainNames(make('nav','rb-pager'));nav.setAttribute('aria-label',`${w('previous')} · ${w('next')}`);
    const side=(entry,cls,label)=>{
      if(!entry)return make('span',`rb-page ${cls} is-empty`);
      const go=link(entry.id,'',`rb-page ${cls}`);go.replaceChildren(make('span','rb-page-label',label),make('span','rb-page-title',title(entry.id)||tr(entry.title)));return go;
    };
    const middle=around.parent?.id?link(around.parent.id,tr(around.parent.title),'rb-page-parent'):make('span','rb-page-parent',tr(around.parent?.title));
    nav.append(side(around.previous,'is-previous',w('previous')),middle,side(around.next,'is-next',w('next')));target.append(nav);
  }

  return {contents,article,setTab:(next,target)=>setTab(next,target),get tab(){return tab;},words:w};
}

/** The word for a role a step plays in an argument. */
export function roleWord(role,locale){const pair=W[`role_${role}`];return pair?(resolveLocale(locale)==='ru'?pair.ru:localeText(pair.en,locale)):'';}
