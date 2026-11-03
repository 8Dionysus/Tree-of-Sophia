import {runResearchOperation} from './research-operations.mjs';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {citationId} from './research-index.mjs';
import jgb from '../../../../bonsai/zarathustra/research-operations.json' with {type:'json'};
import './operation-view.css';

const occurrenceFiles=import.meta.glob('../../../../bonsai/zarathustra/core/occurrences/*.json',{eager:true,import:'default'});
const occurrences=new Map(Object.values(occurrenceFiles).map(packet=>[packet.concept,packet]));
const PREFIX='tos.public-research.calculation.v1:';
const labels={
  distribution:['Употребления и распределение','Occurrences and distribution'],directed_path:['Направленный путь','Directed path'],common_grounds:['Общие основания','Common grounds'],interpretation_comparison:['Сопоставление прочтений JGB §21','Compare JGB §21 readings'],reading_distance:['Расстояние прочтений JGB §21','Distance between JGB §21 readings'],commitment_order:['Порядок обязательств JGB §21','JGB §21 commitment order'],argument_structure:['Структура рассуждения','Argument structure'],signed_support:['Поддержка точного утверждения JGB §21','Evidence for an exact JGB §21 claim'],dependency_impact:['Влияние изменения','Dependency impact'],time_place:['Время и место','Time and place']
};
const formal=new Set(['reading_distance','commitment_order','interpretation_comparison','signed_support']);
const pair=new Set(['directed_path','common_grounds','time_place']);
const viewText=(value,locale)=>localeValue(value,locale)||String(typeof value==='string'?value:'');
const idOf=(index,reference)=>{const id=typeof reference==='string'?reference:reference?.id;return index.resolveId?.(id)??id;};
const rand=()=>globalThis.crypto?.randomUUID?.()??`${Date.now()}-${Math.random().toString(36).slice(2)}`;

