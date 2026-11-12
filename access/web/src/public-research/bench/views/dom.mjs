// Small helpers the bench's views share.
import {signMarkup} from '../../space-glyphs.mjs';
import {chapterTitle} from '../../chapter-title.mjs';
import {localeValue} from '../../locale.mjs';

export function el(tag,cls,text){const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined&&text!==null)node.textContent=text;return node;}
export function btn(label,cls,hint){const node=el('button',cls,label);node.type='button';if(hint){node.setAttribute('aria-label',hint);node.title=hint;}return node;}
export function sign(kind){const span=el('span','bn-sign');span.innerHTML=signMarkup(kind)||'';return span;}
export function section(title,cls=''){const box=el('section',`bn-section ${cls}`.trim());box.append(el('h3','bn-section-title',title));return box;}
/** A {ru, en, …} text in the reader's language, as the stand reads its records: else the English through the catalogs. */
export const localized=(value,locale)=>localeValue(value,(typeof locale==='function'?locale():locale)||'ru');
/** A chapter's title in a witness's language, the original's where that witness leaves the chapter out. */
export const chapterName=(titles,language)=>chapterTitle({titles},language)||chapterTitle({titles},'de');
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
