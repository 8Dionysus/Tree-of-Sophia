// Aligning a bonsai's stored witnesses again, from their own texts.
//
// The stored rows of a witnesses module are what `align.mjs` makes of the
// stored units; running it again must give them back. A change to the
// mechanism or its parameters shows here first, chapter by chapter, before it
// is stored.

import {alignWitnesses,chapterRows,triangle,weakGroups} from './align.mjs';
import {readChapter,readingText} from './witnesses.mjs';

const plainRows=rows=>JSON.stringify(rows);

/**
 * Align the pivot of a witnesses module with each translation, chapter by
 * chapter, and the first two translations with each other for the triangle.
 * Returns the chapter files with fresh rows, the chapters whose rows changed,
 * the alignment receipt for the manifest and the groups to review.
 */
export function realignWitnesses(manifest,files){
  const pivot=manifest.pivot,translations=Object.keys(manifest.witnesses).filter(language=>language!==pivot);
  const chapters=Object.fromEntries(Object.entries(files).map(([key,file])=>[key,readChapter(file)]));
  const side=language=>Object.fromEntries(Object.entries(chapters).filter(([,chapter])=>chapter.units[language]?.length).map(([key,chapter])=>[key,chapter.units[language].map(unit=>({...unit,text:readingText(unit.raw,unit.kind)}))]));
  const units=Object.fromEntries([pivot,...translations].map(language=>[language,side(language)]));
  const pairings=Object.fromEntries(translations.map(language=>[language,alignWitnesses(units[pivot],units[language],pivot,language)]));
  const groupsOf=result=>Object.values(result.chapters).flatMap(chapter=>chapter.groups);
  const shapes=result=>{const counts={};for(const group of groupsOf(result))counts[group.shape]=(counts[group.shape]??0)+1;return counts;};
  let between=null,check=null;
  if(translations.length>=2){
    const [a,b]=translations;
    between=alignWitnesses(units[a],units[b],a,b,{sentences:false});
    check=triangle(groupsOf(pairings[a]),groupsOf(pairings[b]),groupsOf(between));
  }
  const out={},changed=[];
  for(const [key,file] of Object.entries(files)){
    const chapter=chapters[key];
    const present=Object.fromEntries(Object.entries(chapter.units).filter(([,list])=>list.length));
    const groups=Object.fromEntries(translations.filter(language=>present[language]&&pairings[language].chapters[key]).map(language=>[language,pairings[language].chapters[key].groups]));
    const rows=chapterRows(present,groups,pivot);
    const rowOf=new Map();rows.forEach((row,n)=>{for(const i of row[pivot]??[])rowOf.set(i,n);});
    for(const [language,list] of Object.entries(groups)){
      for(const group of list){
        if(!group.s.length||!group.sentences?.length)continue;
        const row=rows[rowOf.get(chapter.index[pivot].get(group.s[0]))];
        ((row.sentences??={})[language]??=[]).push(...group.sentences);
      }
    }
    if(plainRows(rows)!==plainRows(file.rows))changed.push(key);
    out[key]={...file,rows};
  }
  const pairs={};
  for(const language of translations){
    const result=pairings[language];
    pairs[`${pivot}-${language}`]={ratio:result.ratio,variance:result.variance,modelPairs:result.modelPairs,groups:shapes(result),sentencePairs:groupsOf(result).reduce((n,group)=>n+(group.sentences?.length??0),0)};
  }
  if(between)pairs[`${between.source}-${between.target}`]={ratio:between.ratio,variance:between.variance,modelPairs:between.modelPairs,groups:shapes(between)};
  const toReview=[];
  for(const [name,result] of [...translations.map(language=>[`${pivot}-${language}`,pairings[language]]),...(between?[[`${between.source}-${between.target}`,between]]:[])])
    for(const group of weakGroups(result))toReview.push({pair:name,chapter:group.chapter,shape:group.shape,uncovered:group.uncovered,source:group.s,target:group.t});
  const alignment={pairs,...(check?{triangle:{agree:check.agree,disagree:check.disagree,skipped:check.skipped}}:{}),toReview};
  return {chapters:out,changed,alignment,receipt:{pairs,triangle:alignment.triangle??null,toReview:toReview.length}};
}
