// Interactive checks of the real reader against artificial fixture material.
// Recovery changes only the fixture connection; removal consumes one fixture
// pin. These helpers are never production entries or source acceptance checks.
const require=(condition,message)=>{if(!condition)throw new Error(message);};
function reading(root){
  require(root.dataset.fixture==='true','Run this check in the artificial reader fixture.');
  const panel=root.querySelector('.sc-reader'),tabs=panel.querySelector('.sc-reader-tabs');
  require(!panel.hidden,'Open reading before checking continuity.');
  const views=[...panel.querySelectorAll('.sc-reader-article')].map(article=>({article,
    id:article.dataset.readingId,body:article.querySelector('.sc-reader-body'),
    tab:[...tabs.children].find(tab=>tab.getAttribute('aria-controls')===article.id)}));
  require(views.length===2&&new Set(views.map(view=>view.id)).size===2,'Pin two distinct fixture items.');
  require(views.every(view=>view.article.dataset.snapshot&&view.body.getAttribute('aria-busy')==='false'),'Wait for both fixture items to load.');
  return {panel,tabs,views};
}
function selected(view){
  require(view.tab.getAttribute('aria-selected')==='true'&&view.tab.tabIndex===0,'The active item must own the selected keyboard tab.');
  require(!view.article.hidden,'The active reading must be visible.');
}
const press=(tab,key)=>tab.dispatchEvent(new KeyboardEvent('keydown',{key,bubbles:true,cancelable:true}));
function setPosition(view,top){
  view.body.scrollTop=Math.min(top,Math.max(0,view.body.scrollHeight-view.body.clientHeight));
  view.body.dispatchEvent(new Event('scroll'));return view.body.scrollTop;
}
const samePosition=(view,top)=>require(Math.abs(view.body.scrollTop-top)<1,'Switching items must preserve their independent reading positions.');

export function checkHeaderNavigation(root){
  require(root.dataset.fixture==='true','Run this check in the artificial reader fixture.');
  const controls=[...root.querySelectorAll('.sc-header-actions>.sc-control')].filter(el=>el.getClientRects().length);
  for(const control of controls){
    control.focus({preventScroll:true});
    const box=control.getBoundingClientRect();
    for(const x of [.1,.5,.9])require(control.contains(document.elementFromPoint(box.left+box.width*x,box.top+box.height/2)),'A focused header tool is clipped or covered.');
  }
  return {status:'PASS',controls:controls.length,points:controls.length*3};
}

export function checkReadingContinuity(root){
  const {panel,tabs,views}=reading(root),[first,last]=views;let positions=[];
  if(!tabs.hidden){
    first.tab.focus();press(first.tab,'Home');selected(first);
    const firstPosition=setPosition(first,140);
    press(first.tab,'End');selected(last);
    require(document.activeElement===last.tab,'Keyboard switching must focus the selected visible tab.');
    const lastPosition=setPosition(last,230);
    press(last.tab,'Home');selected(first);samePosition(first,firstPosition);
    press(first.tab,'ArrowLeft');selected(last);samePosition(last,lastPosition);
    press(last.tab,'ArrowRight');selected(first);samePosition(first,firstPosition);
    press(first.tab,'End');selected(last);samePosition(last,lastPosition);
    for(const view of views)require(view.article.getAttribute('role')==='tabpanel'&&view.article.getAttribute('aria-labelledby')===view.tab.id,'A tabbed article must be named by its controlling tab.');
    positions=[firstPosition,lastPosition];
  }else{
    require(views.every(view=>!view.article.hidden),'Both comparison columns must be visible.');
    for(const view of views)require(view.article.getAttribute('aria-labelledby')===view.article.querySelector('h4').id,'A comparison article must be named by its visible heading.');
    positions=views.map((view,index)=>setPosition(view,index?230:140));
  }
  last.body.focus({preventScroll:true});selected(last);samePosition(last,positions[1]);
  require(document.activeElement===last.body&&!last.body.closest('[hidden]'),'Focus must stay in the active visible reading.');
  for(const selector of ['.sc-reader-open','.sc-reader-resume']){
    const control=root.querySelector(selector);
    require(control.getAttribute('aria-controls')===panel.id&&control.getAttribute('aria-expanded')==='true','Both reading openers must expose the current panel state.');
  }
  return {status:'PASS',mode:tabs.hidden?'columns':'tabs',ids:views.map(view=>view.id),activeId:last.id,positions};
}

async function settled(check){
  const deadline=performance.now()+3000;
  while(!check()&&performance.now()<deadline)await new Promise(resolve=>setTimeout(resolve,20));
  require(check(),'The fixture reading request did not settle.');
}
export async function checkReadingRecovery(root){
  const {views}=reading(root),view=views.find(view=>view.tab.getAttribute('aria-selected')==='true');
  const connection=document.querySelector('#fixture-connection'),refresh=view.article.querySelector('.sc-reader-item-controls button');
  require(connection?.value==='normal','Use the normal fixture connection before checking recovery.');
  require(view.body.scrollHeight>view.body.clientHeight,'Use a long fixture item for the position recovery check.');
  const position=setPosition(view,170),revision=view.article.dataset.snapshot;
  const sources=view.body.querySelector('details[data-reading-key="sources"]');if(sources)sources.open=true;
  try{
    connection.value='restricted';refresh.click();
    await settled(()=>!view.article.dataset.snapshot&&!refresh.disabled);
    require(view.body.getAttribute('aria-busy')==='false'&&view.body.querySelector('.sc-reader-gap'),'Unavailable material must expose a settled gap.');
    connection.value='normal';refresh.click();
    await settled(()=>view.article.dataset.snapshot===revision&&!refresh.disabled);
    samePosition(view,position);
    if(sources)require(view.body.querySelector('details[data-reading-key="sources"]')?.open,'Recovery must restore the exact reading disclosures.');
    return {status:'PASS',id:view.id,revision,position,restoredPosition:view.body.scrollTop};
  }finally{connection.value='normal';}
}

export function checkReadingRemoval(root){
  const {panel,tabs,views}=reading(root),active=views.find(view=>view.tab.getAttribute('aria-selected')==='true');
  const remaining=views.find(view=>view!==active),remove=active.article.querySelector('.sc-reader-remove');
  remove.focus();remove.click();
  require(panel.querySelectorAll('.sc-reader-article').length===1&&remaining.article.isConnected,'Removing the active pin must retain the other exact item.');
  require(tabs.hidden&&document.activeElement===remaining.body&&!remaining.body.closest('[hidden]'),'After removing a pin, focus must enter the remaining reading rather than a hidden tab.');
  return {status:'PASS',removedId:active.id,remainingId:remaining.id,focused:'reading-body'};
}
