import {ui,uiText,uiComputed,uiLanguage} from './ui-i18n.mjs';
import {renderContextData,hasContextPresentation,contextScalar} from './context-view.mjs';
import {languageName} from './human-presentation.mjs';
const el=(tag,value='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,value);return node;};
const label=value=>uiComputed(()=>value[uiLanguage()]??value.en??Object.values(value)[0]);
const recordFields=new Set(['claim_type','assertion_layer','epistemic_status','review_status','visibility','identity_status','confidence','same_as_posture']);

export function renderReadableContexts(contexts,anchor,{resolveValue}={}){
  const section=el('section','','sc-readable-context'),record=el('dl','','sc-form-values');section.dataset.contextPresentation='complete';const seen=new Set();
  for(const [index,context]of contexts.entries()){
    const content=el('dl','','sc-form-values');
    for(const [ordinal,entry]of context.entries.entries()){
      // Sidecar categories and labels are source-owned. Explanations document
      // those fields; their absence does not alter the source fact's value.
      if(entry.category==='technical'||entry.key==='same_as_posture'&&entry.display.text==='no_equivalence_claim'
        ||entry.binding?.source_pointer?.startsWith('/field_languages/'))continue;
      const stamp=JSON.stringify([entry.key,entry.display,entry.value_label]);if(seen.has(stamp))continue;seen.add(stamp);
      if(entry.display.type==='null'||entry.display.text==='')continue;
      const resolved=resolveValue?.(entry);
      if(!resolved&&!entry.value_label&&!hasContextPresentation(entry.key))continue;
      const target=recordFields.has(entry.key)?record:content;
      if(entry.category==='unclassified'){
        let value=entry.display.text;
        if(['object','array'].includes(entry.display.type)){try{value=JSON.parse(value);}catch{continue;}}
        const human=renderContextData({[entry.key]:value},`${anchor}:${index}:${ordinal}`);
        if(human.children.length)target.append(human);continue;
      }
      const row=el('div','','sc-form-context');row.dataset.contextCategory=entry.category;row.dataset.readingAnchor=`${anchor}:${index}:${ordinal}`;
      const compound=['object','array'].includes(entry.display.type);
      if(!compound)row.append(el('dt',label(entry.label),'sc-form-context-slot'));
      if(resolved)row.append(el('dd',resolved,'sc-form-value'));
      else if(entry.value_label)row.append(el('dd',label(entry.value_label),'sc-form-value'));
      else if(['object','array'].includes(entry.display.type)){
        let value;try{value=JSON.parse(entry.display.text);}catch{continue;}
        const rendered=renderContextData({[entry.key]:value},`${anchor}:${index}:${ordinal}`);if(!rendered.children.length)continue;
        if(['semantic_content','notes'].includes(entry.key)&&entry.language&&entry.language.split('-')[0]!==uiLanguage()){
          const original=el('details');original.append(el('summary',uiComputed(()=>`${ui('Описание источника')} · ${languageName(entry.language)}`)),rendered);row.append(original);
        }else row.append(rendered);
      }else{
        const text=contextScalar(entry.display.type==='boolean'?entry.display.text==='true':entry.display.text,entry.key);if(text===null)continue;
        const value=el('dd',text,'sc-form-value');value.dir='auto';if(entry.language)value.lang=entry.language;row.append(value);
      }
      target.append(row);
    }
    if(content.children.length)section.append(content);
  }
  if(record.children.length){const details=el('details');details.append(el('summary',ui('Сведения о записи')),record);section.append(details);}
  return section;
}
