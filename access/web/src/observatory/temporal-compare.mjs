import {ContractError,RevisionError,sameJson} from './knowledge-client.mjs';
import {validateHumanForms} from './human-forms.mjs';
import {ui,uiChildren} from './ui-i18n.mjs';
import {rawDataDownload,sourceLinkLabel} from './human-presentation.mjs';

const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const requireValue=value=>{if(!value)throw new ContractError('Invalid exact date-comparison response.');};
const relations=new Set(['before','after','equal','contains','contained-by','overlaps']);
const relationLabels=Object.freeze({before:'Раньше',after:'Позже',equal:'Совпадают',contains:'Первый диапазон включает второй',
  'contained-by':'Второй диапазон включает первый',overlaps:'Пересекаются'});

/** Keep comparable relation wording in the shared reactive UI catalog. */
export function temporalComparisonRelationLabel(relation){
  return ui(relationLabels[relation]??'Сопоставление выполнено');
}

export function temporalComparisonRequest(left,right){
  const operand=reading=>{
    const id=reading?.raw?.id,revision=reading?.raw?.content_revision;
    if(reading?.kind!=='node'||typeof id!=='string'||!id||id.length>1024||id.trim()!==id||!hash(revision)||!hash(reading.sourceRevision))throw new ContractError('Date comparison requires two exact node cards.');
    return {node_id:id,content_revision:revision};
  };
  const a=operand(left),b=operand(right);if(left.sourceRevision!==right.sourceRevision)throw new RevisionError();
  return {schema_version:'tos_temporal_comparison_request_v1',source_revision:left.sourceRevision,left:a,right:b};
}

export async function compareExactDates(client,left,right,{signal}={}){
  const request=temporalComparisonRequest(left,right),packet=await client.request('/temporal/compare',{signal,body:request,maxResponseBytes:Math.min(client.maxResponseBytes??2097152,2097152)});
  requireValue(packet.schema_version==='tos_temporal_comparison_result_v1'&&sameJson(packet.request,request));
  if(packet.source_revision!==request.source_revision)throw new RevisionError();
  const c=packet.comparison,boundary=packet.authority_boundary;
  requireValue(object(c)&&['comparable','undetermined','unsupported'].includes(c.status)&&c.basis==='normalized-source-date-envelopes'
    &&Array.isArray(c.reasons)&&c.reasons.length<=128&&c.reasons.every(reason=>object(reason)&&['left','right','pair'].includes(reason.side)&&typeof reason.code==='string'&&reason.code.length>0&&reason.code.length<=2048)
    &&(c.status==='comparable'?relations.has(c.relation)&&c.reasons.length===0:c.relation===null&&c.reasons.length>0));
  requireValue(object(boundary)&&['is_source','writes_to_tree','performs_assessment','creates_inferred_claim'].every(key=>boundary[key]===false)
    &&boundary.comparison_basis==='normalized-source-date-envelopes'&&typeof boundary.note==='string'&&boundary.note.length>0);
  for(const side of ['left','right']){
    const operand=packet[side];requireValue(object(operand)&&operand.claim?.id===request[side].node_id&&operand.claim.content_revision===request[side].content_revision
      &&(operand.value===null||object(operand.value))&&(operand.normalized_time===null||object(operand.normalized_time)));
    validateHumanForms(operand.claim);if(operand.value)validateHumanForms(operand.value);
  }
  requireValue(Array.isArray(packet.source_refs)&&packet.source_refs.every(ref=>typeof ref==='string'&&ref.length<=2048));
  // Exact dates may contain unknown source extensions. Never display rounded
  // unsafe integers from those extensions as if they were the original value.
  const pending=[packet];while(pending.length){const value=pending.pop();
    if(typeof value==='number')requireValue(Number.isFinite(value)&&(!Number.isInteger(value)||Number.isSafeInteger(value)));
    else if(value&&typeof value==='object')pending.push(...Object.values(value));}
  return packet;
}

export function mountTemporalComparison({client,getReadings,locale=()=> 'ru'}={}){
  const el=(tag,text='')=>{const node=document.createElement(tag);node.textContent=text;return node;};
  const element=el('section'),button=el('button'),output=el('div');button.type='button';element.className='sc-temporal-comparison';element.append(button,output);
  let controller=null,key=null;
  function selected(){const readings=getReadings();return readings?.length===2?readings:null;}
  function signature(){try{return JSON.stringify(temporalComparisonRequest(...selected()));}catch{return null;}}
  function update(){const next=signature();if(next!==key){controller?.abort();controller=null;output.replaceChildren();key=next;}
    button.textContent=String(ui('Сопоставить датировки'));button.disabled=!key;
    element.hidden=!key;}
  button.onclick=async()=>{
    controller?.abort();controller=new AbortController();const request=controller,current=signature(),readings=selected();button.disabled=true;
    uiChildren(output,'replaceChildren',el('p',ui('Сопоставляю указанные источниками датировки…')));
    try{const packet=await compareExactDates(client,...readings,{signal:request.signal});if(request.signal.aborted||current!==signature()||!element.isConnected)return;
      const c=packet.comparison;
      const status=c.status==='comparable'?temporalComparisonRelationLabel(c.relation):c.status==='undetermined'?ui('Сопоставление не определено'):ui('Сопоставление датировок недоступно');
      const reasons={
        'source-date-unavailable':ui('Дата источника недоступна'),'incomplete-date':ui('Дата указана не полностью'),
        'ambiguous-date':ui('Дата допускает несколько толкований'),'different-calendars':ui('Используются разные календари'),
      },sides={left:ui('Первый материал'),right:ui('Второй материал'),pair:ui('Оба материала')};
      uiChildren(output,'replaceChildren',el('p',status),el('p',ui('Сопоставление выполнено по диапазонам дат, указанным источниками.')));
      if(c.reasons.length)uiChildren(output,'append',el('p',c.reasons.map(reason=>`${String(sides[reason.side]||ui('Материал'))}: ${String(reasons[reason.code]||ui('Причина не указана'))}`).join(' · ')));
      if(packet.source_refs.length){const details=el('details'),list=el('ul');uiChildren(details,'append',el('summary',ui('Источники · {0}',[packet.source_refs.length])));for(const ref of packet.source_refs){let url=null;try{if(/^https?:\/\//i.test(ref))url=new URL(ref);}catch{}const a=el('a',sourceLinkLabel(ref));if(url){a.href=url.href;a.target='_blank';a.rel='noopener noreferrer';}uiChildren(list,'append',el('li'),a);}uiChildren(details,'append',list);uiChildren(output,'append',details);}
      uiChildren(output,'append',rawDataDownload(packet,ui('Скачать данные сравнения'),'sophia-date-comparison.json'));
    }catch(error){if(!request.signal.aborted&&current===signature())uiChildren(output,'replaceChildren',el('p',ui('Не удалось сопоставить датировки. Повторите запрос.')));}
    finally{if(controller===request){controller=null;button.disabled=!signature();}}
  };
  update();return {element,update,cancel(){controller?.abort();controller=null;},destroy(){controller?.abort();element.remove();}};
}
