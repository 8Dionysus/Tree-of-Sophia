import {ui,uiText,uiComputed,uiLanguage} from './ui-i18n.mjs';

const el=(tag,value='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,value);return node;};
// Only the owner-provided label is localized. Source wording is never run
// through the interface translation catalog or a browser-owned classifier.
const label=value=>uiComputed(()=>value[uiLanguage()]??value.en??Object.values(value)[0]);

export function renderReadableContexts(contexts,anchor){
  const section=el('section','','sc-readable-context');
  section.dataset.contextPresentation='complete';
  for(const [index,context]of contexts.entries()){
    const content=el('dl','','sc-form-values'),technical=el('details','','sc-context-technical');
    technical.append(el('summary',ui('Технические сведения контекста')));
    const technicalValues=el('dl','','sc-form-values');technical.append(technicalValues);
    for(const [ordinal,entry]of context.entries.entries()){
      const row=el('div','','sc-form-context');row.dataset.contextCategory=entry.category;
      row.dataset.readingAnchor=`${anchor}:${index}:${ordinal}`;
      row.append(el('dt',label(entry.label),'sc-form-context-slot'));
      if(entry.category==='unclassified')row.append(el('code',entry.key,'sc-source-ref'),el('small',ui('Не классифицировано; исходное значение сохранено.'),'sc-reader-gap'));
      if(entry.explanation)row.append(el('p',label(entry.explanation),'sc-reader-language-note'));
      if(entry.value_label)row.append(el('dd',label(entry.value_label),'sc-form-value'));
      const value=el(['array','object'].includes(entry.display.type)?'pre':'dd',entry.display.text,'sc-form-value');
      value.dir='auto';if(entry.language)value.lang=entry.language;
      if(!entry.value_label)row.append(value);
      const origin=el('details','','sc-context-location');origin.append(el('summary',ui('Поле источника')));
      if(entry.value_label)origin.append(value);
      origin.append(el('pre',JSON.stringify(entry.binding,null,2),'sc-source-ref'));row.append(origin);
      (entry.category==='technical'?technicalValues:content).append(row);
    }
    section.append(content);if(technicalValues.children.length)section.append(technical);
  }
  return section;
}
