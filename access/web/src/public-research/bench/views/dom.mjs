// Small helpers the bench's views share.
import {signMarkup} from '../../space-glyphs.mjs';

export function el(tag,cls,text){const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined&&text!==null)node.textContent=text;return node;}
export function btn(label,cls,hint){const node=el('button',cls,label);node.type='button';if(hint){node.setAttribute('aria-label',hint);node.title=hint;}return node;}
export function sign(kind){const span=el('span','bn-sign');span.innerHTML=signMarkup(kind)||'';return span;}
export function section(title,cls=''){const box=el('section',`bn-section ${cls}`.trim());box.append(el('h3','bn-section-title',title));return box;}
export const localized=(value,locale)=>{
  if(typeof value==='string')return value;if(!value||typeof value!=='object')return '';
  const lang=String((typeof locale==='function'?locale():locale)||'ru').toLowerCase();
  return value[lang]??value[lang.split('-')[0]]??value.en??value.ru??'';
};
/** The chapter's title as a witness prints it, without its numbering or closing period. */
export const chapterName=(titles,language)=>String(titles?.[language]??titles?.de??'').replace(/^[IVXLC]+\.\s*/,'').replace(/\.$/,'').trim();
/** A shown offset inside an element's text, from a point of a DOM range. */
export function offsetIn(element,node,offset){
  const range=document.createRange();range.selectNodeContents(element);
  try{range.setEnd(node,offset);}catch{return null;}
  return range.toString().length;
}
/** The DOM point at a shown offset inside an element's text. */
export function pointAt(element,target){
  const walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT);let left=target,node;
  while((node=walker.nextNode())){const length=node.nodeValue.length;if(left<=length)return {node,offset:left};left-=length;}
  return {node:element,offset:element.childNodes.length};
}
