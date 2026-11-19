// The images on a card: works bound to the card's subject in the stand's bonsai (bonsai/<stand>/media.json).
//
// A card shows each image whole, in its own proportions, as a small copy: from
// the provider's IIIF image service, or the stand's own copy of the provider's
// smaller file, which the site serves from its layer store (layers/sha256/<hex>,
// the same-origin copy of R2). Under it, the caption says what the image
// is for on this card and when the work was made, then the work's title, the
// inscription on the object, the series, who made it, its size, and the
// holder's record and rights statement as links. Title, series, makers and
// holder are in the reader's language alone (`workWords`): the provider's
// words where they are in it, else the Tree's names of the work; the words
// written on the object stand as written. Choosing the image opens
// it at the full detail its service serves, in a viewer that lies over the
// stand; closing the viewer leaves the space and the card as they were. A
// link's card shows its images as a strip of small copies, in groups: the
// link's own, or those of each of its two ends under the end's name; each
// opens in the same viewer, whose bar names the work.

import {workWords} from '../../../../bonsai/lib/media.mjs';
import {formatDating} from '../../../../bonsai/lib/time.mjs';

const CARD_BOX=480;
const STRIP_BOX=160;
const BASE=import.meta.env?.BASE_URL??'/';

/** Where the site serves a file the stand keeps in its layer store. */
export const layerAddress=(sha256,base=BASE)=>`${base.endsWith('/')?base:`${base}/`}layers/sha256/${sha256}`;

/** The card's copy of an image and, for the full view, the source the viewer reads. */
export function imageSources(image,{base=BASE,box=CARD_BOX}={}){
  if(image.api==='file')return {src:layerAddress(image.preview.sha256,base),srcset:'',full:{type:'image',url:layerAddress(image.file.sha256,base)},crossOrigin:null};
  // Version 2 and 3 of the IIIF Image API ask for a whole image fitted into a box in the same words.
  return {src:fittedImage(image.service,box),srcset:`${fittedImage(image.service,box)} 1x, ${fittedImage(image.service,box*2)} 2x`,full:`${image.service}/info.json`,crossOrigin:'anonymous'};
}

/**
 * The groups a link's card shows: the link's own images, else the images of each end under its name,
 * an image both ends share only once. `own` and each end's `items` come from `readMedia(...).forSubject`.
 */
export function linkGroups({own=[],ends=[]}){
  if(own.length)return [{label:null,items:own}];
  const seen=new Set();
  return ends.map(end=>({label:end.label,items:(end.items??[]).filter(item=>!seen.has(item.image.id)&&seen.add(item.image.id))})).filter(group=>group.items.length);
}

/** A IIIF Image API 3 request for the whole image fitted into a square of `box` pixels. */
export const fittedImage=(service,box)=>`${service}/full/!${box},${box}/0/default.jpg`;

/** The size a whole image takes when fitted into a square of `box` pixels, never larger than it is. */
export function fittedSize({width,height},box=CARD_BOX){
  const scale=Math.min(1,box/width,box/height);
  return {width:Math.max(1,Math.round(width*scale)),height:Math.max(1,Math.round(height*scale))};
}

const PUBLIC_DOMAIN_MARK='https://creativecommons.org/publicdomain/mark/1.0/';

/** The name a reader of `locale` sees a work by, with its language: its first title, else the holder's number. */
export function workName({work,names=null},locale){
  const titles=workWords(work,names,locale).titles;
  return titles?{text:titles.lines[0],lang:titles.language}:{text:work.inventory,lang:null};
}

/**
 * The caption of a medium as lines of text, in the order a card shows them; each line names its
 * `part` and, for words in a language, their `lang`. `t(key)` gives the stand's words.
 */
export function captionLines({role,work,names=null},{t,locale}){
  const lines=[],words=workWords(work,names,locale);
  const when=work.dating?formatDating(work.dating,locale):'';
  lines.push({part:'what',text:[t(`media_${role}`),when].filter(Boolean).join(' · ')});
  for(const text of words.titles?.lines??[])lines.push({part:'title',text,lang:words.titles.language});
  // The words written on the object stand as they are written; the provider's language tag describes its record, not them.
  for(const title of (work.titles??[]).filter(title=>title.role==='inscribed'))lines.push({part:'inscribed',label:t('mediaInscribed'),text:title.text});
  if(words.series)lines.push({part:'series',label:t('mediaSeries'),text:words.series.lines.join(' · '),lang:words.series.language});
  for(const text of words.made?.lines??[])lines.push({part:'made',text,lang:words.made.language});
  if(work.size_mm){const number=new Intl.NumberFormat(locale);lines.push({part:'size',text:t('mediaSize').replace('{0}',number.format(work.size_mm[0])).replace('{1}',number.format(work.size_mm[1]))});}
  return lines;
}

/**
 * Where the caption's sources lead: the holder's record of the work and the rights statement of the image.
 * A provider that marks an image public domain by a flag (the Met's isPublicDomain) says so on its record.
 */
