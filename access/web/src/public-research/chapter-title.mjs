// A chapter of a stand's book as its witness titles it: without numbering or a closing period, and an English
// title printed in capitals read in title case. Shared by the stand's quotations, the workbench and the word's origin.

const SMALL=new Set(['a','an','and','as','at','by','for','from','in','of','on','or','the','to','with']);

/** The title of `chapter` ({titles: {language: title}}) in `language`, or '' where that witness has none. */
export function chapterTitle(chapter,language){
  let value=(chapter?.titles?.[language]??'').replace(/\.$/,'').trim();
  if(language==='en'){
    value=value.replace(/^[IVXLC]+\.\s*/,'');
    if(value===value.toUpperCase())value=value.toLowerCase().split(' ').map((word,index)=>index&&SMALL.has(word)?word:word.replace(/\p{L}/u,letter=>letter.toUpperCase())).join(' ');
  }
  return value;
}
