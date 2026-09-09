import {UI_CATALOG} from './ui-catalog.mjs';

export const UI_LANGUAGES=['ru','en','es'];
let language='ru';
try{const saved=JSON.parse(globalThis.localStorage?.getItem('tos-observatory-interface-v1')||'null')?.uiLanguage;if(UI_LANGUAGES.includes(saved))language=saved;}catch{}
export const uiLanguage=()=>language;
export function t(source,values=[]){
  const template=language==='ru'?source:UI_CATALOG[source]?.[language]??source;
  return template.replace(/\{(\d+)\}/g,(match,index)=>index<values.length?String(values[index]):match);
}
// Only authored UI messages carry this marker. Raw strings from a source,
// server, user note or input remain raw, even if they match a catalog entry.
class UIMessage extends String{
  constructor(source,values){super(t(source,values));this.source=source;this.values=values;}
  toString(){return this.compute?this.compute():(this.padding?.[0]||'')+t(this.source,this.values)+(this.padding?.[1]||'');}
  valueOf(){return this.toString();}
  [Symbol.toPrimitive](){return this.toString();}
  toJSON(){return this.toString();}
  toLowerCase(){return uiComputed(()=>this.toString().toLocaleLowerCase(language));}
  toUpperCase(){return uiComputed(()=>this.toString().toLocaleUpperCase(language));}
}
export const ui=(source,values=[])=>new UIMessage(source,values);
export function uiComputed(compute){const message=ui('');message.compute=compute;return message;}
const bindings=new WeakMap(),targets=new Set();let cleanupPending=false;
function record(target,key,value){
  let entries=bindings.get(target);
  if(value instanceof UIMessage){
    if(!entries){entries=new Map();bindings.set(target,entries);targets.add(new WeakRef(target));}
    entries.set(key,value);
  }else entries?.delete(key);
  if(targets.size>500&&!cleanupPending){cleanupPending=true;queueMicrotask(()=>{cleanupPending=false;for(const ref of targets){const node=ref.deref();if(!node||!node.isConnected){targets.delete(ref);if(node)bindings.delete(node);}}});}
}
export function uiText(element,value=''){
  // Bind the text node so later icons, inputs and counts survive a language
  // switch. A label's textContent must never replace its nested control.
  if(value instanceof UIMessage&&element.nodeType!==Node.TEXT_NODE){
    element.textContent='';const node=document.createTextNode(String(value));element.append(node);record(node,'textContent',value);
  }else{record(element,'textContent',value);element.textContent=String(value??'');}
  return value;
}
export function uiAttribute(element,name,value){
  record(element,name,value);element.setAttribute(name,String(value));
}
export function uiNode(value){const node=document.createTextNode('');uiText(node,value);return node;}
export function uiChildren(element,method,...values){element[method](...values.map(value=>value instanceof Node?value:uiNode(value)));}
// This entry point is only for static, authored markup in shell/panel modules.
// Server and user strings are inserted with textContent, never through here.
export function uiHTML(element,markup){
  element.innerHTML=markup;
  const walk=document.createTreeWalker(element,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT);
  let node;
  while((node=walk.nextNode())){
    if(node.nodeType===Node.TEXT_NODE){const text=node.textContent,trimmed=text.trim();if(UI_CATALOG[trimmed]){const message=ui(trimmed),start=text.indexOf(trimmed);message.padding=[text.slice(0,start),text.slice(start+trimmed.length)];uiText(node,message);}}
    else for(const name of ['aria-label','placeholder','title','data-tooltip']){const value=node.getAttribute(name);if(value&&UI_CATALOG[value])uiAttribute(node,name,ui(value));}
  }
}
export function setUiLanguage(value){
  if(!UI_LANGUAGES.includes(value))throw new Error('Unsupported interface language');
  if(typeof document!=='undefined'){document.documentElement.lang=value;document.title=value==='ru'?'Древо Софии':UI_CATALOG['Древо Софии']?.[value]||'Tree of Sophia';}
  if(language===value)return;
  language=value;
  for(const ref of targets){const target=ref.deref();if(!target||!target.isConnected){targets.delete(ref);if(target)bindings.delete(target);continue;}
    for(const [key,message]of bindings.get(target)||[]){if(key==='textContent')target.textContent=String(message);else target.setAttribute(key,String(message));}
  }
  if(typeof document!=='undefined'){document.documentElement.lang=language;document.title=t('Древо Софии');document.dispatchEvent(new CustomEvent('sophia-ui-language',{detail:{language}}));}
}
