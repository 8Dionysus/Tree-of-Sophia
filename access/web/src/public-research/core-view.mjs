import {localeValue,resolveLocale} from './locale.mjs';

const words={
  essence:['Суть','Essence','Esencia','Wesen','本质'],development:['Развитие','Development','Desarrollo','Entwicklung','发展'],dimensions:['Измерения','Dimensions','Dimensiones','Dimensionen','维度'],links:['Связи','Links','Vínculos','Verbindungen','联系'],witnesses:['Свидетельства','Witnesses','Testimonios','Zeugnisse','文本见证'],
  thesis:['Тезис','Thesis','Tesis','These','论点'],question:['Вопрос','Question','Pregunta','Frage','问题'],answer:['Ответ','Answer','Respuesta','Antwort','回答'],grounds:['Основания','Grounds','Fundamentos','Grundlagen','依据'],consequences:['Следствия','Consequences','Consecuencias','Folgen','后果'],function:['Роль в книге','Role in the book','Papel en el libro','Rolle im Buch','在书中的作用'],
  emergence:['Возникновение','Emergence','Aparición','Entstehung','出现'],movement:['Движение','Movement','Movimiento','Bewegung','演变'],transformation:['Преображение','Transformation','Transformación','Verwandlung','转变'],
  counter:['Другой голос','Another voice','Otra voz','Eine andere Stimme','另一种声音'],reply:['Ответ этому голосу','Reply to this voice','Respuesta a esta voz','Antwort auf diese Stimme','回应'],path:['Путь связи','Path of the link','Camino del vínculo','Weg der Verbindung','联系路径'],review:['Проверка и происхождение','Review and provenance','Revisión y procedencia','Prüfung und Herkunft','审核与来源'],original:['Первоначальное предложение','Original proposal','Propuesta original','Ursprünglicher Vorschlag','原始提议'],
  reconstruction:['Реконструкция','Reconstruction','Reconstrucción','Rekonstruktion','重构'],text:['По тексту','Textual grounding','Fundamento textual','Textgrundlage','文本依据'],rejected:['Отвергнуто','Rejected','Rechazado','Verworfen','已否决'],held:['Ожидает проверки','Awaiting review','Pendiente de revisión','Prüfung ausstehend','待审核'],confirmed:['Подтверждено','Confirmed','Confirmado','Bestätigt','已确认'],revise:['Исправлено по проверке','Revised after review','Corregido tras revisión','Nach Prüfung überarbeitet','审核后修订'],keep:['Формула сохранена','Formula retained','Fórmula conservada','Formel beibehalten','保留原句'],better:['Формула уточнена','Formula refined','Fórmula precisada','Formel präzisiert','原句已调整'],
  who:['Кто говорит и действует','Who speaks and acts','Quién habla y actúa','Wer spricht und handelt','谁在言说与行动'],position:['Позиция','Position','Posición','Position','立场'],deed:['Поступок','Deed','Acción','Tat','行动'],
  names:['Слова и имена','Words and names','Palabras y nombres','Wörter und Namen','词语与名称'],uses:['Употребления','Uses','Usos','Verwendungen','用法'],beyond:['Другие произведения: ориентиры для дальнейшего исследования','Other works: leads for further research','Otras obras: pistas para investigar','Andere Werke: Hinweise für weitere Forschung','其他作品：后续研究线索'],hypothesis:['Гипотеза','Hypothesis','Hipótesis','Hypothese','假设'],occurrences:['Места в книге','Places in the book','Pasajes del libro','Stellen im Buch','书中段落'],more:['Показать ещё','Show more','Mostrar más','Mehr anzeigen','显示更多'],empty:['Этот раздел ещё не разработан.','This section is not developed yet.','Esta sección aún no está desarrollada.','Dieser Abschnitt ist noch nicht ausgearbeitet.','此部分尚未展开。'],unavailable:['Не удалось загрузить. Повторить','Could not load. Retry','No se pudo cargar. Reintentar','Laden fehlgeschlagen. Erneut versuchen','无法加载。重试'],
  narrator:['Рассказчик','Narrator','Narrador','Erzähler','叙述者'],speech:['Речь','Speech','Discurso','Rede','言说'],song:['Песня','Song','Canción','Lied','歌'],quoted:['Цитата в речи','Quoted speech','Cita en el discurso','Zitat in der Rede','话语中的引述'],narration:['Повествование','Narration','Narración','Erzählung','叙述'],dream:['Сон','Dream','Sueño','Traum','梦'],thought:['Мысль','Thought','Pensamiento','Gedanke','思想'],reported:['Переданная речь','Reported speech','Discurso referido','Wiedergegebene Rede','转述'],primary:['Прямой голос','Direct voice','Voz directa','Direkte Stimme','直接声音'],embedded:['Вложенный голос','Embedded voice','Voz insertada','Eingebettete Stimme','嵌入的声音'],imagined:['Воображаемая речь','Imagined speech','Discurso imaginado','Vorgestellte Rede','想象中的言说'],
};
const langs=['ru','en','es','de','zh-Hans'];
const el=(tag,cls='',text)=>{const node=document.createElement(tag);node.className=cls;if(text!==undefined)node.textContent=text;return node;};
const button=(text,run,cls='text-button')=>{const node=el('button',cls,text);node.type='button';node.addEventListener('click',run);return node;};
const occurrenceFiles=import.meta.glob('../../../../bonsai/zarathustra/core/occurrences/*.json',{import:'default'});

