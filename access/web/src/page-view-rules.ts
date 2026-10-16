import {webMcpViewSelector} from './webmcp';

// Raw identities, native iterable custody and error interpolation remain here.
export function requireKnownView(viewId:string,knownViewIds:Iterable<string>):void {
  const Rule=webMcpViewSelector(),session=new Rule('known',Boolean(viewId));
  try {
    if(session.need()==='known-membership')session.membership(new Set(knownViewIds).has(viewId));
    const need=session.need();
    if(need==='empty-error'||need==='unknown-error')throw new Error(`unknown Tree of Sophia view: ${need==='empty-error'?Rule.empty_label():viewId}`);
  } finally {session.free();}
}
export function reloadableFocus(selected:{id:string}|null,selectedGraphId:string|null,reloadableIds:Iterable<string>):string {
  // This unconditional constructor precedes the selected getter in the original.
  const allowed=new Set(reloadableIds);
  const session=new (webMcpViewSelector())('reload',false);
  let selectedId:unknown;
  try {
    while(true)switch(session.need()) {
      case 'selected':selectedId=selected?.id;session.value(Boolean(selectedId));break;
      case 'selected-membership':session.membership(allowed.has(selectedId as string));break;
      case 'graph':session.value(Boolean(selectedGraphId));break;
      case 'graph-membership':session.membership(allowed.has(selectedGraphId as string));break;
      case 'selected-result':return selectedId as string;
      case 'graph-result':return selectedGraphId as string;
      case 'empty':return '';
      default:throw new Error('Unknown maintained page view selector phase');
    }
  } finally {session.free();}
}
