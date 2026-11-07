// A star's or a link's card on the bench, drawn by the stand's own card (public-space.mjs) into the window. A star
// named inside the card opens in the same window, and the window remembers the way back; with Shift it opens beside.

import {measureOf} from '../breathing.mjs';
import {el} from './dom.mjs';

export function cardView({root,windowId,ctx}){
  const body=el('div','bn-card ps-panel ps-panel-content');root.append(body);
  let win=null;
  const go={
    subject:(subject,{beside=false}={})=>{if(!subject)return;if(beside)ctx.open(subject,{place:'auto'});else ctx.replace(windowId,subject);},
    open:(subject,options)=>ctx.open(subject,options),
    compare:()=>ctx.compare?.(),
    read:(cite)=>{if(cite?.c)ctx.open({kind:'chapter',chapter:cite.c,language:ctx.readingLanguage?.()??'de',unit:cite.u?.[0]});},
  };
  function render(){
    body.replaceChildren();
    const drawn=ctx.renderCard?.(body,win.subject,go);
    if(!drawn)body.append(el('p','bn-failed',ctx.t('unavailable')));
    ctx.markTerms?.(body);
  }
  return {
    update(next){win=next;render();root.scrollTop=0;},
    title:w=>ctx.titleOf?.(w.subject)||ctx.t('view_card'),
    sign:w=>ctx.kindOf?.(w.subject)??'concept',
    measure:ch=>measureOf('card',{ch}),
    dispose(){},
  };
}
