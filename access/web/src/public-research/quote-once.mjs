// A passage of the book is quoted once on a page. Where a later ground repeats lines already quoted
// above, those lines step back; where it repeats them all, a single line leads back to the first
// quotation. Quotations arrive after the page and change when the reader switches language, so the
// page is read again whenever its quotations change.

const QUOTE='blockquote.ps-fragment-text',LINE=':scope > *';
// A line is the same line whether or not a quotation marks it as cut short.
const keyOf=node=>`${node.closest('[lang]')?.getAttribute('lang')??''}\u0000${node.textContent.replace(/\s+/gu,' ').trim().replace(/^…\s*|\s*…$/gu,'')}`;

/** Keep every quoted line once in `root`; `label(source, arrow)` names the line that leads to the first quotation,
 * the arrow pointing up or down to where it stands. */
/** `later` names figures that defer to every other figure: a short note in a margin leads back to the text it repeats. */
export function keepQuotesOnce(root,{label=(source,arrow)=>`${arrow} ${source}`,later=null}={}){
  if(typeof root?.querySelectorAll!=='function')return {refresh(){},dispose(){}};
  let working=false,frame=0,observer=null;
  // A figure stands on the page by itself or, deferred, in its note in the margin.
  const unit=figure=>(later&&figure.closest(later))||figure;
  const figureOf=node=>node?.tagName==='FIGURE'?node:node?.querySelector?.('figure')??null;
  const folded=new Set();
  function refresh(){
    working=true;
    try{
      for(const node of folded)node.hidden=false;folded.clear();
      const first=new Map(),leads=new Map();
      const figures=[...root.querySelectorAll('figure')];
      if(later){const deferred=figure=>figure.closest(later)?1:0;figures.sort((a,b)=>deferred(a)-deferred(b));}
      for(const figure of figures){
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
        // Two ways back to the same quotation side by side are one.
        leads.set(figure,lead);
        const before=figureOf(unit(figure).previousElementSibling);
        if(lead&&before&&leads.get(before)===lead){const node=unit(figure);node.hidden=true;folded.add(node);continue;}
        const below=lead&&typeof lead.compareDocumentPosition==='function'&&(figure.compareDocumentPosition(lead)&4);
        const back=document.createElement('button');back.type='button';back.className='text-button ps-fragment-repeat';back.textContent=label(source,below?'↓':'↑');
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
