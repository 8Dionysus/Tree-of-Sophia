// The research workbench. A chosen star's or link's card opens over the whole space and is laid out by the steps of
// interpretation the tree keeps apart: the source itself, its reading, and its comparison with other stars; the reader's
// own work closes the last of them. A link lies between its two stars, each in its own words, and what joins them stands
// in the middle, at the step of comparison.

export const BENCH_STEPS=['source','reading','comparison'];
const HEAD='h1,.eyebrow,.ps-paths,.ps-source-line,.ps-personal-title,.ps-kernel-actions,.ps-link-sentence,.edge-endpoints,.reading-actions,.ps-walk-branch';
const TOOLS='.ps-kernel-actions,.reading-actions,.ps-walk-branch';
const SOURCE='.ps-fact,.ps-fragment,.ps-core-formula,.ps-grounds,.ps-source-open,.ps-source-refs,.ps-author-works';
const COMPARISON='.connection-target,.reading-bottom';
// A wrapper that holds parts of several steps is opened, and each step keeps a shallow copy of it around its own parts.
const OPENS='.ps-core-card,.ps-core-link,.ps-authored';
// Parts that leave any wrapper: the head of the bench, the ladder it unfolds, and the two ends of a link.
const OWN=new Set(['head','ladder','from','to']);

/** The place on the bench of one part of a rendered card. */
export function benchZone(part,mode='node'){
  const zone=part.dataset?.zone||part.dataset?.end||(part.matches(HEAD)?'head':part.matches('.ps-ladder')?'ladder':part.matches(SOURCE)?'source':part.matches('.ps-kernel-personal')?'work':part.matches(COMPARISON)?'comparison':'reading');
  return mode==='edge'&&!OWN.has(zone)&&zone!=='work'?'relation':zone;
}

/** Lays a rendered card out as the workbench in place and returns the bench. */
export function layBench(content,{mode='node',words={},heading=null}={}){
  const doc=content.ownerDocument;
  const el=(tag,cls,text)=>{const node=doc.createElement(tag);node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const zones=new Map(),put=(zone,part)=>{if(!zones.has(zone))zones.set(zone,[]);zones.get(zone).push(part);};
  const lay=(part,place)=>{
    if(part.dataset?.zone||!part.matches(OPENS)){place(benchZone(part,mode),part);return;}
    const copies=new Map();
    for(const child of [...part.children])lay(child,(zone,node)=>{
      if(OWN.has(zone)){place(zone,node);return;}
      if(!copies.has(zone)){const copy=part.cloneNode(false);copies.set(zone,copy);place(zone,copy);}
      copies.get(zone).append(node);
    });
  };
  for(const part of [...content.children])lay(part,put);
  const board=el('div','ps-bench');board.dataset.card=mode;
  const head=el('header','ps-bench-head'),title=el('div','ps-bench-title'),tools=el('div','ps-bench-tools');
  for(const part of zones.get('head')??[])(part.matches(TOOLS)?tools:title).append(part);
  head.append(title);if(tools.childElementCount)head.append(tools);board.append(head);
  const column=(step,name,caption,current=false)=>{
    const part=el('section','ps-bench-step');part.dataset.step=step;
    if(name){const line=el('h2','ps-bench-step-title',name);if(current)line.setAttribute('aria-current','true');heading?.(line);part.append(line);}
    if(caption)part.append(el('p','ps-bench-step-caption',caption));
    const tray=el('div','ps-bench-tray');tray.append(...(zones.get(step)??[]));part.append(tray);board.append(part);return tray;
  };
  const work=zones.get('work')??[],keep=tray=>{if(!work.length)return;const box=el('section','ps-bench-work');box.append(...work);tray.append(box);};
  if(mode==='edge'){
    // Each end opens with the star itself; what the link draws from that star's source follows it.
    for(const side of ['from','to'])if(zones.has(side))zones.set(side,[...zones.get(side)].sort((a,b)=>Number(b.matches('.ps-bench-end'))-Number(a.matches('.ps-bench-end'))));
    column('from');keep(column('relation',words.comparison,words.comparisonCaption,true));column('to');
  }else{
    const rung=[...(zones.get('ladder')?.[0]?.children??[])].find(item=>item.getAttribute?.('aria-current')==='true')?.dataset.rung;
    for(const step of BENCH_STEPS){const tray=column(step,words[step],words[`${step}Caption`],step===rung);if(step==='comparison')keep(tray);}
  }
  content.replaceChildren(board);
  return board;
}
