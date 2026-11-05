import {keepQuotesOnce} from './quote-once.mjs';
import {localeText,localeValue,resolveLocale,shownLanguages} from './locale.mjs';
import {chapterTitle} from './fragments.mjs';
import witnessManifest from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import './kernel-cards.css';

// A place of the book is named by its chapter, as the reader's edition titles it.
const CHAPTERS=new Map((witnessManifest.chapters??[]).map(item=>[item.key,item]));
function chapterName(key,locale){
  const chapter=CHAPTERS.get(key),language=resolveLocale(locale);if(!chapter)return key;
  if(language==='ru'||language==='de'||language==='en')return chapterTitle(chapter,language)||key;
  const raw=chapter.titles?.en??'',translated=localeText(raw,language);
  return translated!==raw?translated.replace(/^[IVXLC]+\.\s*/,'').replace(/\.$/,'').trim():chapterTitle(chapter,'en')||key;
}

const words={
  route:['Исследовательский маршрут','Research route','Ruta de investigación','Forschungsweg','研究路线'],states:['Состояния','States','Estados','Zustände','状态'],transitions:['Переходы','Transitions','Transiciones','Übergänge','过渡'],retiredTransitions:['Прежние переходы','Earlier transitions','Transiciones anteriores','Frühere Übergänge','先前过渡'],
  'source-visible-stand-proposal':['Прочтение Древа','The Tree’s reading','Lectura del Árbol','Lesart des Baumes','智慧之树的解读'],
  essence:['Суть','Essence','Esencia','Wesen','本质'],development:['Развитие','Development','Desarrollo','Entwicklung','发展'],dimensions:['Измерения','Dimensions','Dimensiones','Dimensionen','维度'],links:['Связи','Links','Vínculos','Verbindungen','联系'],witnesses:['Свидетельства','Witnesses','Testimonios','Zeugnisse','文本见证'],
  thesis:['Тезис','Thesis','Tesis','These','论点'],question:['Вопрос','Question','Pregunta','Frage','问题'],answer:['Ответ','Answer','Respuesta','Antwort','回答'],grounds:['Основания','Grounds','Fundamentos','Grundlagen','依据'],consequences:['Следствия','Consequences','Consecuencias','Folgen','后果'],function:['Роль в книге','Role in the book','Papel en el libro','Rolle im Buch','在书中的作用'],
  emergence:['Возникновение','Emergence','Aparición','Entstehung','出现'],movement:['Движение','Movement','Movimiento','Bewegung','演变'],transformation:['Преображение','Transformation','Transformación','Verwandlung','转变'],
  counter:['Другой голос','Another voice','Otra voz','Eine andere Stimme','另一种声音'],reply:['Ответ этому голосу','Reply to this voice','Respuesta a esta voz','Antwort auf diese Stimme','回应'],path:['Путь связи','Path of the link','Camino del vínculo','Weg der Verbindung','联系路径'],review:['Проверка и происхождение','Review and provenance','Revisión y procedencia','Prüfung und Herkunft','审核与来源'],original:['Первоначальное предложение','Original proposal','Propuesta original','Ursprünglicher Vorschlag','原始提议'],
  reconstruction:['Реконструкция','Reconstruction','Reconstrucción','Rekonstruktion','重构'],text:['По тексту','Textual grounding','Fundamento textual','Textgrundlage','文本依据'],rejected:['Отвергнуто','Rejected','Rechazado','Verworfen','已否决'],held:['Ожидает проверки','Awaiting review','Pendiente de revisión','Prüfung ausstehend','待审核'],confirmed:['Подтверждено','Confirmed','Confirmado','Bestätigt','已确认'],revise:['Исправлено по проверке','Revised after review','Corregido tras revisión','Nach Prüfung überarbeitet','审核后修订'],keep:['Формула сохранена','Formula retained','Fórmula conservada','Formel beibehalten','保留原句'],better:['Формула уточнена','Formula refined','Fórmula precisada','Formel präzisiert','原句已调整'],
  who:['Кто говорит и действует','Who speaks and acts','Quién habla y actúa','Wer spricht und handelt','谁在言说与行动'],position:['Позиция','Position','Posición','Position','立场'],deed:['Поступок','Deed','Acción','Tat','行动'],
  names:['Слова и имена','Words and names','Palabras y nombres','Wörter und Namen','词语与名称'],uses:['Употребления','Uses','Usos','Verwendungen','用法'],beyond:['Другие произведения: ориентиры для дальнейшего исследования','Other works: leads for further research','Otras obras: pistas para investigar','Andere Werke: Hinweise für weitere Forschung','其他作品：后续研究线索'],hypothesis:['Гипотеза','Hypothesis','Hipótesis','Hypothese','假设'],occurrences:['Места в книге','Places in the book','Pasajes del libro','Stellen im Buch','书中段落'],more:['Показать ещё','Show more','Mostrar más','Mehr anzeigen','显示更多'],empty:['Этот раздел ещё не разработан.','This section is not developed yet.','Esta sección aún no está desarrollada.','Dieser Abschnitt ist noch nicht ausgearbeitet.','此部分尚未展开。'],unavailable:['Не удалось загрузить. Повторить','Could not load. Retry','No se pudo cargar. Reintentar','Laden fehlgeschlagen. Erneut versuchen','无法加载。重试'],
  sharedCitation:['Это место книги приведено выше','This passage is quoted above','Este pasaje se cita más arriba','Diese Stelle ist oben zitiert','这段文字已在上文引用'],quotedAbove:['↑ {0} — приведено выше','↑ {0} — quoted above','↑ {0} — citado más arriba','↑ {0} — oben zitiert','↑ {0} — 已在上文引用'],scope:['Область','Scope','Ámbito','Geltungsbereich','范围'],conditions:['Условия','Conditions','Condiciones','Bedingungen','条件'],premises:['Посылки','Premises','Premisas','Prämissen','前提'],steps:['Шаги аргумента','Argument steps','Pasos del argumento','Argumentationsschritte','论证步骤'],
  read:['Читать','Read','Leer','Lesen','阅读'],compare:['Сопоставить','Compare','Comparar','Vergleichen','比较'],continue:['Продолжить исследование','Continue research','Continuar investigación','Forschung fortsetzen','继续研究'],history:['История','History','Historia','Geschichte','历史'],myWork:['Моя работа','My work','Mi trabajo','Meine Arbeit','我的研究'],workHelp:['Заметка, тезис или возражение к этому предмету','A note, thesis or objection about this subject','Una nota, tesis u objeción sobre este tema','Notiz, These oder Einwand zu diesem Gegenstand','有关此主题的笔记、论点或异议'],relation:['Отношение','Relation','Relación','Beziehung','关系'],figure:['Фигура','Figure','Figura','Figur','形象'],image:['Образ','Image','Imagen','Bild','意象'],interpretation:['Трактовка','Interpretation','Interpretación','Deutung','诠释'],argument:['Аргумент','Argument','Argumento','Argument','论证'],claim:['Утверждение','Claim','Afirmación','Behauptung','主张'],proposal:['Предложение','Proposal','Propuesta','Vorschlag','提议'],openRelation:['Открыть отношение','Open relation','Abrir relación','Beziehung öffnen','打开关系'],researchState:['Состояние исследования','Research status','Estado de la investigación','Forschungsstand','研究状态'],
  narrator:['Рассказчик','Narrator','Narrador','Erzähler','叙述者'],speech:['Речь','Speech','Discurso','Rede','言说'],song:['Песня','Song','Canción','Lied','歌'],quoted:['Цитата в речи','Quoted speech','Cita en el discurso','Zitat in der Rede','话语中的引述'],narration:['Повествование','Narration','Narración','Erzählung','叙述'],dream:['Сон','Dream','Sueño','Traum','梦'],thought:['Мысль','Thought','Pensamiento','Gedanke','思想'],reported:['Переданная речь','Reported speech','Discurso referido','Wiedergegebene Rede','转述'],primary:['Прямой голос','Direct voice','Voz directa','Direkte Stimme','直接声音'],embedded:['Вложенный голос','Embedded voice','Voz insertada','Eingebettete Stimme','嵌入的声音'],imagined:['Воображаемая речь','Imagined speech','Discurso imaginado','Vorgestellte Rede','想象中的言说'],
  family:['Семейство связи','Relation family','Familia de relación','Beziehungsfamilie','关系类别'],familyStatus:['Прочтение Древа','The Tree’s reading','Lectura del Árbol','Lesart des Baumes','智慧之树的解读'],familyType:['Тип в реестре','Registry type','Tipo en el registro','Registertyp','登记类型'],unmapped:['Точный тип не установлен','Exact type not established','Tipo exacto no establecido','Genauer Typ nicht festgelegt','确切类型尚未确定'],previousFamily:['Предыдущее предложение семейства','Previous family proposal','Propuesta anterior de familia','Früherer Familienvorschlag','先前类别提议'],previousSense:['Прежняя формулировка','Previous wording','Formulación anterior','Frühere Formulierung','先前表述'],figureScope:['Роль в произведении','Role in the work','Papel en la obra','Rolle im Werk','作品中的角色'],voiceScope:['Голос в указанном отрывке','Voice in the indicated passage','Voz en el pasaje indicado','Stimme in der angegebenen Passage','所示段落中的声音'],unknownIdentity:['Безымянный голос','An unnamed voice','Una voz sin nombre','Eine namenlose Stimme','无名的声音'],hereSpeaks:['Здесь говорит','Speaks here','Habla aquí','Spricht hier','在此说话'],hereActs:['Здесь действует','Acts here','Actúa aquí','Handelt hier','在此行动'],hereSpokenOf:['Здесь — предмет речи','Spoken of here','Aquí es objeto del discurso','Hier Gegenstand der Rede','在此被谈及'],hereAddressed:['Здесь — адресат речи','Addressed here','Aquí es el destinatario','Hier angesprochen','在此被称呼'],acts:['Действует','Acts','Actúa','Handelt','行动'],addressed:['Адресат','Addressee','Destinatario','Adressat','受话者'],speaks:['Говорит','Speaks','Habla','Spricht','言说'], 'spoken-of':['О ком говорят','Spoken of','Mencionado','Gegenstand der Rede','被提及'],
  semanticUses:['Смысловые употребления','Contextual uses','Usos contextuales','Kontextuelle Verwendungen','语境用法'],semanticScope:['Слова подобраны по совпадению словоформ или как ориентир для исследования. Группы относятся к указанным местам; не все употребления слова автоматически относятся к этому предмету.','Words are selected by matching forms or as research leads. Groups concern the cited passages; not every use of the word automatically concerns this subject.','Las palabras se seleccionan por formas coincidentes o como pistas; no todo uso concierne automáticamente a este tema.','Wörter werden über Wortformen oder als Forschungshinweise ausgewählt; nicht jede Verwendung betrifft automatisch diesen Gegenstand.','词语依据词形或研究线索选择；并非每次用法都自动属于此主题。'],unknownUses:['Места без рассмотренного значения','Places with unassessed meaning','Pasajes sin significado examinado','Stellen ohne geprüfte Bedeutung','含义尚未审查的段落'],via:['Через','Via','Por medio de','Über','经由'],
  formulaReview:['Как выбрана формула','How the formula was chosen','Cómo se eligió la fórmula','Wie die Formel gewählt wurde','原句如何选定'],madeBy:['Предложил','Proposed by','Propuesto por','Vorgeschlagen von','提出者'],checkedBy:['Проверил','Checked by','Revisado por','Geprüft von','审核者'],samePlace:['То же место книги','The same place in the book','El mismo lugar del libro','Dieselbe Stelle des Buches','书中同一处'],
  proposalReading:['Прочтение','Reading','Lectura','Lesart','解读'],readingLimit:['Границы прочтения','Limits of the reading','Límites de la lectura','Grenzen der Deutung','解读范围'],relatedSubject:['Связанный предмет','Related subject','Tema relacionado','Verwandter Gegenstand','相关主题'],method:['Исследовательский приём','Research method','Método de investigación','Untersuchungsmethode','研究方法'],methodEpisode:['От разбора к приёму','From reading to method','De la lectura al método','Von der Lektüre zur Methode','从阅读到方法'],previousTitle:['Прежнее название','Previous title','Título anterior','Früherer Titel','先前标题'],
  outgoing:['Ведёт к','Leads to','Conduce a','Führt zu','通向'],incoming:['Приходит от','Comes from','Viene de','Kommt von','来自'],
};
const langs=['ru','en','es','de','zh-Hans'];
const el=(tag,cls='',text)=>{const node=document.createElement(tag);node.className=cls;if(text!==undefined)node.textContent=text;return node;};
const button=(text,run,cls='text-button')=>{const node=el('button',cls,text);node.type='button';node.addEventListener('click',run);return node;};
// Generated filler stands where no reading was written yet; a card shows the reading or nothing.
const FILLERS=[/^Как встречаются «.+» и «.+» в этом месте\?$/,/^Контекстное значение ещё не разобрано\.$/,/возникает в словесном окружении и маршрутах понятия/];
const filler=value=>{const ru=typeof value==='string'?value:value?.ru;return typeof ru==='string'&&FILLERS.some(pattern=>pattern.test(ru));};
// A place is the same place whatever words of it a citation quotes.
const placeKey=cite=>cite&&typeof cite==='object'?`${cite.w??''}|${cite.c??''}|${(cite.u??[]).join(',')}`:JSON.stringify(cite);
// Two lines that say the same thing in nearly the same words: most of their longer words are shared.
const alike=(a,b)=>{const words=text=>new Set(String(text??'').toLocaleLowerCase().match(/[\p{L}]{4,}/gu)??[]);const x=words(a),y=words(b);if(!x.size||!y.size)return false;let common=0;for(const word of x)if(y.has(word))common++;return common/Math.min(x.size,y.size)>=.7;};
const occurrenceFiles=import.meta.glob('../../../../bonsai/zarathustra/core/occurrences/*.json',{import:'default'});

