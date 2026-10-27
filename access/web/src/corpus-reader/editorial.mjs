import annotations from '../../../public-corpus/editorial-annotations.v1.json' with {type:'json'};
import retained from '../../../public-corpus/retained-library-map.v1.json' with {type:'json'};

const versionSpans = new Map(annotations.versions.map(version => [version.versionId, version]));
const apparatus = new Map(retained.books.flatMap(book=>book.versions.flatMap(version=>[
  ...(version.bookNotesUnitIds??[]).map(id=>[id,{kind:'notes',versionId:version.versionId,revision:version.revision}]),
  ...(version.adjustmentUnitIds??[]).map(id=>[id,{kind:'adjustments',versionId:version.versionId,revision:version.revision}]),
])));
const labels = {
  ru:{note:'Примечание Rackham',addition:'Редакторское дополнение',deletion:'Исключение издателя',gap:'Пропуск в издании',
    notes:'Примечания к произведению',adjustments:'Сведения обработки текста',publication:'Материалы публикации и лицензии Project Gutenberg',
    fragments:'Редакторское собрание фрагментов и свидетельств',laterWitness:'Позднее свидетельство: письмо Гильдеберта и переданные фрагменты'},
  en:{note:'Note by Rackham',addition:'Editorial addition',deletion:'Editorial deletion',gap:'Gap in the edition',
    notes:'Notes on the work',adjustments:'Text processing information',publication:'Project Gutenberg publication and license material',
    fragments:'Editorial collection of fragments and testimonies',laterWitness:'Later witness: Hildebert’s letter and transmitted fragments'},
};
const language=locale=>String(locale||'ru').toLowerCase().startsWith('en')?'en':'ru';
const unitId=unit=>unit?.unitId||unit?.id;
const points=value=>Array.from(String(value??''));
const revision=version=>version?.revision||version?.source?.textLayerSha256;

export const editorialLabel=(kind,locale='ru')=>labels[language(locale)][kind]||'';
export const hasEditorialProjection=version=>Boolean(versionSpans.get(version?.id) && versionSpans.get(version.id).revision===revision(version));

// Data apply only to the exact version and layer, and only while the source
// string still matches the independently projected half-open range.
export function editorialRanges(version,unit){
  const data=versionSpans.get(version?.id);
  if(!unit || typeof unit.text!=='string')return [];
  const text=points(unit.text);
  const ranges=data?.revision===revision(version) ? data.spans.filter(span=>span.unitId===unitId(unit) && span.start>=0 && span.end>span.start
    && span.end<=text.length && text.slice(span.start,span.end).join('')===span.expected)
    : [];
  if(version?.id==='retained-de-fato-lat1-b6f9f292b185' && revision(version)==='dc65d650f24a16ca9d017debfe1c880191fa4eebdccd94d070731884e13d04e0'
    && unitId(unit)==='tos.text-unit.sid-d3bc0bc17eff4b6ca63c698e89b0f3de' && text.slice(619,628).join('')==='Fragmenta'){
    ranges.push({start:619,end:text.length,kind:'fragments'});
  }
  // Basore's retained II.7 continues after the surviving chapter with a
  // later letter and the fragments it transmits. Preserve the whole carrier
  // and mark only the independently identified boundary in this exact layer.
  const letter='Hildebertus Cenomanensis ep. 1.3 (CLXXI. 145 Mign.)';
  if(version?.id==='retained-de-clementia-lat2-a170840d8fc4' && revision(version)==='dff84f24224394557b5009e99cddb22c6c37e5f56f8b0c0a95e175b1e9d218c3'
    && unitId(unit)==='tos.text-unit.sid-19fbcffd2c0f44c2bf340365636cc66b' && text.slice(2074,2074+points(letter).length).join('')===letter){
    ranges.push({start:2074,end:text.length,kind:'laterWitness'});
  }
  return ranges.sort((a,b)=>a.start-b.start || a.end-b.end);
}

export function editorialKindForMatch(version,unit,start,end){
  return sourceLayerKind(version,unit,start) || editorialRanges(version,unit)
    .find(span=>start<span.end && end>span.start)?.kind || null;
}

export function quotationWithEditorialRoles(version,unit,selector,locale='ru'){
  const chars=points(unit?.text), selected=selector && selector.end>selector.start;
  const start=selected?selector.start:0, end=selected?selector.end:chars.length;
  let cursor=start, value='';
  for(const span of editorialRanges(version,unit)){
    const from=Math.max(start,span.start),to=Math.min(end,span.end);
    if(to<=from || from<cursor)continue;
    value+=chars.slice(cursor,from).join('');
    value+=` [${editorialLabel(span.kind,locale)}: ${chars.slice(from,to).join('')}] `;
    cursor=to;
  }
  return value+chars.slice(cursor,end).join('');
}

// These classifications describe public-carrier material, not an inferred
// authorial voice or a change to the text or its canonical unit.
export function sourceLayerKind(version,unit,start=null){
  const id=unitId(unit), separate=apparatus.get(id);
  if(separate && separate.versionId===version?.id && separate.revision===revision(version))return separate.kind;
  if(version?.id==='gutenberg-7205-de' && revision(version)==='4f71adfeeb39e64055acc2fc3f58964cf39f03d27bf2264d25313bc45d8658ec'
    && Number.isSafeInteger(unit?.ordinal) && unit.ordinal>=3694)return 'publication';
  if(version?.id==='retained-de-fato-lat1-b6f9f292b185' && revision(version)==='dc65d650f24a16ca9d017debfe1c880191fa4eebdccd94d070731884e13d04e0'
    && id==='tos.text-unit.sid-d3bc0bc17eff4b6ca63c698e89b0f3de' && Number.isSafeInteger(start) && start>=619)return 'fragments';
  return null;
}
