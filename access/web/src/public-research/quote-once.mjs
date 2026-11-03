// A passage of the book is quoted once on a page. Where a later ground repeats lines already quoted
// above, those lines step back; where it repeats them all, a single line leads back to the first
// quotation. Quotations arrive after the page and change when the reader switches language, so the
// page is read again whenever its quotations change.

const QUOTE='blockquote.ps-fragment-text',LINE=':scope > *';
const keyOf=node=>`${node.closest('[lang]')?.getAttribute('lang')??''}\u0000${node.textContent.replace(/\s+/gu,' ').trim()}`;

/** Keep every quoted line once in `root`; `label(source)` names the line that leads to the first quotation. */
export function keepQuotesOnce(root,{label=source=>`↑ ${source}`}={}){
  if(typeof root?.querySelectorAll!=='function')return {refresh(){},dispose(){}};
  let working=false,frame=0,observer=null;
  function refresh(){
    working=true;
    try{
      const first=new Map();
      for(const figure of root.querySelectorAll('figure')){
        figure.querySelector(':scope > .ps-fragment-repeat')?.remove();
        const quotes=[...figure.querySelectorAll(QUOTE)];if(!quotes.length)continue;
        let shown=0;
        for(const quote of quotes){
          quote.hidden=false;
          for(const line of quote.querySelectorAll(LINE)){
            line.classList.remove('is-quoted-above');
            if(!line.textContent.trim())continue;
            const key=keyOf(line);
            if(first.has(key)&&first.get(key)!==figure)line.classList.add('is-quoted-above');
            else{first.set(key,figure);shown++;}
          }
        }
        for(const part of figure.querySelectorAll(':scope > figcaption, :scope > .ps-fragment-tools'))part.hidden=false;
        if(shown)continue;
        // Every line stands above: the figure becomes the way back to it.
        const lead=first.get(keyOf(quotes[0].querySelector(LINE)??quotes[0]));
        const source=figure.querySelector('.ps-fragment-source')?.textContent??'';
        for(const quote of quotes)quote.hidden=true;
        for(const part of figure.querySelectorAll(':scope > figcaption, :scope > .ps-fragment-tools'))part.hidden=true;
        const back=document.createElement('button');back.type='button';back.className='text-button ps-fragment-repeat';back.textContent=label(source);
        back.addEventListener('click',()=>{
          for(let node=lead?.parentElement;node;node=node.parentElement)if(node.tagName==='DETAILS')node.open=true;
          lead?.scrollIntoView({behavior:'smooth',block:'center'});
        });
        figure.prepend(back);
      }
    }finally{observer?.takeRecords();working=false;}
  }
  observer=typeof MutationObserver==='function'?new MutationObserver(()=>{if(working||frame)return;frame=requestAnimationFrame(()=>{frame=0;if(root.isConnected)refresh();else observer.disconnect();});}):null;
  observer?.observe(root,{childList:true,subtree:true});
  refresh();
  return {refresh,dispose(){observer?.disconnect();cancelAnimationFrame?.(frame);}};
}
