// The images on a card: works bound to the card's subject in the stand's bonsai (bonsai/<stand>/media.json).
//
// A card shows each image whole, in its own proportions, as a small copy from
// the provider's IIIF image service; under it, the caption says what the image
// is for on this card and when the work was made, then the provider's own
// title, the inscription on the object, the series, who made it, its size, and
// the holder's record and rights statement as links. Choosing the image opens
// it at the full detail its service serves, in a viewer that lies over the
// stand; closing the viewer leaves the space and the card as they were.

import {formatDating} from '../../../../bonsai/lib/time.mjs';

const CARD_BOX=480;

/** A IIIF Image API 3 request for the whole image fitted into a square of `box` pixels. */
export const fittedImage=(service,box)=>`${service}/full/!${box},${box}/0/default.jpg`;

/** The size a whole image takes when fitted into a square of `box` pixels, never larger than it is. */
export function fittedSize({width,height},box=CARD_BOX){
  const scale=Math.min(1,box/width,box/height);
  return {width:Math.max(1,Math.round(width*scale)),height:Math.max(1,Math.round(height*scale))};
}

const titleOf=(work,role)=>(work.titles??[]).filter(title=>title.role===role);

/**
 * The caption of a medium as lines of text, in the order a card shows them; each line names its
 * `part` and, for a provider's words, their `lang`. `t(key)` gives the stand's words.
 */
export function captionLines({role,work},{t,locale}){
  const lines=[];
  const when=work.dating?formatDating(work.dating,locale):'';
  lines.push({part:'what',text:[t(`media_${role}`),when].filter(Boolean).join(' · ')});
  for(const title of titleOf(work,'title'))lines.push({part:'title',text:title.text,lang:title.language});
  // The words written on the object stand as they are written; the provider's language tag describes its record, not them.
  for(const title of titleOf(work,'inscribed'))lines.push({part:'inscribed',label:t('mediaInscribed'),text:title.text});
  const series=titleOf(work,'series');
  if(series.length)lines.push({part:'series',label:t('mediaSeries'),text:series.map(title=>title.text).join(' · '),lang:series[0].language});
  for(const line of work.made??[])lines.push({part:'made',text:line.text,lang:line.language});
  if(work.size_mm){const number=new Intl.NumberFormat(locale);lines.push({part:'size',text:t('mediaSize').replace('{0}',number.format(work.size_mm[0])).replace('{1}',number.format(work.size_mm[1]))});}
  return lines;
}

/** Where the caption's sources lead: the holder's record of the work and the rights statement of the image. */
export function captionLinks({work,image}){
  const links=[{part:'record',text:`${work.holder} · ${work.inventory}`,url:work.record}];
  const rights=image.rights;
  links.push({part:'rights',text:rights.label??rights.statement??rights.value,url:rights.statement??rights.stated_in});
  return links;
}

// The viewer writes one style element of its own for touch screens; the page admits no style written by a script,
// so that element stays unattached while the viewer is made, and the stand's stylesheet carries its rule.
function withoutWrittenStyle(make){
  const head=document.head,append=head.appendChild;
  head.appendChild=function(node){return node instanceof HTMLStyleElement&&node.hasAttribute('data-openseadragon-mobile-css')?node:append.call(this,node);};
  try{return make();}finally{delete head.appendChild;}
}

/**
 * The card's media view. `t(key)` gives the stand's words, `locale()` the reader's language,
 * `hint(element,text)` a hint on an element.
 */
