// The wall of images (H3), the images on a scale of time (H4) and the image as a subject of work (H5). The wall shows
// every image the stand's bonsai binds to its subjects (bonsai/<stand>/media.json) once, with the subjects it serves;
// on the scale of time each stands at the years of its work, with its holder and rights. Two images open side by side;
// a region drawn on an image gets an address (W3C media fragments: media:<stand>:<image>#xywh=pixel:x,y,w,h), which is
// copied, quoted, filed as a ticket or handed to an agent like any other place. Nothing here changes the bonsai.
import './act-desk.css';
import './media-wall.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {loadMedia} from './bonsai.mjs';
import {captionLinks,fittedSize,imageSources,withoutWrittenStyle,workName} from './media-view.mjs';
import {formatDating} from '../../../../bonsai/lib/time.mjs';
import {workWords} from '../../../../bonsai/lib/media.mjs';

const ADDRESS=/^media:([a-z0-9-]+):([^#\s]+)(?:#(?:xywh=pixel:(\d+),(\d+),(\d+),(\d+)|t=(\d+(?:\.\d+)?),(\d+(?:\.\d+)?)))?$/;

/** The address of an image, of a region of it (pixels of the full image) or of a segment of a recording (seconds). */
export function mediaAddress({stand,image,region=null,segment=null}){
  const base=`media:${stand}:${image}`;
  if(region){const [x,y,w,h]=region.map(value=>Math.max(0,Math.round(value)));return `${base}#xywh=pixel:${x},${y},${w},${h}`;}
  if(segment){const [from,to]=segment.map(value=>Math.max(0,Number(value)));return `${base}#t=${+from.toFixed(2)},${+to.toFixed(2)}`;}
  return base;
}
export function parseMediaAddress(address){
  const match=ADDRESS.exec(String(address??''));if(!match)return null;
  const [,stand,image,x,y,w,h,from,to]=match;
  return {stand,image,...(x!==undefined?{region:[x,y,w,h].map(Number)}:{}),...(from!==undefined?{segment:[Number(from),Number(to)]}:{})};
}

/** The years a work's dating names: one year, or the first and last of an interval (an open one at its known end); null where it names none. */
export function yearsOf(dating){
  const year=value=>{const match=/^(-?\d{1,4})/.exec(String(value??''));return match?Number(match[1]):null;};
  if(dating?.kind==='date-assertion'){const value=year(dating.value);return value===null?null:{from:value,to:value};}
  if(dating?.kind==='interval-assertion'){
    const from=year(dating.interval?.start),to=year(dating.interval?.end);
    if(from!==null&&to!==null)return {from,to};
    if(from!==null)return {from,to:from,open:'after'};
    if(to!==null)return {from:to,to,open:'before'};
  }
  return null;
}

/** Each image once, with its work, the subjects it serves and its years; ordered by its years, undated last. */
export function wallItems(media){
  const module=media?.module??{},works=module.works??{},images=module.images??{},bySubject=new Map();
  for(const use of module.uses??[])if(images[use.image]){if(!bySubject.has(use.image))bySubject.set(use.image,[]);const list=bySubject.get(use.image);if(!list.some(item=>item.subject===use.subject))list.push({subject:use.subject,role:use.role});}
  const items=[];
  for(const [id,image] of Object.entries(images)){
    const work=works[image.work];if(!work)continue;
    items.push({id,image:{id,...image},work:{id:image.work,...work},names:media.names??null,uses:bySubject.get(id)??[],years:yearsOf(work.dating)});
  }
  return items.sort((a,b)=>(a.years?.from??Infinity)-(b.years?.from??Infinity)||(a.years?.to??Infinity)-(b.years?.to??Infinity)||a.id.localeCompare(b.id));
}

/**
 * A scale of time where each year is as wide as its images need: a year with images holds them side by side in
 * `lanes` rows, a year without keeps `empty` pixels. `place(entry)` gives an image its row and its left edge at its
 * first year; the ticks name every half-century and each decade there is room to name.
 */
export function densityScale(items,{empty=3,item=100,lanes=3,label=46}={}){
  const dated=items.filter(entry=>entry.years).sort((a,b)=>a.years.from-b.years.from||a.id.localeCompare(b.id));if(!dated.length)return null;
  const first=Math.min(...dated.map(entry=>entry.years.from)),last=Math.max(...dated.map(entry=>entry.years.to));
  const byYear=new Map();for(const entry of dated){if(!byYear.has(entry.years.from))byYear.set(entry.years.from,[]);byYear.get(entry.years.from).push(entry);}
  const starts=new Map();let width=0;
  for(let year=first;year<=last;year++){starts.set(year,width);const own=byYear.get(year)?.length??0;width+=own?Math.ceil(own/lanes)*item:empty;}
  const x=year=>starts.get(Math.min(last,Math.max(first,Math.round(year))))??0;
  const places=new Map();for(const [year,list] of byYear)list.forEach((entry,index)=>places.set(entry.id,{lane:index%lanes,x:x(year)+Math.floor(index/lanes)*item}));
  const ticks=[];let named=-Infinity;
  for(let decade=Math.ceil(first/10)*10;decade<=last;decade+=10){const at=x(decade),major=decade%50===0;if(major||at-named>=label){ticks.push({year:decade,x:at,major});named=at;}}
  return {x,place:entry=>places.get(entry.id)??null,width:width+item,lanes,ticks};
}

const labels={
  wall:['Стена образов','The wall of images'],
  grid:['Стеной','As a wall'],
  scale:['На шкале времени','On a scale of time'],
  allSubjects:['Все предметы','All subjects'],
  chosen:['Выбрано','Chosen'],
  open:['Открыть','Open'],
  beside:['Рядом','Side by side'],
  undated:['без даты','undated'],
  region:['Область','Region'],
  regionHint:['Обведите область на изображении','Draw a region on the image'],
  address:['Адрес области','Address of the region'],
  copyAddress:['Копировать адрес','Copy the address'],
  quote:['Цитировать','Quote'],
  ticket:['Тикет','Ticket'],
  handOver:['Передать…','Hand over…'],
  zoomIn:['Приблизить','Zoom in'],
  zoomOut:['Отдалить','Zoom out'],
  whole:['Показать целиком','Show whole'],
  close:['Закрыть','Close'],
  missing:['Изображение не загрузилось у держателя','The image did not load from the holder'],
};
const rightsWords={mediaPublicDomain:['Общественное достояние','Public domain'],mediaCommonsPublicDomain:['Общественное достояние · Викисклад','Public domain · Wikimedia Commons'],mediaOpenAccess:['Открытый доступ','Open access']};

export function mountMediaWall({shell,actionLayer,actDesk=null,space=null,locale=()=> 'ru',profile='zarathustra',load=loadMedia}){
  if(!['zarathustra','antiquity'].includes(profile))return {open:async()=>{},destroy(){}};
  const ru=()=>resolveLocale(locale())==='ru';
  const pick=pair=>ru()?pair[0]:localeText(pair[1],locale());
  const t=key=>labels[key]?pick(labels[key]):rightsWords[key]?pick(rightsWords[key]):key;
  const doc=shell.ownerDocument;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const dialogs=[];
  function sheet(title,cls='mw-sheet'){
    const dialog=el('dialog',`ad-sheet ${cls}`),head=el('div','ad-head'),heading=el('h2','',title),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});
    dialog.addEventListener('close',()=>dialog.remove());
    head.append(heading,close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);dialog.showModal?.();
    return {dialog,body,head};
  }
  const subjectName=id=>{const item=space?.index?.get?.(id);return (item?localeValue(item.title,locale()):'')||id;};
  const when=item=>item.work.dating?formatDating(item.work.dating,locale()):'';
  const holderOf=item=>workWords(item.work,item.names,locale()).holder?.text??item.work.holder??'';
  const rightsOf=item=>captionLinks(item,{t,locale:locale()}).find(link=>link.part==='rights')??null;

  function thumb(item,{box=150,selected,onToggle,compact=false}){
    const size=fittedSize(item.image,box),sources=imageSources(item.image,{box}),name=workName(item,locale()).text;
    const node=button('','mw-thumb');node.dataset.image=item.id;node.setAttribute('aria-pressed',String(selected.has(item.id)));node.setAttribute('aria-label',name);
    const picture=el('img');if(sources.crossOrigin)picture.crossOrigin=sources.crossOrigin;picture.decoding='async';picture.loading='lazy';
    picture.width=size.width;picture.height=size.height;picture.src=sources.src;if(sources.srcset)picture.srcset=sources.srcset;picture.alt=name;
    // An image its holder does not serve leaves an empty frame of its size, not its name spilled over the wall.
    picture.addEventListener('error',()=>{const frame=el('span','mw-missing');frame.style.width=`${size.width}px`;frame.style.height=`${size.height}px`;frame.title=t('missing');picture.replaceWith(frame);},{once:true});
    const caption=el('span','mw-caption'),rights=rightsOf(item);
    // On the scale an image keeps only its years under it; its name, holder and rights are its hint.
    if(compact){node.classList.add('is-compact');caption.append(el('span','',when(item)||t('undated')));node.title=[name,when(item),holderOf(item),rights?.text].filter(Boolean).join(' · ');}
    else{caption.append(el('strong','',name),el('span','',[when(item)||t('undated'),holderOf(item)].filter(Boolean).join(' · ')));if(rights)caption.append(el('span','mw-rights',rights.text));}
    node.append(picture,caption);node.addEventListener('click',()=>onToggle(item));
    return node;
  }

  // ------------------------------------------------------------ the wall and the scale (H3, H4)
  async function open({subject=null,order='grid'}={}){
    const media=await load(profile).catch(()=>null);
    const items=wallItems(media);
    const {body,head}=sheet(t('wall'));
    const selected=new Set(),bar=el('div','ad-row mw-bar'),count=el('span','ad-muted'),openOne=button(t('open'),'text-button'),beside=button(t('beside'),'primary');
    const tabs=el('div','ad-row'),filter=el('select','ad-input mw-filter'),view=el('div','mw-view');
    const subjects=new Map();for(const item of items)for(const use of item.uses)subjects.set(use.subject,(subjects.get(use.subject)??0)+1);
    const all=el('option','',`${t('allSubjects')} · ${items.length}`);all.value='';filter.append(all);
    for(const [id,n] of [...subjects].sort((a,b)=>b[1]-a[1]||subjectName(a[0]).localeCompare(subjectName(b[0]),locale()))){const option=el('option','',`${subjectName(id)} · ${n}`);option.value=id;filter.append(option);}
    filter.value=subject&&subjects.has(subject)?subject:'';
    for(const [id,key] of [['grid','grid'],['scale','scale']]){const tab=button(t(key),'text-button ad-tab');tab.dataset.order=id;tab.addEventListener('click',()=>{order=id;paint();});tabs.append(tab);}
    const toggle=item=>{if(selected.has(item.id))selected.delete(item.id);else{if(selected.size>=2)selected.delete([...selected][0]);selected.add(item.id);}paint();};
    function shown(){return filter.value?items.filter(item=>item.uses.some(use=>use.subject===filter.value)):items;}
    function paint(){
      for(const tab of tabs.children)tab.setAttribute('aria-pressed',String(tab.dataset.order===order));
      count.textContent=`${t('chosen')}: ${selected.size}`;openOne.disabled=selected.size<1;beside.disabled=selected.size<2;
      view.replaceChildren();
      const list=shown();
      if(order==='grid'){const grid=el('div','mw-grid');for(const item of list)grid.append(thumb(item,{selected,onToggle:toggle}));view.append(grid);return;}
      // A crowded decade is wider than an empty one, so that its images stand clear of each other; the ticks keep the years honest.
      const measure=densityScale(list);if(!measure){view.append(el('p','ad-muted',t('undated')));return;}
      const {x}=measure,scale=el('div','mw-scale');scale.style.width=`${measure.width}px`;
      const ruler=el('div','mw-ruler');
      for(const mark of measure.ticks){const tick=el('span',mark.major?'mw-tick is-major':'mw-tick',String(mark.year));tick.style.left=`${mark.x}px`;ruler.append(tick);}
      const rows=Array.from({length:measure.lanes},()=>el('div','mw-lane'));
      for(const item of list){const at=measure.place(item);if(!at)continue;const node=thumb(item,{box:72,selected,onToggle:toggle,compact:true});node.style.left=`${at.x}px`;
        // An interval's later years run as a line from its image.
        if(item.years.to>item.years.from){const extent=el('span','mw-extent');extent.style.width=`${Math.max(2,x(item.years.to)-x(item.years.from))}px`;node.prepend(extent);}
        rows[at.lane].append(node);}
      scale.append(ruler,...rows.filter(row=>row.childElementCount));
      const undated=list.filter(item=>!item.years);if(undated.length){const row=el('div','mw-grid');for(const item of undated)row.append(thumb(item,{box:110,selected,onToggle:toggle}));view.append(scale,el('h3','ad-minor',t('undated')),row);}else view.append(scale);
    }
    openOne.addEventListener('click',()=>void openViewer(items.filter(item=>selected.has(item.id)).slice(0,1)));
    beside.addEventListener('click',()=>void openViewer(items.filter(item=>selected.has(item.id))));
    filter.addEventListener('change',paint);
    bar.append(tabs,filter,count,openOne,beside);head.after(bar);body.append(view);paint();
    return {items};
  }

  // ------------------------------------------------------------ one or two images, and regions (H3, H5)
  async function openViewer(list){
    if(!list.length)return;
    const {body,dialog}=sheet(list.length>1?t('beside'):workName(list[0],locale()).text,'mw-viewer');
    const panes=el('div','mw-panes');panes.dataset.count=String(list.length);body.append(panes);
    let OpenSeadragon=null;try{({default:OpenSeadragon}=await import('openseadragon'));}catch{}
    const viewers=[];
    for(const item of list){
      const pane=el('section','mw-pane'),top=el('div','mw-pane-top'),stage=el('div','mw-stage'),tools=el('div','ad-row mw-tools'),out=el('div','mw-region');out.hidden=true;
      const name=workName(item,locale()),rights=rightsOf(item);
      const title=el('p','mw-title');title.append(el('strong','',name.text),el('span','',` · ${[when(item),holderOf(item)].filter(Boolean).join(' · ')}`));
      if(rights){const link=el('a','mw-rights',`${rights.text} ↗`);link.href=rights.url;link.target='_blank';link.rel='noopener noreferrer';title.append(' ',link);}
      const region=button(t('region'),'text-button mw-region-toggle');region.setAttribute('aria-pressed','false');
      const zoomIn=button('+','icon'),zoomOut=button('−','icon'),whole=button('⤢','icon');
      for(const [node,key] of [[zoomIn,'zoomIn'],[zoomOut,'zoomOut'],[whole,'whole']])node.setAttribute('aria-label',t(key));
      tools.append(region,zoomIn,zoomOut,whole);top.append(title,tools);pane.append(top,stage,out);panes.append(pane);
      const state={item,stage,out,osd:null,drawing:false,overlay:null};viewers.push(state);
      if(!OpenSeadragon){stage.append(el('p','ad-muted',t('missing')));continue;}
      const sources=imageSources(item.image);
      state.osd=withoutWrittenStyle(()=>OpenSeadragon({element:stage,tileSources:sources.full,crossOriginPolicy:sources.crossOrigin?'Anonymous':false,showNavigationControl:false,visibilityRatio:1,minZoomImageRatio:.9,maxZoomPixelRatio:2,animationTime:.5,gestureSettingsMouse:{clickToZoom:false,dblClickToZoom:true}}));
      state.osd.addHandler('open-failed',()=>stage.replaceChildren(el('p','ad-muted',t('missing'))));
      zoomIn.addEventListener('click',()=>{state.osd.viewport.zoomBy(1.6);state.osd.viewport.applyConstraints();});
      zoomOut.addEventListener('click',()=>{state.osd.viewport.zoomBy(1/1.6);state.osd.viewport.applyConstraints();});
      whole.addEventListener('click',()=>state.osd.viewport.goHome());
      region.addEventListener('click',()=>{state.drawing=!state.drawing;region.setAttribute('aria-pressed',String(state.drawing));stage.classList.toggle('is-drawing',state.drawing);state.osd.setMouseNavEnabled(!state.drawing);if(state.drawing)actionLayer?.say?.(t('regionHint'));});
      drawRegions(state,OpenSeadragon);
    }
    dialog.addEventListener('close',()=>{for(const state of viewers)state.osd?.destroy();},{once:true});
    return viewers;
  }
  function drawRegions(state,OpenSeadragon){
    const {stage}=state;let start=null,box=null;
    const point=event=>{const rect=stage.getBoundingClientRect();return new OpenSeadragon.Point(event.clientX-rect.left,event.clientY-rect.top);};
    stage.addEventListener('pointerdown',event=>{if(!state.drawing)return;event.preventDefault();stage.setPointerCapture?.(event.pointerId);start=point(event);box?.remove();box=el('div','mw-drawn');stage.append(box);});
    stage.addEventListener('pointermove',event=>{if(!state.drawing||!start)return;const now=point(event);Object.assign(box.style,{left:`${Math.min(start.x,now.x)}px`,top:`${Math.min(start.y,now.y)}px`,width:`${Math.abs(now.x-start.x)}px`,height:`${Math.abs(now.y-start.y)}px`});});
    stage.addEventListener('pointerup',event=>{
      if(!state.drawing||!start)return;const end=point(event),from=start;start=null;box?.remove();box=null;
      if(Math.abs(end.x-from.x)<6||Math.abs(end.y-from.y)<6)return;
      const a=state.osd.viewport.viewerElementToImageCoordinates(from),b=state.osd.viewport.viewerElementToImageCoordinates(end);
      const {width,height}=state.item.image,clamp=(value,limit)=>Math.min(limit,Math.max(0,value));
      const x=clamp(Math.min(a.x,b.x),width),y=clamp(Math.min(a.y,b.y),height),w=clamp(Math.max(a.x,b.x),width)-x,h=clamp(Math.max(a.y,b.y),height)-y;
      if(w<2||h<2)return;
      showRegion(state,[x,y,w,h],OpenSeadragon);
    });
  }
  function regionTarget(item,region){
    const address=mediaAddress({stand:profile,image:item.id,region}),name=workName(item,locale()).text,rights=rightsOf(item);
    const [x,y,w,h]=region.map(Math.round);
    return {type:'region',id:address,label:`${name} · ${x},${y} ${w}×${h}`,
      media:{image:item.id,work:item.work.id,title:name,date:when(item),holder:holderOf(item),record:item.work.record,rights:rights?{text:rights.text,url:rights.url}:null,region:[x,y,w,h]}};
  }
  function showRegion(state,region,OpenSeadragon){
    const target=regionTarget(state.item,region);
    if(state.overlay)state.osd.removeOverlay(state.overlay);
    state.overlay=el('div','mw-overlay');
    state.osd.addOverlay({element:state.overlay,location:state.osd.viewport.imageToViewportRectangle(new OpenSeadragon.Rect(...target.media.region))});
    const {out}=state;out.hidden=false;out.replaceChildren(el('span','ad-label',t('address')),el('code','mw-address',target.id));
    const row=el('div','ad-row');
    const copy=button(t('copyAddress'),'text-button');copy.addEventListener('click',()=>void actionLayer?.copy?.(target.id));
    const quote=button(t('quote'),'text-button');quote.addEventListener('click',()=>void actionLayer?.copy?.(quoteOf(target)));
    row.append(copy,quote);
    if(actDesk){
      const ticket=button(t('ticket'),'text-button');ticket.addEventListener('click',()=>void actDesk.openTicket(target,{category:'media'}));
      const hand=button(t('handOver'),'text-button');hand.addEventListener('click',()=>void actDesk.openTransfer(target));
      row.append(ticket,hand);
    }
    out.append(row);state.region=target;return target;
  }
  /** A quotation of a region: the work, its date and holder, its rights, the region's address and the holder's record. */
  function quoteOf(target){
    const m=target.media;
    return [`${m.title}${m.date?` (${m.date})`:''}${m.holder?`, ${m.holder}`:''}`,`${t('region')}: ${m.region.join(', ')} px`,target.id,m.record,m.rights?`${m.rights.text}: ${m.rights.url}`:''].filter(Boolean).join('\n');
  }

  const remove=actionLayer?.addActions?.([
    {id:'media-wall',text:()=>t('wall'),for:['any'],run:target=>open({subject:target?.item?.record?.id??null})},
  ]);
  return {open,openViewer,regionTarget,quoteOf,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
