import {t,ui,uiChildren,uiText} from './ui-i18n.mjs';
import {nodePreview,describePreview} from './graph-preview.mjs';
import {localized} from './knowledge-client.mjs';

export function inclusionDescription(packet,nodeId){
  if(packet?.inclusion?.authority!=='query-execution-not-semantic-proof')return '';
  const cause=packet.inclusion.nodes?.[nodeId];
  if(cause?.kind==='selector')return t("Эта звезда соответствует условиям выбора исходных узлов.");
  if(cause?.kind==='focus')return t("Центр выбранной области.");
  if(!['traversal','endpoint'].includes(cause?.kind))return '';
  const relation=packet.relations.find(r=>r.id===cause.via_relation_id),via=packet.nodes.find(n=>n.id===cause.via_node_id);
  const parts=[t("Эта звезда добавлена окружением; условия исходных узлов не обязаны выполняться.")];
  if(relation)parts.push(t("Через связь «{0}»{1}.", [localized(relation.display?.label,relation.id), (via?t(" с «{0}»", [localized(via.display?.title,via.id)]):'')]));
  if(Number.isInteger(cause.depth)&&cause.depth>0)parts.push(t("Шаг от исходной области: {0}.", [cause.depth]));
  return parts.join(' ');
}

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
  const legend=document.createElement('div');legend.className='sc-inclusion-legend';legend.hidden=true;uiChildren(root.querySelector('.sc-context'), "append", legend);
  let previous=null,timer=null;
  const context=document.createElement('details');context.className='sc-node-context';context.hidden=true;
  const summary=document.createElement('summary');uiText(summary, ui("Почему звезда в этой области"));const explanation=document.createElement('p');uiChildren(context, "append", summary, explanation);root.querySelector('.sc-provenance').after(context);
  function selectionContext(){
    const selection=scene.port.selection;uiText(explanation, selection.relationId?'':inclusionDescription(scene.port.packet,selection.nodeId));context.hidden=!explanation.textContent;
  }
  function update(){
    selectionContext();const packet=scene.port.packet;if(!packet||packet===previous)return;
    clearTimeout(timer);const oldIds=new Set(previous?.nodes.map(n=>n.id)||[]),roles=inclusionRoles(packet),totals={matched:0,focus:0,context:0};
    for(const element of root.querySelectorAll('.sc-node')){const role=roles.get(element.dataset.id);element.dataset.inclusion=role||'';element.dataset.entering=String(Boolean(previous&&!oldIds.has(element.dataset.id)));
      const raw=packet.nodes.find(node=>node.id===element.dataset.id);
      describePreview(element,nodePreview(packet,raw));
      if(role)totals[role]++;

    }
    uiChildren(legend, "replaceChildren");for(const [role,label]of [['matched',ui("По условиям")],['focus',ui("Центр")],['context',ui("Окружение")]])if(totals[role]){const item=document.createElement('span');item.dataset.role=role;uiText(item, label+' '+totals[role]);uiChildren(legend, "append", item);}
    legend.hidden=!roles.size;previous=packet;scene.invalidate();
    timer=setTimeout(()=>{for(const element of root.querySelectorAll('.sc-node[data-entering="true"]'))element.dataset.entering='false';},1600);
  }
  const observer=new MutationObserver(update);observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision','data-node-count','data-relation-count','data-selected','data-inspector-id','data-inspector-kind']});
  update();window.addEventListener('pagehide',()=>{clearTimeout(timer);});return {update};
}
