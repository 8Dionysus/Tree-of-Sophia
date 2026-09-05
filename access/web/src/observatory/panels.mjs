// A small presentation seam for replaceable auxiliary tools. Scene navigation
// continues to own its inspector, search and lens overlays.
export function createPanelHost(root,scene){
  const panels=new Map();
  const sizes=new ResizeObserver(()=>scene.invalidate());
  function preferredDock(){
    const bounds=root.getBoundingClientRect(),middle=bounds.left+bounds.width/2;
    const inspector=root.querySelector('.sc-inspector');
    // The inspector has already made room for the selected star, including
    // an unfinished focus flight. Keep that free side when handing off tools.
    if(!inspector.hidden&&bounds.width>650){const box=inspector.getBoundingClientRect();return box.left+box.width/2<middle?'left':'right';}
    for(const {element}of panels.values())if(!element.hidden)return element.dataset.dock||'right';
    const anchor=[...root.querySelectorAll('.sc-node')].find(node=>node.dataset.id===root.dataset.selected&&!node.hidden);
    if(anchor){const box=anchor.getBoundingClientRect();if(box.width)return box.left+box.width/2>middle?'left':'right';}
    return 'right';
  }
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
      const dock=preferredDock();
      for(const other of panels.keys())close(other);
      scene.closeInspector(false);
      root.querySelector('.sc-search-close').click();root.querySelector('.sc-lenses-close').click();
      const element=panels.get(id).element;
      const context=root.querySelector('.sc-context').getBoundingClientRect();
      element.style.setProperty('--sc-tool-top',`${context.bottom-root.getBoundingClientRect().top+18}px`);
      element.dataset.dock=dock;element.hidden=false;scene.invalidate();
    },
    close,
  };
}
