import {ui,uiText,uiLanguage,t} from './ui-i18n.mjs';
import {displayLanguageKey} from './display-language.mjs';

const present=value=>typeof value==='string'&&Boolean(value.trim());

// A title is allowed to stay in the language supplied by its owner. This
// guard only removes transport identifiers, projected paths and serialized
// records that cannot serve as a human title. It never translates or derives
// wording from an ID.
const machineTitle=value=>{
  if(!present(value))return true;
  const text=value.trim();
  if(/\u0000|\u007f/.test(text))return true;
  if(/^\s*(?:\{\s*["']?(?:id|kind|title|display|relation|node)\b|\[\s*\{)/i.test(text)
    ||/^[a-z][a-z0-9+.-]*:\/\//i.test(text))return true;
  if(/^(?:\/|[a-z]:[\\/]|\.{1,2}[\\/])/i.test(text))return true;
  if(/^(?:ToS|source(?:[-_][a-z0-9-]+)?|(?:srv|home|tmp|var|opt|usr|mnt))[\\/]/i.test(text))return true;
  if(/^tos(?:\.(?:record|node|edge|relation|claim|work|file|item)\b|[\\/]|$)/i.test(text))return true;
  if(/^(?:sha(?:256)?|md5):[a-f0-9]{16,}$/i.test(text)||/^[a-f0-9]{40,}$/i.test(text))return true;
  if(/^(?:source-(?:navigation|claims)|graph|node|edge|relation)(?:[.:/\\]|$)/i.test(text))return true;
  if(/^(?:claim|identity|record|item|file|node|edge|relation):(?:tos\b|source[-_:]|sha(?:256)?\b|[a-f0-9]{16,}\b)/i.test(text))return true;
  return false;
};

export const isReadablePresentationTitle=(value,{guard=true}={})=>present(value)&&(!guard||!machineTitle(value));

// Select the first explicit title form that remains readable. `default` and
// `original` are source vocabulary keys, not guessed language tags.
export function readableTitleForm(value,preferred='ru',{guard=false}={}){
  if(!value||typeof value!=='object'||Array.isArray(value))return null;
  const keys=[preferred,'default','original','ru','en',...Object.keys(value).filter(displayLanguageKey)];
  const key=keys.find((candidate,index)=>keys.indexOf(candidate)===index&&isReadablePresentationTitle(value[candidate],{guard}));
  if(!key)return null;
  return {text:value[key],key,lang:displayLanguageKey(key)?key:null,fallback:key!==preferred};
}

// Interface vocabulary only; source titles and quotations remain source text.
export function languageName(tag,locale=uiLanguage()){
  if(!tag)return '';
  try{return new Intl.DisplayNames([locale],{type:'language',fallback:'none'}).of(tag)||t('Другой язык');}
  catch{return t('Другой язык');}
}
export function sourceLinkLabel(ref,locale=uiLanguage()){
  try{
    const url=new URL(ref);if(!['https:','http:'].includes(url.protocol))return t('Источник');
    const host=url.hostname.replace(/^www\./,'');
    if(host==='archive.org')return 'Internet Archive';
    if(host.endsWith('wikisource.org'))return locale==='ru'?'Викитека':'Wikisource';
    return host;
  }catch{return t('Источник');}
}
export function sourceTitle(raw){
  if(raw?.display?.provenance?.source_title_available===false)return '';
  const title=raw?.display?.title;if(title?.original)return title.original;
  if(['text-unit','anchor'].includes(raw?.kind_id))return '';
  return title?.default??'';
}
const sources={philosophy:['Философия','Philosophy','Filosofía'],canon:['Канон','Canon','Canon'],
  'candidate-intake':['Новые материалы','New materials','Materiales nuevos'],
  'source-navigation':['Источники','Sources','Fuentes'],'source-claims':['Сведения об источниках','Source information','Información de las fuentes'],
  zarathustra:['Заратустра','Zarathustra','Zaratustra'],'source-witnesses':['Свидетельства','Witnesses','Testimonios'],
  'semantic-interchange':['Типы и связи','Types and relations','Tipos y relaciones'],
  repository:['Материалы проекта','Project materials','Materiales del proyecto']};
const profiles={overview:['Обзор','Overview','Resumen'],all:['Все связи','All relations','Todas las relaciones'],
  reading:['Чтение','Reading','Lectura'],semantic:['Смысловые связи','Meaning','Sentido'],
  evidence:['Основания','Evidence','Fundamentos'],provenance:['Происхождение','Provenance','Procedencia']};
const local=(map,id,locale,fallback)=>map[id]?.[{ru:0,en:1,es:2}[locale]??0]||fallback;
export const sourceLabel=(id,locale=uiLanguage())=>local(sources,id,locale,t('Другие источники'));
export const profileLabel=(id,locale=uiLanguage())=>local(profiles,id,locale,t('Другие связи'));

export function fileLabel(attributes={},locale=uiLanguage(),filename=''){
  const formats={'application/pdf':'PDF','application/vnd.djvu+xml':'DjVu XML','application/xml':'XML','text/xml':'XML',
    'application/gzip':'GZip','text/plain':locale==='ru'?'Текст':locale==='es'?'Texto':'Text',
    'image/jpeg':'JPEG','image/png':'PNG','application/epub+zip':'EPUB'};
  // Compact packets retain the supplied filename even when byte metadata is
  // absent. Its extension is a display hint, never a content or rights check.
  const suffix=typeof filename==='string'?filename.match(/\.(pdf|epub|xml|gz|txt|jpe?g|png)$/i)?.[1]?.toUpperCase():null;
  const format=formats[attributes.media_type]??suffix??(locale==='ru'?'Файл':locale==='es'?'Archivo':'File');
  const bytes=attributes.byte_size;if(!Number.isSafeInteger(bytes)||bytes<0)return format;
  const unit=bytes>=1e6?1e6:bytes>=1e3?1e3:1;
  const unitLabel=locale==='ru'?{1:'Б',1000:'КБ',1000000:'МБ'}[unit]:{1:'B',1000:'KB',1000000:'MB'}[unit];
  return format+' · '+new Intl.NumberFormat(locale,{maximumFractionDigits:1}).format(bytes/unit)+' '+unitLabel;
}

// The technical escape hatch is an explicit export, not a JSON DOM subtree.
export function rawDataDownload(value,label=ui('Скачать данные'),filename='sophia-source.json'){
  const button=document.createElement('button');button.type='button';button.className='sc-data-download';uiText(button,label);
  button.addEventListener('click',()=>{
    const current=typeof value==='function'?value():value;
    if(current===undefined)return;
    const url=URL.createObjectURL(new Blob([JSON.stringify(current,null,2)],{type:'application/json'}));
    const link=document.createElement('a');link.href=url;link.download=filename;link.click();
    setTimeout(()=>URL.revokeObjectURL(url),1000);
  });
  return button;
}

// Authored UI translations of the existing predicate vocabulary. They name
// existing relations without adding, merging or evaluating source edges.
const predicates={
  canonized_by:['канонизировано','canonized by','canonizado por'],
  'commentary-on':['комментарий к','commentary on','comentario sobre'],commented_by:['комментируется','commented on by','comentado por'],
  contains_row:['содержит запись','contains record','contiene registro'],contains_view:['содержит представление','contains view','contiene vista'],
  contested_by:['оспаривается','contested by','cuestionado por'],'contextualized-by':['в контексте','contextualized by','en el contexto de'],
  descendant:['продолжает','descends from','continúa'],described_by:['описано в','described by','descrito por'],
  downloadable_at:['доступно для скачивания','download at','descargar en'],fragments_preserved_by:['фрагменты сохранены','fragments preserved by','fragmentos conservados por'],
  has_node_type_pressure:['требует уточнения типа','needs type clarification','requiere aclarar el tipo'],
  has_prepared_dossier:['имеет исследовательское досье','has research dossier','tiene un expediente de investigación'],
  has_relation_pressure:['требует уточнения связи','needs relation clarification','requiere aclarar la relación'],
  metadata_at:['описание доступно в','metadata at','descripción en'],mutation:['изменение','change','cambio'],parallel:['параллель','parallel','paralelo'],
  predecessor:['предшествует','predecessor','precede'],preserved_in:['сохранено в','preserved in','conservado en'],
  preserves_in:['сохраняет в','preserves in','conserva en'],receives_from:['получает от','receives from','recibe de'],
  survives_as:['сохранилось как','survives as','se conserva como'],tension:['напряжение','tension','tensión'],
  translated_into:['переведено на','translated into','traducido a'],transmits_to:['передаёт','transmits to','transmite a'],
  uncertain_relation:['предполагаемая связь','uncertain relation','relación incierta'],uses_language:['использует язык','uses language','usa el idioma'],
  uses_script:['использует письменность','uses script','usa la escritura']
};
const navigationPredicates={
  provision_activity:['Выходные сведения','Publication details','Datos de publicación'],
  grounds_source_backlog_anchor:['основание поиска источника','basis for source discovery','base para buscar la fuente'],
  has_anchor:['место в тексте','text location','lugar del texto'],
  has_record_version:['версия записи','record version','versión del registro'],
  has_source_planting:['содержит источник','includes source','incluye una fuente'],
  owns_manifest:['описание раздела','section description','descripción de la sección'],
  owns_resource:['материал раздела','section material','material de la sección'],
  has_subject:['предмет утверждения','claim subject','sujeto de la afirmación'],
  has_object:['объект утверждения','claim object','objeto de la afirmación'],
  embodied_by:['воплощено изданием','embodied by edition','plasmado en la edición'],
  has_expression:['Текст или перевод','Text or translation','Texto o traducción'],
  translated_by:['Переводчик','Translator','Traductor'],
  has_normalized_agent:['Участник','Participant','Participante'],
  has_normalized_place:['Место','Place','Lugar']
};
export function relationLabel(raw,locale=uiLanguage()){
  const id=raw?.predicate_id??'',display=raw?.display?.label??raw?.display;
  if(Object.hasOwn(navigationPredicates,id))return local(navigationPredicates,id,locale,'');
  const value=display?.[locale]||display?.default||display?.original||display?.en;
  if(value&&value!==id&&value!==raw?.relation_type_id)return value;
  return local(predicates,id,locale,locale==='ru'?'Связь':locale==='es'?'Relación':'Relation');
}
