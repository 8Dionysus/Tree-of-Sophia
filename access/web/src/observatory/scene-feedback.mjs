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
  let previous=null,timer=null;
  const descriptions={matched:'выбрано условиями',focus:'центр области',context:'добавлено окружением'};
  function update(){
    const packet=scene.port.packet;if(!packet||packet===previous)return;
    clearTimeout(timer);const oldIds=new Set(previous?.nodes.map(n=>n.id)||[]),roles=inclusionRoles(packet),totals={matched:0,focus:0,context:0};
    for(const element of root.querySelectorAll('.sc-node')){const role=roles.get(element.dataset.id);element.dataset.inclusion=role||'';element.dataset.entering=String(Boolean(previous&&!oldIds.has(element.dataset.id)));
      if(role){totals[role]++;element.setAttribute('aria-label',element.getAttribute('aria-label')+' · '+descriptions[role]);}
    }
    legend.replaceChildren();for(const [role,label]of [['matched','◈ По условиям'],['focus','✧ Центр'],['context','◇ Окружение']])if(totals[role]){const item=document.createElement('span');item.dataset.role=role;item.textContent=label+' '+totals[role];legend.append(item);}
    legend.hidden=!roles.size;previous=packet;scene.invalidate();
    timer=setTimeout(()=>{for(const element of root.querySelectorAll('.sc-node[data-entering="true"]'))element.dataset.entering='false';},1600);
  }
  const observer=new MutationObserver(update);observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision','data-node-count','data-relation-count']});
  update();window.addEventListener('pagehide',()=>clearTimeout(timer));return {update};
}