// What a figure does at one place of the book.
const HERE={speaks:'hereSpeaks',acts:'hereActs','spoken-of':'hereSpokenOf',addressed:'hereAddressed'};

/** Every quoted word is rendered by the existing aligned-witness reader. */
export function createCoreView({core,locale,fragments,onNode,onRoute,onRelation,onAction}){
  let serial=0;
  const tr=value=>localeValue(value,locale());
  const t=key=>words[key]?.[Math.max(0,langs.indexOf(resolveLocale(locale())))]??key;
  const name=id=>tr(core.records[id]?.title)||(id==='narrator'?t('narrator'):id);
  // A voice is named by its figure; a voice the core does not name speaks without a label.
  const speakerName=id=>tr(core.records[id]?.title)||(id==='narrator'?t('narrator'):'');
  const status=(parent,value)=>{if(value)parent.append(el('small','ps-core-status',t(value)));};
  function form(parent,value,seen=null){
    if(!value||filler(value))return;
    if(Array.isArray(value)){for(const item of value)form(parent,item,seen);return;}
    let normalized=value?.cite&&!Array.isArray(value.cite)?{...value,cite:[value.cite]}:value;
    let shared=false;
    if(seen&&Array.isArray(normalized?.cite)){
      const cites=normalized.cite.filter(cite=>{const key=placeKey(cite);if(seen.has(key)){shared=true;return false;}seen.add(key);return true;});
      normalized={...normalized,cite:cites};
    }
    fragments.render(parent,normalized);
    if(shared)parent.append(el('small','ps-core-status',t('sharedCitation')));
    status(parent,value.status);
  }
  function section(parent,key,value,seen=null){
    if(!value||(Array.isArray(value)&&!value.length)||filler(value))return;
    const box=el('section','ps-core-section');box.append(el('h3','minor-title',t(key)));form(box,value,seen);parent.append(box);
  }
  function fold(parent,label,render){
    const box=el('details','ps-core-fold'),body=el('div');box.append(el('summary','',label),body);let loaded=false;
    box.addEventListener('toggle',()=>{if(box.open&&!loaded){loaded=true;render(body);}});parent.append(box);return box;
  }
  function voice(parent,value){
    if(!value)return;
    const parts=[speakerName(value.speaker),value.mode&&t(value.mode),value.layer&&t(value.layer),value.via&&speakerName(value.via)].filter(Boolean);
    parent.append(el('p','ps-core-voice',parts.join(' · ')));
  }
  function voiceDescriptor(parent,descriptor){
    if(!descriptor)return;
    const box=el('section','ps-core-section');box.append(el('h3','minor-title',t('voiceScope')));
    // The voice is named by its speaker; a speaker the book leaves without a name is an unnamed voice.
    const knownName=id=>id==='narrator'||core.records[id]?name(id):null;
    const speaker=knownName(descriptor.speaker_designation),via=knownName(descriptor.via),addressee=knownName(descriptor.addressee);
    box.append(el('p','ps-core-voice',[speaker??t('unknownIdentity'),descriptor.mode&&t(descriptor.mode),descriptor.layer&&t(descriptor.layer),via&&`${t('via')}: ${via}`,addressee&&`${t('addressed')}: ${addressee}`].filter(Boolean).join(' · ')));
    if(descriptor.cite)box.append(fragments.fragment(descriptor.cite));
    parent.append(box);
  }
  function appearanceVoices(parent,item,record){
    const anchors=new Set(item.cite?.u??[]);
    const segments=(core.voices?.[item.cite?.c]??[]).filter(segment=>segment.voice_descriptor&&segment.u?.some(unit=>anchors.has(unit))&&(segment.speaker===record.id||segment.addressee===record.id||segment.actors?.includes(record.id)));
    for(const segment of segments)voiceDescriptor(parent,segment.voice_descriptor);
  }
  function relationFamily(parent,record){
    const family=record.relation_family;if(!family)return;
    const box=el('section','ps-core-section');
    box.append(el('p','',tr(family.family_label)||family.family||''));
    if(family.family_reason)form(box,family.family_reason);
    if(family.family_status)box.append(el('small','ps-core-status',family.family_status==='stand-proposal'?t('familyStatus'):family.family_status));
    if(family.relation_type)box.append(el('p','muted',`${t('familyType')}: ${family.relation_type}${family.relation_type==='tos.relation.unmapped'?` · ${t('unmapped')}`:''}`));
    parent.append(box);
  }
  function semanticUses(parent,record){
    const forms=(record.lexicon??record.names)?.de??[],known=new Set(forms.map(item=>item.form?.toLocaleLowerCase('de')).filter(Boolean));
    const leads={'brothers':['bruder'],'gold':['gold'],'disciples':['bruder'],'bestowing-virtue':['schenken','gold'],'going-under':['untergehen']};
    const terms=(core.semanticUses?.terms??[]).filter(term=>leads[record.id]?.includes(term.id)||term.forms?.some(form=>known.has(form.toLocaleLowerCase('de'))));
    if(!terms.length)return;
    fold(parent,t('semanticUses'),body=>{
      body.append(el('p','muted',t('semanticScope')));
      for(const term of terms){
        const part=el('section','ps-core-section');part.append(el('h3','minor-title',term.lemma));form(part,term.scope);
        for(const group of term.groups??[])fold(part,tr(group.label),box=>{form(box,group.meaning);section(box,'scope',group.scope);for(const cite of group.grounds??[])box.append(fragments.fragment(cite));if(group.review_status)box.append(el('small','ps-core-status',group.review_status==='stand-proposal'?t('familyStatus'):group.review_status));});
        const unknown=(term.occurrences??[]).filter(item=>!item.semantic_use?.group_id);
        if(unknown.length)fold(part,`${t('unknownUses')} · ${unknown.length}`,box=>{for(const item of unknown)if(item.cite)box.append(fragments.fragment(item.cite));});
        body.append(part);
      }
    });
  }
  const samePlace=(a,b)=>Boolean(a&&b&&a.c===b.c&&JSON.stringify(a.u)===JSON.stringify(b.u));
  function original(parent,value,current){
    if(!value)return;
    const cites=value.cite?(Array.isArray(value.cite)?value.cite:[value.cite]):[];
    // The same paragraph is not drawn twice: an earlier quote from the current place is named as that place.
    if(cites.length&&cites.every(cite=>samePlace(cite,current)))parent.append(el('p','muted',t('samePlace')));
    else if(cites.length)form(parent,{cite:cites});
    for(const key of ['verb','label','sense','why'])if(value[key])form(parent,value[key]);
    for(const cite of value.grounds??[])parent.append(fragments.fragment(cite));
    voice(parent,value.voice);
  }
  const proposalOf=record=>record.proposed&&!Array.isArray(record.proposed)?record.proposed:null;
  function review(parent,record,label=t('review')){
    const proposed=proposalOf(record);
    if(!record.assessment&&!proposed)return;
    fold(parent,label,body=>{
      const a=record.assessment??{};status(body,a.verdict);
      if(a.reason)form(body,a.reason);
      // Who made and checked a record is kept in the data for its history; a reader sees what was found.
      if(a.counter_reading){section(body,'counter',a.counter_reading);for(const cite of a.counter_reading.grounds??[])body.append(fragments.fragment(cite));}
      if(proposed)fold(body,t('original'),box=>original(box,proposed,record.cite));
    });
  }
  function formula(parent,item,lead=null){
    if(!item)return;
    const box=el('section','ps-core-formula');voice(box,item.voice??{speaker:item.speaker});
    if(item.cite)box.append(fragments.fragment(item.cite));
    // After review replaced a formula, its why is the reviewer's note on that choice and lives in the history.
    if(item.why&&item.assessment?.reason!==item.why.ru&&!alike(tr(item.why),lead))form(box,item.why);
    parent.append(box);
  }
  function link(parent,record,{bench=false}={}){
    // The link reads as a sentence of the book: subject, the book's verb, object. Its registry family and type are provenance.
    const box=el('section','ps-core-link'),ends=el('div','edge-endpoints ps-link-sentence');
    ends.append(button(name(record.from),()=>onNode?.(core.materialIds[record.from]),'ps-link-end'),el('span','ps-link-verb',tr(record.verb)),button(name(record.to),()=>onNode?.(core.materialIds[record.to]),'ps-link-end'));
    const kindLine=el('p','ps-core-kind',t('relation'));status(kindLine,record.admission==='rejected'?'rejected':record.status);box.append(kindLine,ends);
    voice(box,record.voice);
    const seen=new Set(),sense=tr(record.sense);
    form(box,{...record.sense,cite:[...new Map([...(record.cite?[record.cite]:[]),...(record.grounds??[])].map(cite=>[JSON.stringify(cite),cite])).values()]},seen);
    actions(box,record,'relation',null,{bench});
    section(box,'question',record.question);
    if(record.answer&&tr(record.answer)!==sense)section(box,'answer',record.answer,seen);
    for(const field of ['scope','conditions'])if(record[field])section(box,field,record[field]);
    if(record.path?.length){
      const path=el('section','ps-core-path');path.append(el('h3','minor-title',t('path')));
      const endpoint=id=>core.materialIds[id]?button(name(id),()=>onNode?.(core.materialIds[id])):el('span','',name(id));
      for(const step of record.path){
        const row=el('div','ps-core-path-step');
        if(step.id)row.append(endpoint(step.id));
        else if(step.from&&step.to)row.append(endpoint(step.from),el('span','',tr(step.verb)),endpoint(step.to));
        for(const cite of step.cite?(Array.isArray(step.cite)?step.cite:[step.cite]):[])row.append(fragments.fragment(cite));
        status(row,step.status);path.append(row);
      }
      box.append(path);
    }
    methodTransfer(box,record);
    if(record.relation_family)fold(box,t('family'),body=>relationFamily(body,record));
    history(box,record);personal(box,record,'relation');parent.append(box);
  }
  function methodTransfer(parent,record){
    const method=record.method_transfer;if(!method)return;
    fold(parent,tr(method.title)||t('methodEpisode'),body=>{
      form(body,method.episode);section(body,'method',method.method);
      for(const application of method.applications??[])fold(body,tr(application.title)||application.id,box=>{
        form(box,application.result);section(box,'readingLimit',application.limit);
        if(typeof application.source_url==='string')try{const url=new URL(application.source_url);if(url.protocol==='https:'&&!url.username&&!url.password){const source=el('a','text-button',tr(application.source_label)||url.hostname);source.href=url.href;source.target='_blank';source.rel='noopener noreferrer';box.append(source);}}catch{}
      });
    });
  }
  function witnesses(parent,record){
    for(const item of record.appearances??[])fold(parent,`${chapterName(item.cite.c,locale())} · ${tr(item.line)}`,box=>{if(item.role)box.append(el('p','ps-core-voice',HERE[item.role]?t(HERE[item.role]):`${t('figureScope')}: ${t(item.role)}`));form(box,{...item.line,cite:[item.cite],status:item.status});appearanceVoices(box,item,record);});
    for(const item of record.voice??[])fold(parent,[chapterName(item.cite.c,locale()),speakerName(item.speaker),tr(item.line)].filter(Boolean).join(' · '),box=>{voice(box,item);form(box,{...item.line,cite:[item.cite],status:item.status});});
    for(const item of record.conception?.formulas??[])fold(parent,`${chapterName(item.cite.c,locale())} · ${tr(item.why)}`,box=>{formula(box,item);review(box,item,t('formulaReview'));});
    for(const item of record.questions??[])fold(parent,tr(item.question),box=>{form(box,item.answer);status(box,item.status);});
    const names=record.lexicon??record.names;
    if(names)fold(parent,t('names'),box=>{const shown=shownLanguages(locale(),'de',Object.keys(names));for(const [language,items]of Object.entries(names)){if(!shown.includes(language)||!Array.isArray(items))continue;box.append(el('h3','minor-title',language.toUpperCase()));box.append(el('p','',items.filter(item=>typeof item?.form==='string').map(item=>`${item.form} (${item.count})`).join(' · ')));}});
    semanticUses(parent,record);
    if(record.uses?.length)fold(parent,t('uses'),box=>{for(const item of record.uses)box.append(el('p','',`${tr(item.name)} · ${item.count}`));});
    if(record.beyond?.length)fold(parent,t('beyond'),box=>{for(const item of record.beyond){const entry=el('section','ps-core-section');entry.append(el('h3','minor-title',[item.work,item.place].filter(Boolean).join(' · ')));form(entry,item.note);status(entry,item.status);box.append(entry);}});
    const loader=occurrenceFiles[`../../../../bonsai/zarathustra/core/occurrences/${record.id}.json`];
    if(loader)fold(parent,t('occurrences'),box=>{
      const load=async()=>{try{
        const data=await loader(),items=Array.isArray(data)?data:data.occurrences??[];box.replaceChildren();let offset=0;
        const more=button(t('more'),()=>page());
        const page=()=>{more.remove();for(const item of items.slice(offset,offset+20))fold(box,[chapterName(item.cite.c,locale()),speakerName(item.speaker),item.form].filter(Boolean).join(' · '),body=>{voice(body,item);form(body,{...item.line,cite:[item.cite]});});offset+=20;if(offset<items.length)box.append(more);};page();
      }catch{box.replaceChildren(button(t('unavailable'),load));}};void load();
    });
    if(record.cite)form(parent,{cite:[record.cite]});
  }
  function kind(record){
    const type=String(record.type??'').toLowerCase();
    if(type.includes('argument'))return 'argument';
    if(type.includes('claim')||type.includes('assertion'))return 'claim';
    if(record.existence)return 'figure';
    if(type.includes('thought-image'))return 'image';
    if(type.includes('conception')||record.conception)return 'interpretation';
    return 'proposal';
  }
  function firstCite(record,recordKind){
    const essence=record.conception??record.essence??{};
    const cite=recordKind==='relation'?record.cite??record.grounds?.[0]:essence.formula?.cite??essence.formulas?.[0]?.cite??record.reading?.grounds?.[0]??record.cite??record.appearances?.[0]?.cite??record.voice?.[0]?.cite;
    return Array.isArray(cite)?cite[0]:cite;
  }
  function relationId(record){
    if(record.id)return record.id;
    for(const chain of core.chains){const index=chain.steps?.indexOf(record)??-1;if(index>=0)return `core-chain:${chain.id}:${index}`;}
    return null;
  }
  function actions(parent,record,recordKind,selectLinks,{bench=false}={}){
    if(!onAction&&!selectLinks)return;
    const bar=el('nav','ps-kernel-actions');bar.setAttribute('aria-label',t('researchState'));
    for(const action of ['read','compare','links','continue']){
      if(!onAction&&action!=='links')continue;
      // On the workbench the reader's own work has a place of its own, so the tools keep reading, comparing and the links.
      if(bench&&action==='continue')continue;
      bar.append(button(t(action),()=>{
        if(action==='links'&&selectLinks&&!onAction){selectLinks();return;}
        onAction?.({action,recordId:recordKind==='relation'?relationId(record):record.id,kind:recordKind,record,cite:firstCite(record,recordKind)});
      },'ps-kernel-action'));
    }
    parent.append(bar);
  }
  function personal(parent,record,recordKind){
    if(!onAction)return;
    const box=el('section','ps-kernel-personal');box.append(el('h3','minor-title',t('myWork')),el('p','muted',t('workHelp')));
    box.append(button(t('continue'),()=>onAction({action:'continue',recordId:recordKind==='relation'?relationId(record):record.id,kind:recordKind,record,cite:firstCite(record,recordKind)}),'ps-kernel-action'));parent.append(box);
  }
  function history(parent,record){
    const formulaRecord=(record.conception??record.essence)?.formula;
    const revisions=[...(record.sense_revision_history??[]),...(record.step_revision_history??[])],familyProposed=record.relation_family?.family_proposed;
    const oldReadings=record.reading?.reading_revision_history??record.reading_revision_history??[],oldTitles=record.title_revision_history??[];
    if(!record.assessment&&!proposalOf(record)&&!formulaRecord?.assessment&&!formulaRecord?.proposed&&!revisions.length&&!familyProposed&&!oldReadings.length&&!oldTitles.length)return;
    fold(parent,t('history'),body=>{
      review(body,record);
      if(formulaRecord)review(body,formulaRecord,t('formulaReview'));
      for(const revision of revisions){const box=el('section','ps-core-section');box.append(el('h3','minor-title',t('previousSense')));if(revision.verb)form(box,revision.verb);form(box,{...revision.sense,...(revision.cite?{cite:[revision.cite]}:{})});if(revision.answer)section(box,'answer',revision.answer);if(revision.assessment?.reason)form(box,revision.assessment.reason);status(box,revision.assessment?.verdict);body.append(box);}
      for(const title of oldTitles){const box=el('section','ps-core-section');box.append(el('h3','minor-title',t('previousTitle')));form(box,title);body.append(box);}
      for(const reading of oldReadings){const box=el('section','ps-core-section');box.append(el('h3','minor-title',t('previousSense')));form(box,{...reading.reading,cite:reading.grounds??(reading.cite?[reading.cite]:[])});section(box,'readingLimit',reading.limit);body.append(box);}
      if(familyProposed){const box=el('section','ps-core-section'),label=core.links.find(link=>link.relation_family?.family===familyProposed.family)?.relation_family?.family_label;box.append(el('h3','minor-title',t('previousFamily')),el('p','',tr(label)||familyProposed.family||''));if(familyProposed.family_reason)form(box,familyProposed.family_reason);body.append(box);}
    });
  }
  function sections(body,key,record,id,lead,bench=false){
    const essence=record.conception??record.essence??{};
    if(key==='essence'){
      const first=essence.formula??essence.formulas?.[0],seen=new Set(first?.cite?[placeKey(first.cite)]:[]);
      // The lead field's words already open the card; here it keeps only the book's grounds for them.
      for(const field of ['thesis','who','position','answer','grounds','consequences','deed','function'])section(body,field,field===lead?(essence[field]?.cite?{cite:essence[field].cite}:null):essence[field],seen);
      if(kind(record)==='argument')for(const field of ['premises','steps'])section(body,field,record[field]??essence[field],seen);
      if(kind(record)==='claim')for(const field of ['scope','conditions'])section(body,field,record[field]??essence[field],seen);
      for(const counter of record.counter_voices??[])fold(body,[t('counter'),speakerName(counter.speaker)].filter(Boolean).join(' · '),box=>{voice(box,counter.voice??{speaker:counter.speaker});form(box,{...counter.line,cite:[counter.cite],status:counter.status});section(box,'reply',counter.reply);});
      if(record.reading?.reading){
        for(const cite of record.reading.grounds??[])body.append(fragments.fragment(cite));
        section(body,'readingLimit',record.reading.limit);
        if(record.reading.related_existing&&core.records[record.reading.related_existing])body.append(button(`${t('relatedSubject')}: ${name(record.reading.related_existing)}`,()=>onNode?.(core.materialIds[record.reading.related_existing]),'connection-target'));
        const review=record.reading.independent_review;if(review)fold(body,t('review'),box=>{status(box,review.verdict);});
      }
    }
    if(key==='development'){
      for(const field of ['emergence','movement','transformation'])section(body,field,record.development?.[field]);
      for(const chain of core.chains.filter(chain=>chain.from===id&&chain.admission==='map')){
        body.append(button(`${tr(chain.title)} →`,()=>onRoute?.(`core:${chain.id}`),'connection-target'));
        fold(body,t('review'),box=>{review(box,chain);for(const step of chain.steps){form(box,{...step.verb,cite:[step.cite],status:step.status});review(box,step);}});
      }
    }
    if(key==='dimensions')for(const item of record.dimensions??[]){const box=el('section','ps-core-section');box.append(el('h3','minor-title',tr(item.name)));form(box,item);body.append(box);}
    if(key==='links'&&bench){
      // On the workbench the links read as two lists, what the subject leads to and what leads to it: a name lays that
      // subject on the bench, the verb opens the link itself between the two.
      const items=core.links.filter(link=>(link.from===id||link.to===id)&&link.admission!=='rejected').sort((a,b)=>(a.order??0)-(b.order??0));
      for(const [arrow,side,list] of [['→','outgoing',items.filter(item=>item.from===id)],['←','incoming',items.filter(item=>item.from!==id)]]){
        if(!list.length)continue;
        const group=el('div','ps-relation-group');group.append(el('h3','ps-relation-kind',`${arrow} ${t(side)} · ${list.length}`));
        for(const item of list){
          const other=item.from===id?item.to:item.from,row=el('div','ps-link-row'),materialId=core.materialIds[other];
          row.append(onRelation?button(tr(item.verb),()=>onRelation(item.id),'ps-link-row-verb'):el('span','ps-link-row-verb',tr(item.verb)));
          row.append(materialId&&onNode?button(name(other),()=>onNode(materialId),'ps-link-row-end'):el('span','ps-link-row-end',name(other)));
          group.append(row);
        }
        body.append(group);
      }
    }else if(key==='links')for(const item of core.links.filter(link=>(link.from===id||link.to===id)&&link.admission!=='rejected').sort((a,b)=>(a.order??0)-(b.order??0))){
      const other=item.from===id?item.to:item.from;
      fold(body,`${item.from===id?'→':'←'} ${tr(item.verb)} · ${name(other)}${item.admission==='rejected'?` · ${t('rejected')}`:''}`,box=>{
        if(onRelation)box.append(button(t('openRelation'),()=>onRelation(item.id),'ps-kernel-action'));
        link(box,item);
      });
    }
    if(key==='witnesses')witnesses(body,record);
  }
  // A part of the card speaks only when it has something to say: an empty part is left out, its tab and its heading too.
  const filledKeys=(keys,record,id,lead)=>keys.filter(key=>{if(key==='essence')return true;const probe=el('div');sections(probe,key,record,id,lead);return probe.childElementCount>0;});
  // A subject at one end of a link, as the workbench lays it beside the other: its thesis or reading, its formula
  // in the book's words and the question it holds.
  function brief(parent,id){
    const record=core.records[id];if(!record)return false;
    const essence=record.conception??record.essence??{},short=essence.thesis??essence.who??record.reading?.reading??record.reading;
    if(tr(short))parent.append(el('p','body',tr(short)));
    formula(parent,essence.formula??essence.formulas?.[0],tr(short));
    section(parent,'question',record.question??essence.question);
    return true;
  }
  function node(parent,id,{reference=false,bench=false}={}){
    const record=core.records[id];if(!record)return false;
    const essence=record.conception??record.essence??{},root=el('article',reference?'ps-core-card ps-kernel-reference':'ps-core-card');root.dataset.coreId=id;parent.append(root);
    const header=el('header','ps-kernel-header');const kindLine=el('p','ps-core-kind',t(kind(record)));header.append(kindLine);if(reference)header.append(el('h2','ps-kernel-title',name(id)));root.append(header);
    if(record.work_scope?.roles?.length){const scope=el('section','ps-core-section');scope.append(el('h3','minor-title',t('figureScope')),el('p','',record.work_scope.roles.map(role=>t(role)).join(' · ')));if(record.work_scope.status)scope.append(el('small','ps-core-status',record.work_scope.status==='stand-proposal'?t('familyStatus'):record.work_scope.status));root.append(scope);}
    const short=essence.thesis??essence.who??record.reading?.reading??record.reading,lead=essence.thesis?'thesis':essence.who?'who':null;
    if(tr(short)){const summary=el('section','ps-kernel-summary');summary.append(el('h3','minor-title',t(essence.thesis?'thesis':record.reading?'proposalReading':'who')),el('p','body',tr(short)));root.append(summary);}
    section(root,'question',record.question??essence.question);
    if(record.reading?.reading||record.reading?.ru||record.reading?.en){status(kindLine,'familyStatus');status(kindLine,record.reading.independent_review?.verdict??record.independent_review?.verdict);}
    else if(!record.conception&&!record.essence&&!record.chainId)status(kindLine,'held');
    else status(kindLine,essence.thesis?.status??essence.formula?.status??record.status);
    const keys=['essence','development','dimensions','links','witnesses'];let showLinks;
    actions(root,record,'node',reference?null:()=>showLinks?.(),{bench});
    formula(root,essence.formula??essence.formulas?.[0],tr(short));
    if(reference||bench){
      // On the workbench every part lies open at its step of interpretation: the book's places with the source,
      // the links with the comparison, the rest with the reading.
      const steps={witnesses:'source',links:'comparison'};
      for(const key of keys){const content=el('div');sections(content,key,record,id,lead,bench);if(!content.childElementCount)continue;const part=el('section','ps-kernel-reference-section');part.append(el('h3','ps-kernel-section-title',t(key)),content);root.append(part);
        if(bench){part.dataset.zone=steps[key]??'reading';part.dataset.part=key;if(key==='links')showLinks=()=>part.scrollIntoView({block:'start',behavior:'smooth'});}}
      if(reference)keepQuotesOnce(root,{label:source=>t('quotedAbove').replace('{0}',`«${source.split(' · ')[0]}»`)});
    }else{
      const tabs=el('div','ps-core-tabs'),body=el('section','ps-core-pane'),instance=++serial;tabs.setAttribute('role','tablist');tabs.setAttribute('aria-label',name(id));body.setAttribute('role','tabpanel');
      const draw=key=>{
        body.replaceChildren();
        for(const tab of tabs.children){const chosen=tab.dataset.tab===key;tab.setAttribute('aria-selected',String(chosen));tab.tabIndex=chosen?0:-1;}
        body.setAttribute('aria-labelledby',`core-tab-${instance}-${key}`);body.id=`core-pane-${instance}-${key}`;
        sections(body,key,record,id,lead);
      };
      const shown=filledKeys(keys,record,id,lead);
      showLinks=()=>draw(shown.includes('links')?'links':'essence');
      for(const key of shown){const tab=button(t(key),()=>draw(key));tab.dataset.tab=key;tab.id=`core-tab-${instance}-${key}`;tab.setAttribute('role','tab');tab.setAttribute('aria-controls',`core-pane-${instance}-${key}`);tabs.append(tab);}
      tabs.addEventListener('keydown',event=>{const index=[...tabs.children].indexOf(document.activeElement);if(index<0)return;const next=event.key==='ArrowRight'?(index+1)%shown.length:event.key==='ArrowLeft'?(index+shown.length-1)%shown.length:event.key==='Home'?0:event.key==='End'?shown.length-1:null;if(next===null)return;event.preventDefault();event.stopPropagation();draw(shown[next]);tabs.children[next].focus();});
      root.append(tabs,body);draw(record.chainId&&shown.includes('witnesses')?'witnesses':'essence');
    }
    history(root,record);personal(root,record,'node');return true;
  }
  function routeReference(parent,chain){
    const article=el('article','ps-kernel-reference');parent.append(article);
    article.append(el('p','eyebrow',t('route')),el('h2','',tr(chain.title)));status(article,'familyStatus');
    form(article,chain.sense);section(article,'question',chain.question);
    const states=el('section','ps-core-section');states.append(el('h3','minor-title',t('states')));
    for(const state of chain.states??[]){
      const part=el('section','ps-core-section');part.append(button(tr(state.title),()=>onNode?.(core.materialIds[state.id]??state.id),'connection-target'));
      form(part,state.reading);if(state.cite)part.append(fragments.fragment(state.cite));status(part,state.independent_review?.verdict);states.append(part);
    }
    article.append(states);
    const transitions=el('section','ps-core-section');transitions.append(el('h3','minor-title',t('transitions')));
    for(const step of chain.steps??[]){
      const stateName=id=>tr(chain.states?.find(state=>state.id===id)?.title)||name(id);
      fold(transitions,`${stateName(step.from)} → ${tr(step.verb)} → ${stateName(step.to)}`,box=>{
        if(onRelation)box.append(button(t('openRelation'),()=>onRelation(relationId(step)),'ps-kernel-action'));
        link(box,step);
      });
    }
    article.append(transitions);review(article,chain);
    if(chain.retired_steps?.length)fold(article,t('retiredTransitions'),box=>{for(const step of chain.retired_steps)link(box,step);});
    return true;
  }
  function renderReference(parent,{kind:recordKind,id}={}){
    if(recordKind==='node'||recordKind==='chain'){
      const chain=core.chains.find(item=>item.id===id);
      return chain?routeReference(parent,chain):node(parent,id,{reference:true});
    }
    if(recordKind==='relation'){
      let record=core.links.find(item=>item.id===id||`core-link:${item.id}`===id);
      if(!record)for(const chain of core.chains){record=chain.steps?.find(item=>item.id===id||item.aliases?.includes(id));if(record)break;}
      if(!record&&id?.startsWith('core-chain:'))for(const chain of core.chains){const prefix=`core-chain:${chain.id}:`;if(id.startsWith(prefix)){record=chain.steps?.[Number(id.slice(prefix.length))];break;}}
      if(!record)return false;
      const article=el('article','ps-kernel-reference');parent.append(article);link(article,record);return true;
    }
    return false;
  }

  return {node,link,brief,renderReference};
}
