/** Original-language identity is supplied by the source adapter, never guessed. */
export function readerVersionCodes(document){
 const rank=code=>code==='ru'?0:code==='en'?1:code===document.originalLanguage?2:3;
 return Object.keys(document.versions??{}).sort((a,b)=>rank(a)-rank(b)||a.localeCompare(b));
}

/** Compare two independent editions; adding an original must not shrink three columns. */
export function readerComparisonVersion(document,active,preferred=null){
 const codes=readerVersionCodes(document).filter(code=>code!==active);
 return codes.includes(preferred)?preferred:codes.includes(document.originalLanguage)?document.originalLanguage:codes[0]??null;
}

export function readerDisplayVersions(document,active,mode,preferred=null){
 if(mode!=='parallel')return [active];
 const other=readerComparisonVersion(document,active,preferred);
 return readerVersionCodes(document).filter(code=>code===active||code===other);
}