export function createMediaView({t,locale,hint=()=>{}}){
  let viewer=null;
  const el=(tag,className='',text='')=>{const node=document.createElement(tag);if(className)node.className=className;if(text)node.textContent=text;return node;};

  function figure(item){
    const {image,work,role}=item,size=fittedSize(image);
    const box=el('figure','ps-media');box.dataset.role=role;
    const open=el('button','ps-media-open');open.type='button';open.setAttribute('aria-label',t('mediaOpen'));hint(open,t('mediaOpen'));
    const picture=el('img');
    // The page is cross-origin isolated: an image from the provider is requested in CORS mode, which its service answers.
    picture.crossOrigin='anonymous';picture.decoding='async';picture.loading='lazy';
    picture.width=size.width;picture.height=size.height;
    picture.src=fittedImage(image.service,CARD_BOX);
    picture.srcset=`${fittedImage(image.service,CARD_BOX)} 1x, ${fittedImage(image.service,CARD_BOX*2)} 2x`;
    picture.alt=titleOf(work,'title')[0]?.text??'';
    picture.addEventListener('error',()=>{open.replaceWith(el('p','ps-media-missing',t('mediaMissing')));},{once:true});
    open.append(picture);open.addEventListener('click',()=>void openViewer(item,open));
    const caption=el('figcaption');
    for(const line of captionLines(item,{t,locale:locale()})){
      const row=el('p',`ps-media-${line.part}`);
      if(line.label)row.append(el('span','ps-media-label',`${line.label}: `));
      const words=el('span','',line.text);if(line.lang)words.lang=line.lang;row.append(words);
      caption.append(row);
    }
    const sources=el('p','ps-media-sources');
    for(const link of captionLinks(item)){const anchor=el('a','',`${link.text} ↗`);anchor.href=link.url;anchor.target='_blank';anchor.rel='noopener noreferrer';anchor.dataset.part=link.part;sources.append(anchor);}
    caption.append(sources);
    box.append(open,caption);
    return box;
  }

  async function openViewer(item,opener){
    closeViewer();
    const {image,work}=item,host=document.fullscreenElement??document.body;
    // A modal dialog holds the page: the stand's keys, its hints and its controls let it be while it is open.
    const layer=el('dialog','ps-media-viewer');
    const name=titleOf(work,'title')[0]?.text??work.inventory;layer.setAttribute('aria-label',name);
    const stage=el('div','ps-media-stage'),bar=el('div','ps-media-bar'),title=el('p','ps-media-viewer-title');
    const words=el('span','',name);if(titleOf(work,'title')[0]?.language)words.lang=titleOf(work,'title')[0].language;
    const when=work.dating?formatDating(work.dating,locale()):'';
    title.append(words);if(when)title.append(el('span','ps-media-viewer-date',` · ${when}`));title.append(el('span','ps-media-viewer-holder',` · ${work.holder}`));
    const control=(text,label,action)=>{const node=el('button','ps-media-control',text);node.type='button';node.setAttribute('aria-label',label);hint(node,label);node.addEventListener('click',action);return node;};
    const state={layer,opener,osd:null};
    const zoomIn=control('+',t('mediaZoomIn'),()=>{state.osd?.viewport.zoomBy(1.6);state.osd?.viewport.applyConstraints();});
    const zoomOut=control('−',t('mediaZoomOut'),()=>{state.osd?.viewport.zoomBy(1/1.6);state.osd?.viewport.applyConstraints();});
    const whole=control('⤢',t('mediaWhole'),()=>state.osd?.viewport.goHome());
    const close=control('×',t('close'),closeViewer);
    bar.append(title,zoomIn,zoomOut,whole,close);layer.append(stage,bar);
    layer.addEventListener('keydown',event=>event.stopPropagation());
    layer.addEventListener('cancel',event=>{event.preventDefault();closeViewer();});
    host.append(layer);layer.showModal();viewer=state;close.focus();
    try{
      const {default:OpenSeadragon}=await import('openseadragon');
      if(viewer!==state)return;
      state.osd=withoutWrittenStyle(()=>OpenSeadragon({
        element:stage,tileSources:`${image.service}/info.json`,crossOriginPolicy:'Anonymous',
        showNavigationControl:false,visibilityRatio:1,minZoomImageRatio:0.9,maxZoomPixelRatio:2,
        animationTime:0.6,gestureSettingsMouse:{clickToZoom:false,dblClickToZoom:true},
      }));
      state.osd.addHandler('open-failed',()=>{stage.replaceChildren(el('p','ps-media-missing',t('mediaMissing')));});
    }catch{stage.replaceChildren(el('p','ps-media-missing',t('mediaMissing')));}
  }

  function closeViewer(){
    if(!viewer)return;
    const {layer,opener,osd}=viewer;viewer=null;
    osd?.destroy();if(layer.open)layer.close();layer.remove();
    if(opener?.isConnected)opener.focus({preventScroll:true});
  }

  return {
    /** Put the media `items` (from `readMedia(...).forSubject`) into `parent`. */
    render(parent,items){
      if(!items?.length)return;
      const section=el('div','ps-media-list');
      for(const item of items)section.append(figure(item));
      parent.append(section);
    },
    close:closeViewer,
  };
}
