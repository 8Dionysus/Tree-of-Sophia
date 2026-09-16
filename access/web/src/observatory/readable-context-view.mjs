import {ui,uiText,uiComputed,uiLanguage} from './ui-i18n.mjs';
import {renderContextData,hasContextPresentation,contextScalar,contextLabel} from './context-view.mjs';
import {languageName,rawDataDownload} from './human-presentation.mjs';
const el=(tag,value='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,value);return node;};
const label=value=>uiComputed(()=>value[uiLanguage()]??value.en??Object.values(value)[0]);
const recordFields=new Set(['claim_type','assertion_layer','epistemic_status','review_status','visibility','identity_status','confidence','same_as_posture']);
const referenceField=key=>/(?:^|_)(?:ids?|refs?|digests?|sha256|pointers?|schema|version)$/.test(key)||['predicate','source_graph'].includes(key);
// This fallback handles unclassified source values only. Authored wording in
// named text fields is never filtered by its spelling.
const opaqueScalar=text=>/^(?:tos\.[^\s]+|sha256:[a-f0-9]+|[a-f0-9]{32,}|[a-z]+(?:_[a-z0-9]+)+|(?:source-navigation|source-claims|philosophy|repository):[^\s]+)$/.test(text)
  ||/^(?:https?:\/\/|ToS\/|\/|[A-Za-z]:\\)/.test(text);
function sourceScalar(entry){
  if(referenceField(entry.key))return null;
  if(entry.display.type==='number')return entry.display.text;
  if(entry.display.type==='boolean')return entry.display.text==='true'?ui('Да'):ui('Нет');
  return entry.display.type==='string'&&!opaqueScalar(entry.display.text)?entry.display.text:null;
}
function appendSourceValue(parent,entry,anchor){
  const value=sourceScalar(entry);if(value===null)return false;
  const row=el('div','','sc-form-context');row.dataset.readingAnchor=anchor;
  const title=entry.label[uiLanguage()];
  if(title&&title!==entry.key&&!['Неразобранное поле источника','Unclassified source field'].includes(title))row.append(el('dt',label(entry.label)));
  const body=document.createElement('dd');body.className='sc-form-value';body.dir='auto';if(entry.language)body.lang=entry.language;
  // Source strings and number lexemes remain literal, including words that
  // also happen to be present in the interface translation catalog.
  if(entry.display.type==='boolean')uiText(body,value);else body.textContent=value;
  row.append(body);parent.append(row);return true;
}

export function renderReadableContexts(contexts,anchor,{resolveValue}={}){
  const section=el('section','','sc-readable-context'),record=el('dl','','sc-form-values'),additional=el('dl','','sc-form-values');
  section.dataset.contextPresentation='selected';const seen=new Set();let hasAdditional=false;
  for(const [index,context]of contexts.entries()){
    const content=el('dl','','sc-form-values');
    for(const [ordinal,entry]of context.entries.entries()){
      // Sidecar categories and labels are source-owned. Explanations document
      // those fields; their absence does not alter the source fact's value.
      if(entry.category==='technical'||entry.key==='same_as_posture'&&entry.display.text==='no_equivalence_claim'
        ||entry.binding?.source_pointer?.startsWith('/field_languages/'))continue;
      const stamp=JSON.stringify([entry.key,entry.binding?.source_pointer,entry.display,entry.value_label]);if(seen.has(stamp))continue;seen.add(stamp);
      if(entry.display.type==='null'||entry.display.text==='')continue;
      const nameStatus=entry.key==='status'&&/^\/variant_labels\/\d+\/status$/.test(entry.binding?.source_pointer??'');
      if(nameStatus&&entry.display.text==='verified'){
        const row=el('div','','sc-form-context');row.dataset.readingAnchor=`${anchor}:${index}:${ordinal}`;
        row.append(el('dt',label({ru:'Проверка названия',en:'Title verification',es:'Verificación del título'})),el('dd',ui('Проверено'),'sc-form-value'));content.append(row);continue;
      }
      const resolved=resolveValue?.(entry);
      if(!resolved&&!entry.value_label&&!hasContextPresentation(entry.key)){
        if(referenceField(entry.key))continue;
        if(entry.category==='governing'&&appendSourceValue(content,entry,`${anchor}:${index}:${ordinal}`))continue;
        hasAdditional=true;appendSourceValue(additional,entry,`${anchor}:${index}:${ordinal}`);continue;
      }
      const target=recordFields.has(entry.key)?record:content;
      const row=el('div','','sc-form-context');row.dataset.contextCategory=entry.category;row.dataset.readingAnchor=`${anchor}:${index}:${ordinal}`;
      const compound=['object','array'].includes(entry.display.type);
      const nameLanguage=entry.key==='language'&&/^\/variant_labels\/\d+\/language$/.test(entry.binding?.source_pointer??'');
      if(!compound)row.append(el('dt',nameLanguage?label({ru:'Язык названия',en:'Title language',es:'Idioma del título'}):entry.category==='unclassified'?contextLabel(entry.key)||label(entry.label):label(entry.label),'sc-form-context-slot'));
      if(resolved)row.append(el('dd',resolved,'sc-form-value'));
      else if(entry.value_label)row.append(el('dd',label(entry.value_label),'sc-form-value'));
      else if(['object','array'].includes(entry.display.type)){
        let value;try{value=JSON.parse(entry.display.text);}catch{continue;}
        const rendered=renderContextData({[entry.key]:value},`${anchor}:${index}:${ordinal}`);if(!rendered.children.length){hasAdditional=true;continue;}
        if(['semantic_content','notes'].includes(entry.key)&&entry.language&&entry.language.split('-')[0]!==uiLanguage()){
          const original=el('details');original.append(el('summary',uiComputed(()=>`${ui('Описание источника')} · ${languageName(entry.language)}`)),rendered);row.append(original);
        }else row.append(rendered);
      }else{
        const text=contextScalar(entry.display.type==='boolean'?entry.display.text==='true':entry.display.text,entry.key);if(text===null){if(!referenceField(entry.key))hasAdditional=true;continue;}
        const value=el('dd',text,'sc-form-value');value.dir='auto';if(entry.language)value.lang=entry.language;row.append(value);
      }
      target.append(row);
    }
    if(content.children.length)section.append(content);
  }
  if(record.children.length){const details=el('details');details.append(el('summary',ui('Сведения о записи')),record);section.append(details);}
  if(hasAdditional){const details=el('details');details.append(el('summary',ui('Дополнительные сведения')),additional,rawDataDownload(contexts,ui('Скачать данные'),'sophia-context.json'));section.append(details);}
  return section;
}
