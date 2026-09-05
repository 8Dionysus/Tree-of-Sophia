export function inclusionRoles(packet){
  const result=new Map();
  if(packet?.inclusion?.authority!=='query-execution-not-semantic-proof')return result;
  for(const node of packet.nodes){const kind=packet.inclusion.nodes?.[node.id]?.kind;
    const role=kind==='selector'?'matched':kind==='focus'?'focus':['traversal','endpoint'].includes(kind)?'context':null;
    if(role)result.set(node.id,role);
  }
  return result;
}
export function createSceneFeedback(root,scene){
  const legend=document.createElement('div');legend.className='sc-inclusion-legend';legend.hidden=true;legend.title='Причина появления в области; не оценка истинности или значимости';root.querySelector('.sc-context').append(legend);
  let previous=null,timer=null,activeHint=null,hintId=0;
  const descriptions={matched:'Эта звезда соответствует условиям линзы.',focus:'Центр выбранной области.',context:'Эта звезда добавлена через связи с выбранной областью.'};
  function hideHint(){if(!activeHint)return;activeHint.querySelector('.sc-inclusion-hint').hidden=true;delete activeHint.dataset.hintOpen;activeHint=null;}
  function showHint(element){
    if(!element||element.hidden||element===activeHint)return;
    hideHint();const hint=element.querySelector('.sc-inclusion-hint');if(!hint)return;
    activeHint=element;hint.hidden=false;element.dataset.hintOpen='true';
    // Measure only when opening; the hint then travels with the existing node.
    const area=root.getBoundingClientRect(),node=element.getBoundingClientRect(),box=hint.getBoundingClientRect();
    const x=Math.max(area.left+12,Math.min(node.left+(node.width-box.width)/2,area.right-box.width-12));
    const below=node.bottom+22,y=below+box.height<area.bottom-90?below:node.top-box.height-22;
    hint.style.left=(x-node.left)+'px';hint.style.top=(Math.max(area.top+90,y)-node.top)+'px';
  }
  root.addEventListener('pointerover',event=>showHint(event.target.closest?.('.sc-node')));
  root.addEventListener('pointerout',event=>{if(activeHint?.contains(event.target)&&!activeHint.contains(event.relatedTarget)&&!activeHint.contains(document.activeElement))hideHint();});
  root.addEventListener('focusin',event=>showHint(event.target.closest?.('.sc-node')));
  root.addEventListener('focusout',event=>{if(activeHint?.contains(event.target)&&!activeHint.contains(event.relatedTarget))hideHint();});
  window.addEventListener('keydown',event=>{if(event.key==='Escape'&&activeHint){hideHint();event.preventDefault();event.stopPropagation();}},{capture:true});
  root.addEventListener('pointerdown',hideHint,{capture:true});root.addEventListener('click',hideHint);
  root.addEventListener('wheel',hideHint,{passive:true});
  window.addEventListener('resize',hideHint);
  function update(){
    const packet=scene.port.packet;if(!packet||packet===previous)return;
    hideHint();clearTimeout(timer);const oldIds=new Set(previous?.nodes.map(n=>n.id)||[]),roles=inclusionRoles(packet),totals={matched:0,focus:0,context:0};
    for(const element of root.querySelectorAll('.sc-node')){const role=roles.get(element.dataset.id);element.dataset.inclusion=role||'';element.dataset.entering=String(Boolean(previous&&!oldIds.has(element.dataset.id)));
      let hint=element.querySelector('.sc-inclusion-hint');
      if(role){totals[role]++;if(!hint){hint=document.createElement('span');hint.className='sc-inclusion-hint';hint.id='sc-inclusion-hint-'+(++hintId);hint.setAttribute('role','tooltip');element.append(hint);}hint.hidden=true;hint.textContent=descriptions[role];element.setAttribute('aria-describedby',hint.id);}
      else{hint?.remove();element.removeAttribute('aria-describedby');}
    }
    legend.replaceChildren();for(const [role,label]of [['matched','По условиям'],['focus','Центр'],['context','Окружение']])if(totals[role]){const item=document.createElement('span');item.dataset.role=role;item.textContent=label+' '+totals[role];legend.append(item);}
    legend.hidden=!roles.size;previous=packet;scene.invalidate();
    timer=setTimeout(()=>{for(const element of root.querySelectorAll('.sc-node[data-entering="true"]'))element.dataset.entering='false';},1600);
  }
  const observer=new MutationObserver(update);observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision','data-node-count','data-relation-count']});
  update();window.addEventListener('pagehide',()=>{hideHint();clearTimeout(timer);});return {update};
}