/** Every quoted word is rendered by the existing aligned-witness reader. */
export function createCoreView({core,locale,fragments,onNode,onRoute}){
  const tr=value=>localeValue(value,locale());
  const t=key=>words[key]?.[Math.max(0,langs.indexOf(resolveLocale(locale())))]??key;
  const name=id=>tr(core.records[id]?.title)||(id==='narrator'?t('narrator'):id);
  const status=(parent,value)=>{if(value)parent.append(el('small','ps-core-status',t(value)));};
  function form(parent,value){
    if(!value)return;
    if(Array.isArray(value)){for(const item of value)form(parent,item);return;}
    const normalized=value.cite&&!Array.isArray(value.cite)?{...value,cite:[value.cite]}:value;
    fragments.render(parent,normalized);
    status(parent,value.status);
  }
  function section(parent,key,value){
    if(!value||(Array.isArray(value)&&!value.length))return;
    const box=el('section','ps-core-section');box.append(el('h3','minor-title',t(key)));form(box,value);parent.append(box);
  }
  function fold(parent,label,render){
    const box=el('details','ps-core-fold'),body=el('div');box.append(el('summary','',label),body);let loaded=false;
    box.addEventListener('toggle',()=>{if(box.open&&!loaded){loaded=true;render(body);}});parent.append(box);return box;
  }
  function voice(parent,value){
    if(!value)return;
    const parts=[name(value.speaker),value.mode&&t(value.mode),value.layer&&t(value.layer),value.via&&name(value.via)].filter(Boolean);
    parent.append(el('p','ps-core-voice',parts.join(' · ')));
  }
  function original(parent,value){
    if(!value)return;
    if(value.cite)form(parent,{cite:Array.isArray(value.cite)?value.cite:[value.cite]});
    for(const key of ['verb','label','sense','why'])if(value[key])form(parent,value[key]);
    for(const cite of value.grounds??[])parent.append(fragments.fragment(cite));
    voice(parent,value.voice);
  }
  function review(parent,record){
    if(!record.assessment&&!record.proposed)return;
    fold(parent,t('review'),body=>{
      const a=record.assessment??{};status(body,a.verdict);
      if(a.reason)form(body,a.reason);
      if(a.reviewer||record.maker)body.append(el('p','muted',[record.maker,a.reviewer].filter(Boolean).join(' → ')));
      if(a.counter_reading){section(body,'counter',a.counter_reading);for(const cite of a.counter_reading.grounds??[])body.append(fragments.fragment(cite));}
      if(record.proposed)fold(body,t('original'),box=>original(box,record.proposed));
    });
  }
  function formula(parent,item){
    if(!item)return;
    const box=el('section','ps-core-formula');voice(box,item.voice??{speaker:item.speaker});
    if(item.cite)box.append(fragments.fragment(item.cite));
    if(item.why)form(box,item.why);review(box,item);parent.append(box);
  }
  function link(parent,record){
    const box=el('section','ps-core-link'),ends=el('div','edge-endpoints');
    ends.append(button(name(record.from),()=>onNode(core.materialIds[record.from])),el('span','',tr(record.verb)),button(name(record.to),()=>onNode(core.materialIds[record.to])));
    box.append(ends);status(box,record.admission==='rejected'?'rejected':record.status);
    form(box,{...record.sense,cite:[...new Map([...(record.cite?[record.cite]:[]),...(record.grounds??[])].map(cite=>[JSON.stringify(cite),cite])).values()]});
    section(box,'question',record.question);section(box,'answer',record.answer);
    if(record.path?.length){
      const path=el('section','ps-core-path');path.append(el('h3','minor-title',t('path')));
      const endpoint=id=>core.materialIds[id]?button(name(id),()=>onNode(core.materialIds[id])):el('span','',name(id));
      for(const step of record.path){
        const row=el('div','ps-core-path-step');
        if(step.id)row.append(endpoint(step.id));
        else if(step.from&&step.to)row.append(endpoint(step.from),el('span','',tr(step.verb)),endpoint(step.to));
        for(const cite of step.cite?(Array.isArray(step.cite)?step.cite:[step.cite]):[])row.append(fragments.fragment(cite));
        status(row,step.status);path.append(row);
      }
      box.append(path);
    }
    review(box,record);parent.append(box);
  }
  function witnesses(parent,record){
    for(const item of record.appearances??[])fold(parent,`${item.cite.c} · ${tr(item.line)}`,box=>form(box,{...item.line,cite:[item.cite],status:item.status}));
    for(const item of record.voice??[])fold(parent,`${item.cite.c} · ${name(item.speaker)} · ${tr(item.line)}`,box=>{voice(box,item);form(box,{...item.line,cite:[item.cite],status:item.status});});
    for(const item of record.conception?.formulas??[])fold(parent,`${item.cite.c} · ${tr(item.why)}`,box=>formula(box,item));
    for(const item of record.questions??[])fold(parent,tr(item.question),box=>{form(box,item.answer);status(box,item.status);});
    const names=record.lexicon??record.names;
    if(names)fold(parent,t('names'),box=>{for(const [language,items]of Object.entries(names)){if(!['de','ru','en'].includes(language)||!Array.isArray(items))continue;box.append(el('h3','minor-title',language.toUpperCase()));box.append(el('p','',items.filter(item=>typeof item?.form==='string').map(item=>`${item.form} (${item.count})`).join(' · ')));}});
    if(record.uses?.length)fold(parent,t('uses'),box=>{for(const item of record.uses)box.append(el('p','',`${tr(item.name)} · ${item.count}`));});
    if(record.beyond?.length)fold(parent,t('beyond'),box=>{for(const item of record.beyond){const entry=el('section','ps-core-section');entry.append(el('h3','minor-title',[item.work,item.place].filter(Boolean).join(' · ')));form(entry,item.note);status(entry,item.status);box.append(entry);}});
    const loader=occurrenceFiles[`../../../../bonsai/zarathustra/core/occurrences/${record.id}.json`];
    if(loader)fold(parent,t('occurrences'),box=>{
      const load=async()=>{try{
        const data=await loader(),items=Array.isArray(data)?data:data.occurrences??[];box.replaceChildren();let offset=0;
        const more=button(t('more'),()=>page());
        const page=()=>{more.remove();for(const item of items.slice(offset,offset+20))fold(box,`${item.cite.c} · ${name(item.speaker)} · ${item.form??''}`,body=>{voice(body,item);form(body,{...item.line,cite:[item.cite]});});offset+=20;if(offset<items.length)box.append(more);};page();
      }catch{box.replaceChildren(button(t('unavailable'),load));}};void load();
    });
    if(record.cite)form(parent,{cite:[record.cite]});
  }
  function node(parent,id){
    const record=core.records[id];if(!record)return;
    const essence=record.conception??record.essence??{};
    const root=el('div','ps-core-card');root.dataset.coreId=id;parent.append(root);
    formula(root,essence.formula);
    section(root,'question',record.question??essence.question);
    if(!record.conception&&!record.essence&&!record.chainId)status(root,'held');
    const tabs=el('div','ps-core-tabs'),body=el('section','ps-core-pane');tabs.setAttribute('role','tablist');tabs.setAttribute('aria-label',name(id));body.setAttribute('role','tabpanel');
    const keys=['essence','development','dimensions','links','witnesses'];
    const draw=key=>{
      body.replaceChildren();
      for(const tab of tabs.children){const chosen=tab.dataset.tab===key;tab.setAttribute('aria-selected',String(chosen));tab.tabIndex=chosen?0:-1;}
      body.setAttribute('aria-labelledby',`core-tab-${id}-${key}`);body.id=`core-pane-${id}-${key}`;
      if(key==='essence'){
        for(const field of ['thesis','who','position','answer','grounds','consequences','deed','function'])section(body,field,essence[field]);
        for(const counter of record.counter_voices??[])fold(body,`${t('counter')} · ${name(counter.speaker)}`,box=>{voice(box,counter.voice??{speaker:counter.speaker});form(box,{...counter.line,cite:[counter.cite],status:counter.status});section(box,'reply',counter.reply);});
      }
      if(key==='development'){
        for(const field of ['emergence','movement','transformation'])section(body,field,record.development?.[field]);
        for(const chain of core.chains.filter(chain=>chain.from===id&&chain.admission==='map')){
          const walk=button(`${tr(chain.title)} →`,()=>onRoute(`core:${chain.id}`),'connection-target');body.append(walk);
          fold(body,t('review'),box=>{review(box,chain);for(const step of chain.steps){form(box,{...step.verb,cite:[step.cite],status:step.status});review(box,step);}});
        }
      }
      if(key==='dimensions')for(const item of record.dimensions??[]){const box=el('section','ps-core-section');box.append(el('h3','minor-title',tr(item.name)));form(box,item);body.append(box);}
      if(key==='links')for(const item of core.links.filter(link=>link.from===id||link.to===id).sort((a,b)=>(a.order??0)-(b.order??0))){const other=item.from===id?item.to:item.from;fold(body,`${item.from===id?'→':'←'} ${tr(item.verb)} · ${name(other)}${item.admission==='rejected'?` · ${t('rejected')}`:''}`,box=>link(box,item));}
      if(key==='witnesses')witnesses(body,record);
      if(!body.childElementCount)body.append(el('p','muted',t('empty')));
    };
    for(const key of keys){const tab=button(t(key),()=>draw(key));tab.dataset.tab=key;tab.id=`core-tab-${id}-${key}`;tab.setAttribute('role','tab');tab.setAttribute('aria-controls',`core-pane-${id}-${key}`);tabs.append(tab);}
    tabs.addEventListener('keydown',event=>{const index=[...tabs.children].indexOf(document.activeElement);if(index<0)return;const next=event.key==='ArrowRight'?(index+1)%keys.length:event.key==='ArrowLeft'?(index+keys.length-1)%keys.length:event.key==='Home'?0:event.key==='End'?keys.length-1:null;if(next===null)return;event.preventDefault();draw(keys[next]);tabs.children[next].focus();});
    root.append(tabs,body);draw(record.chainId?'witnesses':'essence');
  }
  return {node,link};
}