/** Mounts one self-contained research panel. `run` accepts the same request as the agent. */
export function mountResearchOperations({root,index,getSelection=()=>null,notebook,locale='ru',onOpen=()=>{},timeOf=()=>null,placeOf=()=>null,current=true}){
  if(!root||!index?.items)throw new TypeError('root and research index required');
  // Occurrence sets are annotated on the present core and are not kept with past revisions: a past revision counts no distributions.
  const occurrenceSet=current?occurrences:new Map();
  let last=null,disposed=false,saved=[];
  const lang=()=>resolveLocale(typeof locale==='function'?locale():locale);
  const t=(ru,en,values=[])=>localeText(lang()==='ru'?ru:en,lang(),values);
  const make=(tag,text,cls)=>{const el=document.createElement(tag);if(text!==undefined)el.textContent=text;if(cls)el.className=cls;return el;};
  const control=(caption,element)=>{const label=make('label',undefined,'research-op-field');label.append(make('span',caption),element);return label;};
  const axisName=axis=>lang()==='ru'?({f1:'Критика causa sui',f2:'Признание критики несвободной воли',f3:'Приписывание Ницше учения о несвободе',f4:'Историческая замена одного учения другим'}[axis.id]??axis.label):localeText(axis.label,lang());
  const itemName=id=>{const item=index.get(id);return item?`${viewText(item.title,lang())||item.id} · ${item.kind}`:id;};
  root.replaceChildren();root.classList.add('research-operations');
  const heading=make('h2'),help=make('p'),form=make('form'),operation=document.createElement('select'),fields=make('div',undefined,'research-op-fields'),runButton=make('button'),message=make('p',undefined,'research-op-message'),output=make('section',undefined,'research-op-output'),history=make('section',undefined,'research-op-history');
  runButton.type='submit';form.append(control('',operation),fields,runButton);root.append(heading,help,form,message,output,history);
  let state={operation:'directed_path',a:'',b:'',searchA:'',searchB:'',profileA:'0',profileB:'1',claim:'0',form:'',types:[]};
  const selection=()=>{const value=getSelection();return typeof value==='string'?value:value?.id??value?.materialId??'';};
  state.a=idOf(index,selection());
  const itemList=()=>[...index.items.values()];
  const optionsFor=slot=>{
    let items=itemList();const op=state.operation;
    if(op==='argument_structure')items=items.filter(x=>x.kind==='chain');
    if(op==='distribution')items=items.filter(x=>occurrenceSet.has(x.id));
    if(op==='directed_path'){
      const nodes=new Set(itemList().filter(x=>x.kind==='relation'&&x.record?.relation&&x.record?.from&&x.record?.to).flatMap(x=>[idOf(index,x.record.from),idOf(index,x.record.to)]));
      items=items.filter(x=>nodes.has(x.id));
    }
    const query=state[slot==='a'?'searchA':'searchB'].trim().toLocaleLowerCase();
    if(query)items=items.filter(x=>`${x.id} ${viewText(x.title,lang())}`.toLocaleLowerCase().includes(query));
    items.sort((a,b)=>(viewText(a.title,lang())||a.id).localeCompare(viewText(b.title,lang())||b.id,lang()));
    return items;
  };
  function selector(slot,caption){
    const wrap=make('div',undefined,'research-op-choice'),search=document.createElement('input'),select=document.createElement('select');
    search.type='search';search.placeholder=t('Искать по названию или ID','Search title or ID');search.value=state[slot==='a'?'searchA':'searchB'];
    select.setAttribute('aria-label',caption);
    const fill=()=>{const items=optionsFor(slot),chosen=state[slot];select.replaceChildren();
      if(chosen&&!items.some(x=>x.id===chosen)&&index.get(chosen)&&state[slot==='a'?'searchA':'searchB']){const opt=make('option',itemName(chosen));opt.value=chosen;select.append(opt);}
      for(const item of items.slice(0,200)){const opt=make('option',`${viewText(item.title,lang())||item.id} · ${item.kind}`);opt.value=item.id;select.append(opt);}
      if(!select.options.length){const empty=make('option',t('Нет подходящих предметов','No matching items'));empty.value='';select.append(empty);}
      if([...select.options].some(option=>option.value===chosen))select.value=chosen;else state[slot]=select.value;
    };
    search.addEventListener('input',()=>{state[slot==='a'?'searchA':'searchB']=search.value;fill();});select.addEventListener('change',()=>{state[slot]=select.value;if(state.operation==='distribution')renderFields();});fill();
    wrap.append(control(caption,search),select);return wrap;
  }
  function profiles(slot,caption){const select=document.createElement('select');jgb.profiles.forEach((p,i)=>{const names=[["Критика обеих крайностей","Critique of both extremes"],["Положительная несвобода","Positive unfreedom"],["Узкая критика","Narrow critique"],["Неполная разметка","Incomplete annotation"]];const opt=make('option',t(...names[i]));opt.value=String(i);select.append(opt);});select.value=state[slot];select.addEventListener('change',()=>state[slot]=select.value);return control(caption,select);}
  function renderFields(){
    fields.replaceChildren();
    if(state.operation==='signed_support'){
      const select=document.createElement('select');jgb.claims.forEach((claim,i)=>{const opt=make('option',i===0?t('p: ограниченное сравнение','p: qualified comparison'):t('q: положительное принятие несвободы','q: endorsement of unfreedom'));opt.value=String(i);select.append(opt);});select.value=state.claim;select.addEventListener('change',()=>state.claim=select.value);fields.append(control(t('Формулировка','Claim'),select));
      fields.append(make('p',t('Одна запись источника поддерживает p и ограничивает отдельное q. Это не отрицание p.','One source record supports p and limits the separate q. This is not a negation of p.'),'research-op-note'));return;
    }
    if(formal.has(state.operation)){
      fields.append(profiles('profileA',t('Первое прочтение','First reading')),profiles('profileB',t('Второе прочтение','Second reading')));
      fields.append(make('p',t('Четыре признака размечены вручную; это исследовательское предложение. null означает отсутствие разметки.','Four features are manually annotated proposals. Null means unannotated.'),'research-op-note'));return;
    }
    fields.append(selector('a',state.operation==='argument_structure'?t('Цепочка','Chain'):t('Первый предмет','First item')));
    if(pair.has(state.operation))fields.append(selector('b',t('Второй предмет','Second item')));
    if(state.operation==='distribution'){
      const input=document.createElement('input');input.value=state.form;input.placeholder=t('Все формы','All forms');input.addEventListener('input',()=>state.form=input.value);fields.append(control(t('Точная словоформа (необязательно)','Exact word form (optional)'),input));
      fields.append(make('p',t('Знаменатель — все размеченные употребления выбранного предмета, не все слова книги.','The denominator is all annotated occurrences of the selected concept, not all words in the book.'),'research-op-note'));
    }
    if(state.operation==='directed_path'){
      const types=[...new Set(itemList().filter(x=>x.kind==='relation'&&x.record?.relation).map(x=>x.record.relation))].sort();
      if(!state.types.length)state.types=[...types];
      const details=make('details',undefined,'research-op-types'),summary=make('summary'),box=make('fieldset'),legend=make('legend',t('Разрешённые типы связей','Allowed relation types'));box.append(legend);
      const updateSummary=()=>{summary.textContent=t('Типы связей: выбрано {0} из {1}','Relation types: {0} of {1} selected',[state.types.length,types.length]);};
      for(const type of types){const label=make('label'),input=document.createElement('input');input.type='checkbox';input.checked=state.types.includes(type);input.addEventListener('change',()=>{state.types=input.checked?[...state.types,type]:state.types.filter(v=>v!==type);updateSummary();});label.append(input,document.createTextNode(` ${type} `));box.append(label);}updateSummary();details.append(summary,box);fields.append(details);
    }
  }
  function buildRequest(){
    const op=state.operation,a=index.get(state.a),b=index.get(state.b);
    const versions={projection:index.revision??null,operator:'tos-research-operations@1'};
    if(formal.has(op)){
      if(op==='signed_support'){const claim=jgb.claims[Number(state.claim)];return {operation:op,scope:claim.scope,versions:{...versions,case:jgb.schema_version},input:{claim,evidence:claim.evidence,source:jgb.source}};}
      const left=jgb.profiles[Number(state.profileA)],right=jgb.profiles[Number(state.profileB)];
      if(op==='interpretation_comparison'){
        const values=p=>Object.fromEntries(jgb.axes.map((axis,i)=>[axis.id,p.values[i]]));
        return {operation:op,scope:jgb.scope,versions:{...versions,axes:jgb.profile_provenance.version},input:{axes:jgb.axes,left:values(left),right:values(right),profiles:[left,right],provenance:jgb.profile_provenance,source:jgb.source}};
      }
      return {operation:op,scope:jgb.scope,versions:{...versions,axes:jgb.profile_provenance.version},input:{axes:jgb.axes,left,right,provenance:jgb.profile_provenance,source:jgb.source}};
    }
    if(!a)throw new Error(t('Выберите первый предмет.','Choose the first item.'));
    if(pair.has(op)&&!b)throw new Error(t('Выберите второй предмет.','Choose the second item.'));
    const scope={projection:index.revision??null,selection:'research-index'},subjects=[a,b].filter(Boolean).map(x=>({id:x.id,kind:x.kind,revision:x.revision,title:x.title}));
    let input;
    if(op==='directed_path'){
      const edges=itemList().filter(x=>x.kind==='relation'&&x.record?.relation&&x.record?.from&&x.record?.to).map(x=>({id:x.id,from:idOf(index,x.record.from),to:idOf(index,x.record.to),type:x.record.relation,verb:x.record.verb,revision:x.revision}));
      input={from:a.id,to:b.id,allowed_types:state.types,edges,subjects};
    }else if(op==='common_grounds'){
      const grounds=x=>(x.dependencies??[]).map(reference=>index.get(idOf(index,reference))).filter(item=>item?.kind==='passage').map(item=>item.id);
      input={left:grounds(a),right:grounds(b),subjects};
    }else if(op==='argument_structure'){
      const steps=(a.record.steps??[]).map(step=>({...step,role:step.role??null}));
      const objections=steps.filter(step=>step.assessment?.counter_reading).map(step=>({target_step:step.id,reading:step.assessment.counter_reading}));
      input={steps,objections,subjects};
    }else if(op==='dependency_impact'){
      const dependencies=[],queue=[a.id],seen=new Set(queue);
      for(let i=0;i<queue.length;i++)for(const item of index.references(queue[i])){dependencies.push({from:queue[i],to:item.id,revision:item.revision});if(!seen.has(item.id)){seen.add(item.id);queue.push(item.id);}}
      input={changed:a.id,dependencies,subjects};
    }else if(op==='distribution'){
      const packet=occurrenceSet.get(a.id);if(!packet)throw new Error(t('Для этого предмета нет массива употреблений.','No occurrence set for this item.'));
      const query=state.form.trim().toLocaleLowerCase();
      input={witness:'DTA: German first editions',search_kind:query?'exact annotated form':'all annotated occurrences',form:state.form.trim()||null,items:packet.occurrences.map(o=>({matches:!query||o.form.toLocaleLowerCase()===query,group:o.cite?.c??'unknown',location:o.cite,form:o.form,mode:o.mode,speaker:o.speaker})),subjects};
    }else if(op==='time_place'){
      const operand=x=>{const time=timeOf(x.materialId??x.id)??null,place=placeOf(x.materialId??x.id)??null;const span=time?.span??(Number.isFinite(time?.from)&&Number.isFinite(time?.to)?time:null);return {id:x.id,span,point:place?.point??null,time_source:time,place_source:place};};input={left:operand(a),right:operand(b),subjects};
    }else throw new Error('Unknown operation');
    return {operation:op,scope,versions,input};
  }
  function openButton(id,label){const button=make('button',label??itemName(id));button.type='button';button.addEventListener('click',()=>onOpen(id));return button;}
  function show(response){
    output.replaceChildren();const result=response.result,op=response.request.operation;output.append(make('h3',t('Результат','Result')));
    const line=text=>output.append(make('p',text));
    if(op==='signed_support'){
      const states={no_evidence:t('Сведений нет','No evidence'),support:t('Есть поддержка','Support'),refutation:t('Есть опровержение','Refutation'),both:t('Есть поддержка и опровержение','Support and refutation')};line(`${states[result.status]} · β = (${result.beta.map(x=>x?'1':'0').join(', ')})`);line(lang()==='ru'?(response.request.input.claim.id===jgb.claims[0].id?'Механическая несвобода и causa sui сопоставлены как две противоположные цели критики; обе подвергаются критике.':'Ницше положительно принимает механическую несвободу.'):localeText(response.request.input.claim.statement,lang()));
      for(const evidence of [...result.positive,...result.negative])line(`${evidence.sign==='positive'?'+':'−'} ${evidence.ref}`);
    }else if(op==='reading_distance'){
      if(result.status==='incomplete')line(t('Недостаточно разметки для расстояния.','Annotation is incomplete; distance is unknown.'));
      else {line(`dH = ${result.hamming}; d2 = ${Number(result.euclidean).toFixed(3)}`);for(const d of result.differences)line(`${d.axis.id}: ${axisName(d.axis)} · ${d.left} → ${d.right}`);}
    }else if(op==='commitment_order'){
      const relations={equal:t('Наборы совпадают','Sets are equal'),left_subset:t('Первое прочтение входит во второе','First reading is a subset of the second'),right_subset:t('Второе прочтение входит в первое','Second reading is a subset of the first'),incomparable:t('Прочтения несравнимы по включению','Readings are incomparable by inclusion')};
      line(result.status==='incomplete'?t('Порядок неизвестен: не вся разметка заполнена.','Order unknown: annotations are incomplete.'):relations[result.relation]);
      if(result.status==='complete')for(const [name,items] of [[t('Только первое','Only first'),result.only_left],[t('Только второе','Only second'),result.only_right]])line(`${name}: ${items.map(axisName).join(', ')||'—'}`);
    }else if(op==='interpretation_comparison')for(const x of result.features)line(`${axisName(x.axis)}: ${x.left??'?'} / ${x.right??'?'} · ${({same:t('Совпадает','Same'),different:t('Различается','Different'),unknown:t('Неизвестно','Unknown')})[x.status]??x.status}`);
    else if(op==='directed_path'){
      line(result.found?t('Найден путь: переходов {0}, посещено узлов {1}.','Path found: {0} transitions; {1} nodes visited.',[result.transitions,result.visited_nodes]):t('Направленный путь с выбранными типами не найден.','No directed path with the selected types was found.'));
      for(const edge of result.edges)line(`${itemName(edge.from)} → ${viewText(edge.verb,lang())||edge.type} → ${itemName(edge.to)}`);
    }else if(op==='common_grounds'){
      for(const [name,ids] of [[t('Общие','Shared'),result.common],[t('Только первое','Only first'),result.only_left],[t('Только второе','Only second'),result.only_right]]){line(`${name}: ${ids.length}`);for(const id of ids)output.append(openButton(id));}
      line(`Jaccard: ${result.jaccard_distance===null?t('не измеряется','not measured'):result.jaccard_distance.toFixed(3)}`);
    }else if(op==='distribution'){
      line(t('Найдено {0} из {1} размеченных употреблений.','Found {0} of {1} annotated occurrences.',[result.count,result.denominator]));line(`${t('Свидетель','Witness')}: ${result.witness} · ${result.search_kind}`);
      for(const [chapter,count] of Object.entries(result.groups))line(`${chapter}: ${count}`);
      for(const loc of result.locations){const match=index.get(citationId(loc));if(match?.kind==='passage')output.append(openButton(match.id));else line([loc?.w,loc?.c,loc?.loc,(loc?.u??[]).join(' – '),loc?.s!==undefined||loc?.e!==undefined?`${loc.s??'?'}–${loc.e??'?'}`:''].filter(Boolean).join(' · '));}
    }else if(op==='argument_structure'){
      if(!result.steps.length)line(t('У цепочки нет записанных шагов.','This chain has no recorded steps.'));
      for(const [i,step] of result.steps.entries())line(`${i+1}. ${itemName(step.from)} → ${viewText(step.verb,lang())} → ${itemName(step.to)}${step.role?` · ${step.role}`:''}`);
      line(t('Адресных возражений: {0}.','Addressed objections: {0}.',[result.objections.length]));
      for(const objection of result.objections)line(`${objection.target_step}: ${viewText(objection.reading,lang())}`);
    }else if(op==='dependency_impact'){
      line(t('Затронуто предметов: {0}.','Affected items: {0}.',[result.affected.length]));for(const id of result.affected)output.append(openButton(id));
    }else if(op==='time_place'){
      for(const side of [result.left,result.right]){const span=side.span,point=side.point;line(`${itemName(side.id)}: ${t('интервал','interval')} ${span?`${span.from}–${span.to}${span.certainty?` · ${span.certainty}`:''}`:t('неизвестен','unknown')}; ${t('координаты','coordinates')} ${point?point.join(', '):t('неизвестны','unknown')}`);}
      line(result.time_midpoint_distance_years==null?t('Разница середин интервалов неизвестна.','Interval midpoint distance is unknown.'):t('Разница середин интервалов: {0} лет.','Interval midpoint distance: {0} years.',[result.time_midpoint_distance_years]));
      line(result.place_distance_km==null?t('Расстояние между точками неизвестно.','Great-circle distance is unknown.'):t('Расстояние между точками: {0} км.','Great-circle distance: {0} km.',[result.place_distance_km.toFixed(1)]));
    }
    const missing=result.missing??[];for(const x of missing)line(`${x.side}: ${axisName(x.axis)??x.axis.id} · ${t('нет разметки','unannotated')}`);
    const explanations={
      distribution:["Доля считается только среди переданных размеченных употреблений; поиск по форме точный.", "The proportion uses only the supplied annotated occurrences; the word-form match is exact."],
      directed_path:["Кратчайший путь следует направлению связей и выбранным типам; основание каждого перехода открывается по его связи.", "The shortest path follows relation direction and selected types; each relation opens the grounds for its transition."],
      common_grounds:["Точное пересечение ссылок на места и различия множеств. При пустом множестве расстояние не измеряется.", "Exact intersection and differences of passage references. Distance is not measured when either set is empty."],
      interpretation_comparison:["Сравниваются заданные признаки; отсутствие разметки не означает отсутствие признака.", "Declared features are compared; a missing annotation does not mean the feature is absent."],
      reading_distance:["Для равновесных бинарных признаков расстояние Хэмминга считает различия, евклидово равно корню из их числа.", "For equally weighted binary features, Hamming distance counts differences and Euclidean distance is the square root of that count."],
      commitment_order:["Сравнивается включение явно принятых обязательств; это не утверждение логического следования.", "The sets of explicitly adopted commitments are compared by inclusion; this does not assert logical entailment."],
      argument_structure:["Показаны записанные шаги и адресные возражения; недостающие логические роли не добавляются автоматически.", "Recorded steps and objections to exact steps are shown; missing logical roles are not added automatically."],
      signed_support:["Сводка явно размеченных оснований для точной формулировки и области; она не определяет истинность.", "A summary of explicitly annotated grounds for an exact formulation and scope; it does not determine truth."],
      dependency_impact:["Показаны достижимые по записанным зависимостям предметы; влияние не означает автоматической отмены.", "Items reachable through recorded dependencies are shown; impact does not mean automatic invalidation."],
      time_place:["Сравниваются середины заданных интервалов в годах и расстояние по дуге в километрах; неизвестные данные не заменяются нулями.", "Interval midpoints are compared in years and great-circle distance in kilometres; unknown data are not replaced with zeroes."]
    };line(t(...explanations[op]));
    if(formal.has(op)) {const source=response.request.input.source;if(source?.locator){const link=make('a',t('Открыть JGB §21 у свидетеля','Open JGB §21 in the witness'));link.href=source.locator;link.target='_blank';link.rel='noopener noreferrer';output.append(link);}if(source?.source_reading)line(`${t('Запись чтения','Source-reading record')}: ${source.source_reading}`);}
    const subjects=response.request.input.subjects??[];if(subjects.length){output.append(make('h4',t('Входные предметы и основания','Input items and grounds')));const ids=new Set();for(const subject of subjects){ids.add(subject.id);for(const dep of index.get(subject.id)?.dependencies??[])if(index.get(idOf(index,dep))?.kind==='passage')ids.add(idOf(index,dep));}for(const edge of result.edges??[])if(index.get(edge.id))ids.add(edge.id);for(const id of ids)output.append(openButton(id));}
    const details=make('details'),summary=make('summary',t('Воспроизводимый запрос и результат','Reproducible request and result')),pre=make('pre',JSON.stringify(response,null,2));details.append(summary,pre);output.append(details);
    const save=make('button',t('Сохранить расчёт','Save calculation'));save.type='button';save.disabled=!notebook?.setPreference;save.addEventListener('click',async()=>{try{const id=rand(),value={schema:'tos.public-research.calculation.v1',id,createdAt:new Date().toISOString(),request:response.request,result:{rule_version:response.rule_version,result:response.result,explanation:response.explanation}};await notebook.setPreference(PREFIX+id,value);message.textContent=t('Расчёт сохранён в блокноте.','Calculation saved in the notebook.');await loadSaved();}catch(error){message.textContent=error.message;}});output.append(save);
  }
  function run(request){const result=runResearchOperation(request);last=result;if(!disposed){show(result);message.textContent='';}return result;}
  async function loadSaved(){
    if(!notebook?.exportPacket){renderSaved();return;}
    try{const packet=await notebook.exportPacket();if(disposed)return;saved=packet.preferences.filter(x=>x.key.startsWith(PREFIX)&&x.value?.request&&x.value?.result).map(x=>x.value).sort((a,b)=>String(b.createdAt).localeCompare(String(a.createdAt)));renderSaved();}catch(error){if(!disposed)message.textContent=error.message;}
  }
  function renderSaved(){history.replaceChildren();history.append(make('h3',t('Сохранённые расчёты','Saved calculations')));if(!saved.length){history.append(make('p',t('Пока нет сохранённых расчётов.','No saved calculations yet.')));return;}for(const value of saved){const name=labels[value.request.operation],button=make('button',`${name?t(...name):value.request.operation} · ${value.createdAt??''}`);button.type='button';button.addEventListener('click',()=>{last={request:value.request,...value.result};show(last);message.textContent=t('Показан сохранённый результат; прежние входы сохранены.','Showing the saved result with its original inputs.');});history.append(button);}}
  function render(){
    heading.textContent=t('Вычисления над содержанием','Research operations');help.textContent=t('Выберите операцию и предметы. Результат сохраняет область, версии и основания.','Choose an operation and items. The result retains scope, versions and grounds.');
    form.firstChild.firstChild.textContent=t('Операция','Operation');operation.replaceChildren();for(const [id,label]of Object.entries(labels)){if(id==='distribution'&&!current)continue;const opt=make('option',t(...label));opt.value=id;operation.append(opt);}operation.value=state.operation;runButton.textContent=t('Вычислить','Calculate');renderFields();if(last)show(last);renderSaved();
  }
  operation.addEventListener('change',()=>{state.operation=operation.value;state.searchA='';state.searchB='';state.a=idOf(index,selection());state.b='';state.types=[];renderFields();});
  form.addEventListener('submit',event=>{event.preventDefault();try{run(buildRequest());}catch(error){message.textContent=error.message;}});
  render();void loadSaved();
  return {run,getLastResult:()=>last,refreshLocale:render,destroy(){disposed=true;root.replaceChildren();root.classList.remove('research-operations');}};
}
