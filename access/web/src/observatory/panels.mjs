// A small presentation seam for replaceable auxiliary tools. Scene navigation
// continues to own its inspector, search and lens overlays.
export function createPanelHost(root,scene){
  const panels=new Map();
  const sizes=new ResizeObserver(()=>scene.invalidate());
  function close(id){const entry=panels.get(id);if(!entry)return;entry.element.hidden=true;entry.onHide();scene.invalidate();}
  const observer=new MutationObserver(()=>{
    if(['.sc-inspector','.sc-search','.sc-lenses'].some(selector=>!root.querySelector(selector).hidden)){
      for(const [id,entry]of panels)if(!entry.element.hidden)close(id);
    }
  });
  for(const selector of ['.sc-inspector','.sc-search','.sc-lenses'])observer.observe(root.querySelector(selector),{attributes:true,attributeFilter:['hidden']});
  return {
    register(id,element,onHide){if(panels.has(id))throw new Error('Duplicate panel: '+id);panels.set(id,{element,onHide});sizes.observe(element);},
    open(id){
      if(!panels.has(id))throw new Error('Unknown panel: '+id);
      for(const other of panels.keys())close(other);
      scene.closeInspector(false);
      root.querySelector('.sc-search-close').click();root.querySelector('.sc-lenses-close').click();
      panels.get(id).element.hidden=false;scene.invalidate();
    },
    close,
  };
}