export function captionLinks({work,image,names=null},{t=key=>key,locale='en'}={}){
  const holder=workWords(work,names,locale).holder;
  const links=[{part:'record',text:[holder?.text,work.inventory].filter(Boolean).join(' · '),url:work.record}];
  const rights=image.rights;
  // The Public Domain Mark says public domain in the reader's words, whatever label the provider gives it.
  if(rights.statement===PUBLIC_DOMAIN_MARK)return [...links,{part:'rights',text:t('mediaPublicDomain'),url:rights.statement}];
  // A Commons file's copyright status, or its page's licence, says public domain on the file's own page.
  if(rights.field==='P6216'&&rights.value==='Q19652'||rights.field==='License'&&rights.value==='pd')return [...links,{part:'rights',text:t('mediaCommonsPublicDomain'),url:rights.stated_in}];
  // A flag names no statement: the Met's isPublicDomain and the Saint Louis Art Museum's Rights say public domain, the
  // National Gallery of Art's openaccess open access; their records state it.
  const flag=rights.statement===null?({isPublicDomain:'mediaPublicDomain',openaccess:'mediaOpenAccess',Rights:'mediaPublicDomain'})[rights.field]:null;
  const set=flag&&rights.value===({isPublicDomain:'true',openaccess:'1',Rights:'Public Domain'})[rights.field];
  links.push({part:'rights',text:set?t(flag):rights.label??rights.statement??rights.value,url:rights.statement??(set?work.record:rights.stated_in)});
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
    const picture=el('img'),sources=imageSources(image);
    // The page is cross-origin isolated: an image from the provider's service is requested in CORS mode, which the
    // service answers; the stand's own copies come from the site itself.
    if(sources.crossOrigin)picture.crossOrigin=sources.crossOrigin;
    picture.decoding='async';picture.loading='lazy';
    picture.width=size.width;picture.height=size.height;
    picture.src=sources.src;if(sources.srcset)picture.srcset=sources.srcset;
    const name=workName(item,locale());picture.alt=name.text;if(name.lang)picture.lang=name.lang;
    picture.addEventListener('error',()=>{open.replaceWith(el('p','ps-media-missing',t('mediaMissing')));},{once:true});
    open.append(picture);open.addEventListener('click',()=>void openViewer(item,open));
    const caption=el('figcaption');
    for(const line of captionLines(item,{t,locale:locale()})){
      const row=el('p',`ps-media-${line.part}`);
      if(line.label)row.append(el('span','ps-media-label',`${line.label}: `));
      const words=el('span','',line.text);if(line.lang)words.lang=line.lang;row.append(words);
      caption.append(row);
    }
    const links=el('p','ps-media-sources');
    for(const link of captionLinks(item,{t,locale:locale()})){const anchor=el('a','',`${link.text} ↗`);anchor.href=link.url;anchor.target='_blank';anchor.rel='noopener noreferrer';anchor.dataset.part=link.part;links.append(anchor);}
    caption.append(links);
    box.append(open,caption);
    return box;
  }

  // A small copy in a link's strip: the work's title and date as its hint, the viewer on a click.
  function thumb(item){
    const {image,work}=item,size=fittedSize(image,STRIP_BOX),sources=imageSources(image,{box:STRIP_BOX});
    const name=workName(item,locale()).text,when=work.dating?formatDating(work.dating,locale()):'';
    const open=el('button','ps-media-thumb');open.type='button';open.setAttribute('aria-label',`${t('mediaOpen')}: ${name}`);hint(open,[name,when].filter(Boolean).join(' · '));
    const picture=el('img');if(sources.crossOrigin)picture.crossOrigin=sources.crossOrigin;
    picture.decoding='async';picture.loading='lazy';picture.width=size.width;picture.height=size.height;
    picture.src=sources.src;if(sources.srcset)picture.srcset=sources.srcset;picture.alt=name;
    picture.addEventListener('error',()=>open.remove(),{once:true});
    open.append(picture);open.addEventListener('click',()=>void openViewer(item,open));
    return open;
  }

  async function openViewer(item,opener){
    closeViewer();
    const {image,work}=item,host=document.fullscreenElement??document.body;
    // A modal dialog holds the page: the stand's keys, its hints and its controls let it be while it is open.
    const layer=el('dialog','ps-media-viewer');
    const name=workName(item,locale()),holder=workWords(work,item.names??null,locale()).holder;layer.setAttribute('aria-label',name.text);
    const stage=el('div','ps-media-stage'),bar=el('div','ps-media-bar'),title=el('p','ps-media-viewer-title');
    const words=el('span','',name.text);if(name.lang)words.lang=name.lang;
    const when=work.dating?formatDating(work.dating,locale()):'';
    title.append(words);if(when)title.append(el('span','ps-media-viewer-date',` · ${when}`));if(holder)title.append(el('span','ps-media-viewer-holder',` · ${holder.text}`));
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
      const sources=imageSources(image);
      state.osd=withoutWrittenStyle(()=>OpenSeadragon({
        element:stage,tileSources:sources.full,crossOriginPolicy:sources.crossOrigin?'Anonymous':false,
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
    /** Put a link's media `groups` (from `linkGroups`) into `parent` as a strip of small copies. */
    renderStrip(parent,groups){
      if(!groups?.length)return;
      const strip=el('div','ps-media-strip');
      for(const group of groups){
        const box=el('div','ps-media-group');
        if(group.label)box.append(el('p','ps-media-end',group.label));
        const row=el('div','ps-media-thumbs');for(const item of group.items)row.append(thumb(item));
        box.append(row);strip.append(box);
      }
      parent.append(strip);
    },
    close:closeViewer,
  };
}
