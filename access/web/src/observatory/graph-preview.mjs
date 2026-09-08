import {t} from './ui-i18n.mjs';
import {localized} from './knowledge-client.mjs';

const compact=(value,limit)=>{const text=String(value||'').replace(/\s+/g,' ').trim();return text.length>limit?text.slice(0,limit-1).trimEnd()+'…':text;};
// Read every type and relationship through its supplied display contract.
// Inverse labels carry roles such as author, translator, editor and designer;
// no browser-owned list decides which roles or node kinds can appear.
export function nodePreview(packet,node){
  const kind=localized(node?.display?.kind_label,node?.kind_id||'');
  const summary=['authored','source-derived'].includes(node?.display?.summary_state)?localized(node.display.summary):'';
  const connections=(packet?.relations||[]).filter(r=>r.from_id===node?.id||r.to_id===node?.id).map(relation=>{
    const outgoing=relation.from_id===node.id,other=packet.nodes.find(n=>n.id===(outgoing?relation.to_id:relation.from_id));
    const inverse=localized(relation.display?.inverse_label),label=localized(relation.display?.label),name=localized(other?.display?.title);
    if(!name)return '';
    return outgoing?label+' → '+name:inverse?inverse+': '+name:name+' → '+label;
  }).filter(Boolean);
  return {kind:compact(kind,72),title:compact(localized(node?.display?.title),100),body:compact(summary||[...new Set(connections)].slice(0,2).join(' · '),140)};
}
export function relationPreview(packet,relation){
  const title=localized(relation?.display?.label),left=packet.nodes.find(n=>n.id===relation?.from_id),right=packet.nodes.find(n=>n.id===relation?.to_id);
  return {kind:t("Связь"),title:compact(title,90),body:compact([localized(left?.display?.title),localized(right?.display?.title)].filter(Boolean).join(' → '),150)};
}
export function describePreview(element,preview){
  element.dataset.tooltip=[preview.kind,preview.title,preview.body].filter(Boolean).join(' — ');
  element.dataset.tooltipKind=preview.kind;element.dataset.tooltipTitle=preview.title;element.dataset.tooltipBody=preview.body;
}
