// The pages of the first editions beside a chapter (H1, TOS-D-0084): a witness with a page map (bonsai/<stand>/pages/)
// names the printed pages a chapter's units stand on, and each page image opens from its holder. The pages stand in
// the chapter window's technical section (TOS-D-0082): they serve checking a transcription, not the reading. A
// witness without a page map shows nothing.
import {technicalSection} from './technical.mjs';
import {localeText,resolveLocale} from './locale.mjs';

const MAPS=import.meta.glob('../../../../bonsai/zarathustra/pages/*.json',{import:'default'});
const maps=new Map();

/** The page map of a witness by its language code, or null. */
export function pageMapFor(language,loaders=MAPS){
  const path=Object.keys(loaders).find(item=>item.endsWith(`/${language}.json`));
  if(!path)return Promise.resolve(null);
  if(!maps.has(path))maps.set(path,loaders[path]().catch(()=>null));
  return maps.get(path);
}

/** The pages a set of units stands on, in order, each once: [{index, n, image}]. */
export function pagesOf(map,unitIds){
  const indexes=new Set();
  for(const id of unitIds){const span=map?.units?.[id];if(!span)continue;for(let i=span[0];i<=span[1];i++)indexes.add(i);}
  return [...indexes].sort((a,b)=>a-b).map(index=>({index,n:map.pages[index]?.n??String(index+1),image:map.pages[index]?.image??null})).filter(page=>page.image);
}

const labels={pages:['Страницы издания','Pages of the edition'],page:['с.','p.'],rights:['Права на изображения','Rights of the images']};

/** The technical section of a chapter window: per witness with a map, its pages for the chapter's units. */
export async function editionPagesSection(doc,locale,{data,languages,loaders=MAPS}){
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const blocks=[];
  for(const language of languages){
    const map=await pageMapFor(language,loaders);if(!map)continue;
    const pages=pagesOf(map,(data.units[language]??[]).map(unit=>unit.id));if(!pages.length)continue;
    const block=doc.createElement('div');block.className='ep-witness';
    const head=doc.createElement('p');head.textContent=`${language.toUpperCase()} · ${t('page')} ${pages[0].n}–${pages.at(-1).n}`;block.append(head);
    const strip=doc.createElement('div');strip.className='ep-pages';
    for(const page of pages){
      const link=doc.createElement('a');link.href=page.image;link.target='_blank';link.rel='noopener noreferrer';link.className='ep-page';
      const image=doc.createElement('img');image.loading='lazy';image.decoding='async';image.alt=`${t('page')} ${page.n}`;image.dataset.src=page.image;
      const label=doc.createElement('span');label.textContent=`${t('page')} ${page.n}`;link.append(image,label);strip.append(link);
    }
    block.append(strip);
    if(map.rights?.statement){const rights=doc.createElement('p');const link=doc.createElement('a');link.href=map.rights.url??'#';link.target='_blank';link.rel='noopener noreferrer';link.textContent=`${t('rights')}: ${map.rights.statement}`;rights.append(link);block.append(rights);}
    blocks.push(block);
  }
  if(!blocks.length)return null;
  const section=technicalSection(doc,locale,blocks,{label:t('pages')});
  // The images come from their holders only when the reader opens the door.
  section.addEventListener('toggle',()=>{if(section.open)for(const image of section.querySelectorAll('img[data-src]')){image.src=image.dataset.src;delete image.dataset.src;}});
  return section;
}
