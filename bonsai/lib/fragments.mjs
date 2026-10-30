// The places of works a stand cites beyond its own witnesses, and the alignment
// of the corpus works it cites.
//
// `fragments.json` (tos.bonsai.fragments.v1) holds the exact text of each
// cited place of a work that is neither one of the stand's witnesses nor in the
// public corpus: an aphorism, a proposition, a section, a fragment of a lost
// work. Every witness of such a work names its edition, the URL its text was
// taken from, its licence and attribution, and the basis on which the stand may
// show it: public domain, public domain with a licence of the digital copy, or a
// short quotation under the quotation right of a protected work. A protected
// work the stand cannot quote exactly is cited by its locator only.
//
// `alignment.json` (tos.bonsai.corpus-alignment.v1) holds, for each cited unit
// of an original in the public corpus, its counterparts in the corpus
// translations and the sentence pairs inside them, made by align.mjs: unit ids
// and offsets only, never text.

export const FRAGMENTS_SCHEMA='tos.bonsai.fragments.v1';
export const CORPUS_ALIGNMENT_SCHEMA='tos.bonsai.corpus-alignment.v1';
const BASES=['public-domain','public-domain+licence','short-quotation','locator-only'];
// What a link opens: the whole work, the page a quotation is taken from, or where a book can be found.
const OPENS=['work','quotation','book'];
const bilingual=value=>typeof value?.ru==='string'&&value.ru.trim()&&typeof value?.en==='string'&&value.en.trim();

/** Problems of a stand's places of other works. */
export function externalPlacesIssues(module){
  const problems=[];
  if(module?.schema!==FRAGMENTS_SCHEMA)problems.push(`schema is not ${FRAGMENTS_SCHEMA}`);
  const works=module?.works??{},places=module?.places??{};
  for(const [id,work] of Object.entries(works)){
    if(!bilingual(work.title))problems.push(`work ${id} has no title in both languages`);
    if(typeof work.original!=='string'||!work.original)problems.push(`work ${id} names no original language`);
    for(const [language,witness] of Object.entries(work.witnesses??{})){
      for(const field of ['edition','attribution'])if(!witness?.[field])problems.push(`work ${id} ${language} witness has no ${field}`);
      if(!BASES.includes(witness?.rights?.basis))problems.push(`work ${id} ${language} witness has no rights basis`);
      if(witness?.rights?.basis!=='locator-only'&&!/^https:\/\//.test(witness?.url??''))problems.push(`work ${id} ${language} witness has no source URL`);
      if(witness?.opens!==undefined&&!OPENS.includes(witness.opens))problems.push(`work ${id} ${language} witness opens ${witness.opens}`);
    }
  }
  for(const [id,list] of Object.entries(places)){
    const work=works[id];
    if(!work){problems.push(`places of unknown work ${id}`);continue;}
    for(const [loc,place] of Object.entries(list)){
      if(!bilingual(place.label))problems.push(`${id} ${loc} has no label in both languages`);
      // A place may link its own page in a witness, or a page for the place as a whole (where a cited-only work can be found).
      for(const [language,url] of Object.entries(place.links??{}))if(!work.witnesses?.[language]||!/^https:\/\//.test(url??''))problems.push(`${id} ${loc} links ${language} without a witness or a https URL`);
      if(place.url!==undefined&&!/^https:\/\//.test(place.url))problems.push(`${id} ${loc} has a URL that is not https`);
      if(place.opens!==undefined&&!OPENS.includes(place.opens))problems.push(`${id} ${loc} opens ${place.opens}`);
      for(const [language,text] of Object.entries(place.texts??{})){
        if(text===null)continue;
        if(typeof text!=='string'||!text.trim())problems.push(`${id} ${loc} ${language} text is empty`);
        const basis=work.witnesses?.[language]?.rights?.basis;
        if(!basis)problems.push(`${id} ${loc} has ${language} text without a witness`);
        if(basis==='locator-only')problems.push(`${id} ${loc} shows ${language} text of a work cited by locator only`);
        if(basis==='short-quotation'&&text.length>900)problems.push(`${id} ${loc} ${language} quotation is longer than a short quotation`);
      }
    }
  }
  return problems;
}

/** Problems of a stand's alignment of corpus works: known works, counterparts and well-formed sentence pairs. */
export function corpusAlignmentIssues(module){
  const problems=[];
  if(module?.schema!==CORPUS_ALIGNMENT_SCHEMA)problems.push(`schema is not ${CORPUS_ALIGNMENT_SCHEMA}`);
  const works=module?.works??{};
  for(const [id,work] of Object.entries(works)){
    if(!/^tos\.work\./.test(work.documentId??''))problems.push(`work ${id} has no corpus document`);
    if(!work.original?.versionId||!work.original?.language)problems.push(`work ${id} has no original version`);
    for(const item of work.translations??[])if(!item.versionId||!item.language)problems.push(`work ${id} has a translation without version or language`);
  }
  for(const [unit,entry] of Object.entries(module?.units??{})){
    const work=works[entry.w];
    if(!work){problems.push(`unit ${unit} of unknown work ${entry.w}`);continue;}
    const versions=new Set((work.translations??[]).map(item=>item.versionId));
    for(const [version,target] of Object.entries(entry.t??{})){
      if(!versions.has(version))problems.push(`unit ${unit}: ${version} is not a translation of ${entry.w}`);
      if(!Array.isArray(target.u)||!target.u.length)problems.push(`unit ${unit} ${version}: no counterpart`);
      for(const [source,translation] of target.pairs??[]){
        for(const [s,e] of source??[])if(!(Number.isInteger(s)&&Number.isInteger(e)&&0<=s&&s<e))problems.push(`unit ${unit} ${version}: bad span ${s}-${e}`);
        for(const [i,s,e] of translation??[])if(!(Number.isInteger(i)&&i>=0&&i<(target.u?.length??0)&&Number.isInteger(s)&&Number.isInteger(e)&&0<=s&&s<e))problems.push(`unit ${unit} ${version}: bad counterpart ${i} ${s}-${e}`);
      }
    }
  }
  return problems;
}
