// Browser regression for the real reading action. Run after pinning two
// artificial items, reloading, and opening an inspector in reader.html.
export function checkReadingResume(root){
  const require=(condition,message)=>{if(!condition)throw new Error(message);};
  const inspector=root.querySelector('.sc-inspector'),resume=root.querySelector('.sc-reader-resume');
  const reader=root.querySelector('.sc-reader');
  require(!inspector.hidden&&reader.hidden,'Open the inspector with reading closed before checking.');
  const ids=[...reader.querySelectorAll('.sc-reader-article')].map(article=>article.dataset.readingId).sort();
  require(ids.length===2&&new Set(ids).size===2,'Restore two distinct pinned items before checking.');
  const rect=resume.getBoundingClientRect();
  require(!resume.hidden&&rect.width>0&&rect.height>0,'The reading return must be visible.');
  for(const [x,y]of [[.5,.5],[.05,.5],[.95,.5],[.5,.2],[.5,.8]]){
    const hit=document.elementFromPoint(rect.left+rect.width*x,rect.top+rect.height*y);
    require(hit===resume||resume.contains(hit),'The reading return is covered or clipped.');
  }
  // Hit testing above is essential: HTMLElement.click alone bypasses overlap.
  resume.click();
  require(!reader.hidden&&inspector.hidden,'One activation must open reading without closing the inspector first.');
  const restored=[...reader.querySelectorAll('.sc-reader-article')].map(article=>article.dataset.readingId).sort();
  require(JSON.stringify(restored)===JSON.stringify(ids),'Opening reading must retain both exact pins.');
  require(reader.contains(document.activeElement),'Focus must enter the restored reading.');
  return {status:'PASS',ids,hitPoints:5,activations:1};
}
