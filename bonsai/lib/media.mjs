// A bonsai's media: images of works that a stand shows on its cards.
//
// The module keeps three levels, as the Tree does. A work is the object its
// holder keeps (an engraving, a painting, a manuscript leaf), with the
// provider's own titles, makers, dating, size and record. An image is one
// reproduction of a work, with its pixel size and the rights statement the
// provider gives for it: either the provider's IIIF image service, which the
// page reads from the provider, or the provider's file and its smaller copy,
// which the stand keeps as its own copies in its layer store
// (`layers.json`, R2) and the page reads from the site. A use binds an image to a
// subject of the stand in a role: an author's portrait, a material of an era,
// an object of analysis, a comparison. Provider texts are kept word for word,
// with their language. A card shows the image and a caption built from these
// facts and their sources.
//
// Each work stands for a Tree Artifact and each image for a Tree
// representation; the graft map (`identities.json`, sections `artifacts` and
// `representations`) says which, and whether the Tree already holds them.
// Everything here comes from a provider observation made by the Tree's media
// bridges (`tos-native-owner-command acquisition`, `media.observe`), and
// `addMedia` turns one observation into the module's entries.
//
// A reader sees a work in one language: the language the interface speaks to
// them in (`translationLanguage`). The Tree's own names of a work, its makers,
// its series and its holder (`media-names.json`, `tos.bonsai.media-names.v1`)
// stand where the providers' words are in another language; `workWords` picks
// them, and `mediaNamesIssues` asks every work to be named in each language
// the stands are written in. The words on the object itself stand as written.

import {MEDIA_HOSTS,MEDIA_ROLES} from './media-hosts.mjs';
import {BONSAI_LOCALES,translationLanguage} from './locales.mjs';
import {dateIssues,yearLexeme} from './time.mjs';

export {MEDIA_HOSTS,MEDIA_ROLES};
export const MEDIA_SCHEMA='tos.bonsai.media.v1';
/** The kinds of title a provider gives: the preferred one, the one written on the object, the series. */
export const TITLE_ROLES=Object.freeze(['title','inscribed','series']);
/** The image APIs a card reads: a provider's IIIF Image API 3 or 2 service, or a file the stand keeps a copy of. */
export const IMAGE_APIS=Object.freeze(['iiif-image-3','iiif-image-2','file']);
const IIIF=new Set(['iiif-image-3','iiif-image-2']);
/**
 * Flags by which a provider marks an image free to use, where it names no statement: the Met's, the National
 * Gallery of Art's, the copyright status a Wikimedia Commons file's structured data states (P6216: public domain)
 * or, where it states none, the public-domain licence of the file's page, and the rights the Saint Louis Art
 * Museum's object page states.
 */
export const OPEN_FLAGS=Object.freeze([{field:'isPublicDomain',value:'true'},{field:'openaccess',value:'1'},{field:'P6216',value:'Q19652'},{field:'License',value:'pd'},{field:'Rights',value:'Public Domain'}]);
export const MEDIA_NAMES_SCHEMA='tos.bonsai.media-names.v1';
/** The languages every work of a stand is named in: the languages the stands are written in. */
export const NAMED_LANGUAGES=Object.freeze(['en','ru']);
/** What of a work the Tree names in a reader's language: its preferred titles, its series and who made it. */
export const NAME_PARTS=Object.freeze(['titles','series','made']);
/**
 * The language a provider writes its records in, where it tags none: the Met's, the National Gallery of Art's,
 * Saint Louis's and the Wikidata labels the Commons bridge reads are English, Berlin's German, the Rijksmuseum's Dutch.
 */
export const PROVIDER_LANGUAGES=Object.freeze({met:'en',nga:'en',commons:'en',slam:'en',smb:'de',rijksmuseum:'nl'});
/** Where a stand keeps the copies of image files, inside its layer store. */
export const MEDIA_LAYER_PATH='media/';
const FILE_TYPES=Object.freeze({'image/jpeg':'jpg','image/png':'png'});
/** Rights statements under which a stand shows an image. */
export const OPEN_STATEMENTS=Object.freeze([
  'https://creativecommons.org/publicdomain/mark/1.0/',
  'https://creativecommons.org/publicdomain/zero/1.0/',
]);

const TITLE_ROLE_URIS=Object.freeze({
  'http://vocab.getty.edu/aat/300404670':'title',
  'title on object':'inscribed',
  'series title':'series',
});
const MILLIMETRES='http://vocab.getty.edu/aat/300379097';
const ARTIFACT=/^tos\.artifact\.[a-z0-9]+(?:-[a-z0-9]+)*\.[a-z0-9]+(?:-[a-z0-9]+)*\.[a-z0-9]+(?:-[a-z0-9]+)*$/;
const ARTIFACT_PATH=/^[a-z0-9]+(?:-[a-z0-9]+)*\/[a-z0-9]+(?:-[a-z0-9]+)*\/[a-z0-9]+(?:-[a-z0-9]+)*$/;
const REPRESENTATION=/^tos\.(?:image-service|file)\.sha256\.[0-9a-f]{64}$/;
const STATUSES=Object.freeze(['to-mint','resolved']);

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const text=value=>typeof value==='string'&&/\S/.test(value);
const https=value=>typeof value==='string'&&/^https:\/\/\S+$/.test(value);
const uri=value=>typeof value==='string'&&/^https?:\/\/\S+$/.test(value);
const sha=value=>typeof value==='string'&&/^[0-9a-f]{64}$/.test(value);
const instant=value=>typeof value==='string'&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value);
const worded=value=>object(value)&&text(value.text)&&(value.language===null||text(value.language))&&Object.keys(value).every(key=>key==='text'||key==='language');
const hostOf=value=>{try{return new URL(value).host;}catch{return null;}};

/** Where in a stand's layer store the copy of a file stands. */
export const mediaFilePath=file=>`${MEDIA_LAYER_PATH}${file.sha256}.${FILE_TYPES[file.media_type]??'bin'}`;
const fileIssues=(file,at)=>{
  if(!object(file))return [`${at}: missing`];
  const issues=[];
  if(!https(file.source))issues.push(`${at}: source`);
  if(!sha(file.sha256))issues.push(`${at}: sha256`);
  if(!Number.isInteger(file.bytes)||file.bytes<=0)issues.push(`${at}: bytes`);
  for(const key of ['width','height'])if(!Number.isInteger(file[key])||file[key]<=0)issues.push(`${at}: ${key}`);
  if(!FILE_TYPES[file.media_type])issues.push(`${at}: media_type`);
  return issues;
};

/** The identifier that keys a work where its inventory number names it only within one museum. */
const WORK_KEYS=Object.freeze({commons:'Wikidata',smb:'SMB ObjID'});

/** Whether a rights entry lets a stand show the image. */
export const openRights=rights=>object(rights)&&(OPEN_STATEMENTS.includes(rights.statement)||rights.statement===null&&OPEN_FLAGS.some(flag=>flag.field===rights.field&&flag.value===rights.value));

/**
 * Where a media module departs from its own contract; empty when it holds.
 * `subjects` (a Set) are the stand's subject ids a use may name; `identities`
 * is the graft map, which must name a Tree identity for every work and image;
 * `digest(text)` (hex SHA-256) checks that a representation is named after
 * its image service, as the Tree names it; `layers` (the stand's layer
 * manifest) must hold every file copy with its digest and size.
 */
export function mediaIssues(module,{subjects=null,identities=null,digest=null,layers=null}={}){
  if(!object(module))return ['not an object'];
  const issues=[];
  if(module.schema!==MEDIA_SCHEMA)issues.push('schema');
  if(!text(module.bonsai))issues.push('bonsai');
  const works=object(module.works)?module.works:{},images=object(module.images)?module.images:{},uses=Array.isArray(module.uses)?module.uses:[];
  if(!object(module.works))issues.push('works');
  if(!object(module.images))issues.push('images');
  if(!Array.isArray(module.uses))issues.push('uses');
  for(const [id,work] of Object.entries(works)){
    const at=`work ${id}`;
    if(!object(work)){issues.push(`${at}: not an object`);continue;}
    if(!text(work.provider))issues.push(`${at}: provider`);
    if(!https(work.record))issues.push(`${at}: record`);
    if(!text(work.holder))issues.push(`${at}: holder`);
    if(!text(work.inventory))issues.push(`${at}: inventory`);
    const titles=Array.isArray(work.titles)?work.titles:[];
    if(!titles.some(title=>title?.role==='title'))issues.push(`${at}: no preferred title`);
    for(const title of titles)if(!object(title)||!text(title.text)||!(title.language===null||text(title.language))||!TITLE_ROLES.includes(title.role))issues.push(`${at}: title`);
    if(!Array.isArray(work.made)||work.made.some(line=>!worded(line)))issues.push(`${at}: made`);
    if(work.dating!==null)for(const issue of dateIssues(work.dating))issues.push(`${at}: dating ${issue}`);
    for(const key of ['classification','materials'])if(!Array.isArray(work[key])||work[key].some(item=>!text(item)))issues.push(`${at}: ${key}`);
    if(work.size_mm!==null&&!(Array.isArray(work.size_mm)&&work.size_mm.length===2&&work.size_mm.every(value=>Number.isFinite(value)&&value>0)))issues.push(`${at}: size_mm`);
    if(!Array.isArray(work.credit)||work.credit.some(line=>!worded(line)))issues.push(`${at}: credit`);
    if(!Array.isArray(work.depicts)||work.depicts.some(item=>!uri(item)))issues.push(`${at}: depicts`);
    if(!object(work.observed)||!instant(work.observed.at)||!sha(work.observed.record_sha256))issues.push(`${at}: observed`);
    if(identities){
      const graft=identities.artifacts?.[id];
      if(!object(graft)||!ARTIFACT.test(graft.tree??'')||!STATUSES.includes(graft.status))issues.push(`${at}: no Tree Artifact in the graft map`);
      else if(!ARTIFACT_PATH.test(graft.path??'')||graft.tree!==`tos.artifact.${graft.path.replaceAll('/','.')}`)issues.push(`${at}: the graft map's Artifact path does not name its Artifact`);
    }
  }
  const shown=new Set();
  for(const [id,image] of Object.entries(images)){
    const at=`image ${id}`;
    if(!object(image)){issues.push(`${at}: not an object`);continue;}
    if(!works[image.work])issues.push(`${at}: work ${image.work} is not a work of this module`);
    else shown.add(image.work);
    if(!IMAGE_APIS.includes(image.api))issues.push(`${at}: api`);
    if(IIIF.has(image.api)){
      if(!https(image.service))issues.push(`${at}: service`);
      else if(!MEDIA_HOSTS.includes(hostOf(image.service)))issues.push(`${at}: ${hostOf(image.service)} is not a media host; add it to bonsai/lib/media-hosts.mjs`);
      if(/\/$/.test(image.service??''))issues.push(`${at}: service ends with a slash`);
    }
    if(image.api==='file'){
      issues.push(...fileIssues(image.file,`${at}: file`),...fileIssues(image.preview,`${at}: preview`));
      if(image.file&&(image.width!==image.file.width||image.height!==image.file.height))issues.push(`${at}: its size is not its file's`);
      // The page reads the stand's own copies: the layer store holds each one, with its digest and size.
      if(!layers)issues.push(`${at}: file media requires a layer store`);
      else for(const [name,file] of [['file',image.file],['preview',image.preview]]){
        const entry=object(file)&&(layers.entries??[]).find(item=>item.path===mediaFilePath(file));
        if(object(file)&&!(entry&&entry.sha256===file.sha256&&entry.bytes===file.bytes))issues.push(`${at}: the layer store holds no copy of its ${name} (${object(file)?mediaFilePath(file):'?'})`);
      }
    }
    for(const key of ['width','height'])if(!Number.isInteger(image[key])||image[key]<=0)issues.push(`${at}: ${key}`);
    const rights=image.rights;
    if(!object(rights)||!(rights.statement===null||https(rights.statement))||!text(rights.field)||!text(rights.value)||!(rights.label===null||text(rights.label))||!https(rights.stated_in))issues.push(`${at}: rights`);
    else if(!openRights(rights))issues.push(`${at}: its rights do not let a stand show it`);
    if(!object(image.observed)||!instant(image.observed.at)||IIIF.has(image.api)&&!sha(image.observed.info_sha256))issues.push(`${at}: observed`);
    if(identities){
      const graft=identities.representations?.[id];
      if(!object(graft)||!REPRESENTATION.test(graft.tree??'')||!STATUSES.includes(graft.status))issues.push(`${at}: no Tree representation in the graft map`);
      // The Tree names an image service after its address and a file after its bytes.
      else if(IIIF.has(image.api)&&digest&&https(image.service)&&graft.tree!==`tos.image-service.sha256.${digest(image.service)}`)issues.push(`${at}: the graft map's representation is not named after its service`);
      else if(image.api==='file'&&graft.tree!==`tos.file.sha256.${image.file?.sha256}`)issues.push(`${at}: the graft map's representation is not named after its file`);
    }
  }
  for(const id of Object.keys(works))if(!shown.has(id))issues.push(`work ${id}: no image shows it`);
  const used=new Set(),seen=new Set();
  uses.forEach((use,index)=>{
    const at=`use ${index}`;
    if(!object(use)){issues.push(`${at}: not an object`);return;}
    if(Object.keys(use).some(key=>!['subject','image','role'].includes(key)))issues.push(`${at}: unknown field`);
    if(!text(use.subject))issues.push(`${at}: subject`);
    else if(subjects&&!subjects.has(use.subject))issues.push(`${at}: ${use.subject} is not a subject of the stand`);
    if(!images[use.image])issues.push(`${at}: image ${use.image} is not an image of this module`);
    else used.add(use.image);
    if(!MEDIA_ROLES.includes(use.role))issues.push(`${at}: role`);
    const key=`${use.subject}\u0000${use.image}\u0000${use.role}`;
    if(seen.has(key))issues.push(`${at}: repeats an earlier use`);
    seen.add(key);
  });
  for(const id of Object.keys(images))if(!used.has(id))issues.push(`image ${id}: no use shows it`);
  if(identities){
    for(const id of Object.keys(identities.artifacts??{}))if(!works[id])issues.push(`graft map artifact ${id}: not a work of the media module`);
    for(const id of Object.keys(identities.representations??{}))if(!images[id])issues.push(`graft map representation ${id}: not an image of the media module`);
  }
  return issues;
}

// The English statements a provider gives, without those another one already contains:
// "René Boyvin (mentioned on object)" goes where "printmaker: René Boyvin (mentioned on object)" stands.
function statements(entries){
  const all=(entries??[]).filter(entry=>text(entry?.text));
  const english=all.filter(entry=>entry.language==='en'),pool=english.length?english:all.filter(entry=>entry.language===all[0]?.language);
  const kept=pool.filter((entry,index)=>!pool.some((other,j)=>j!==index&&other.text!==entry.text&&other.text.includes(entry.text))&&pool.findIndex(other=>other.text===entry.text)===index);
  return kept.map(entry=>({text:entry.text,language:entry.language??null}));
}

// A provider dating as a Tree historicalDate in the provider's own words: one year, or the years of every
// timespan it names. Where the provider gives no dating or no words for it, the work has none: a gap stays a gap.
function datingOf(dating){
  let {statement=null,begin_year:begin=null,end_year:end=null}=dating??{};
  if(!text(statement)||!Number.isInteger(begin)&&!Number.isInteger(end))return null;
  const certainty=/\b(?:c|ca|circa)\b\.?|\?/i.test(statement)?'approximate':'exact';
  const base={role:'historical-time',calendar:null,year_numbering:'astronomical',certainty,source_wording:{text:statement,language:null}};
  // "In or after 1868" opens the interval forward, "in or before 1868" backward, whatever years the record states with it.
  if(/\b(?:in )?or (?:after|later)\b|\bafter\b/i.test(statement)&&Number.isInteger(begin))return {kind:'interval-assertion',...base,interval:{start:yearLexeme(begin)}};
  if(/\b(?:in )?or (?:before|earlier)\b|\bbefore\b/i.test(statement)&&Number.isInteger(end))return {kind:'interval-assertion',...base,interval:{end:yearLexeme(end)}};
  // An approximate dating keeps the years its words state ("ca. 1819–23"), where the record widens them (1814–1828).
  const stated=certainty==='approximate'?/(\d{3,4})(?:\s*[–-]\s*(\d{1,4}))?/.exec(statement):null;
  if(stated){
    const first=Number(stated[1]),tail=stated[2],last=tail?Number(tail.length<stated[1].length?stated[1].slice(0,stated[1].length-tail.length)+tail:tail):first;
    if(first<=last&&(!Number.isInteger(begin)||begin<=first)&&(!Number.isInteger(end)||last<=end)){begin=first;end=last;}
  }
  if(begin===end)return {kind:'date-assertion',...base,value:yearLexeme(begin)};
  return {kind:'interval-assertion',...base,interval:{...(Number.isInteger(begin)?{start:yearLexeme(begin)}:{}),...(Number.isInteger(end)?{end:yearLexeme(end)}:{})}};
}

// The height and width a provider states in millimetres or centimetres (to the millimetre): the whole object's,
// else the sheet's, else the image's.
function sizeOf(dimensions){
  const millimetres=item=>{const number=Number(item?.value);if(!Number.isFinite(number)||number<=0)return null;return item.unit_uri===MILLIMETRES||item.unit==='mm'?number:item.unit==='cm'?Math.round(number*10):null;};
  const pick=(axis,prefix)=>(dimensions??[]).find(item=>typeof item?.kind==='string'&&item.kind.toLowerCase()===`${prefix}${axis}`&&millimetres(item)!==null);
  for(const prefix of ['','overall ','sheet ','image ']){
    const height=pick('height',prefix),width=pick('width',prefix);
    if(height&&width)return [millimetres(height),millimetres(width)];
  }
  return null;
}

/**
 * The work and image entries of one provider observation (`media.observe`):
 * the work keyed `<provider>:<inventory>` (or by the provider's own item where an inventory names it only within
 * one museum: a Commons artwork's Wikidata item, an SMB object's ObjID), its primary image `<work>/<view>`.
 * Refuses an observation without an image service or whose image rights do
 * not let a stand show it.
 */
export function mediaFromObservation(observation,{view='whole'}={}){
  if(observation?.schema_version!=='tos_media_provider_observation_v1')throw new Error('not a media provider observation');
  const record=observation.object??{},provider=observation.provider;
  const item=(record.identifiers??[]).find(entry=>entry?.scheme===WORK_KEYS[provider])?.value;
  // The holder's number, else the provider's own item, else the object the observation asked for.
  const inventory=(record.identifiers??[]).find(entry=>/object number|accession number|inventory number/i.test(entry?.scheme??''))?.value
    ??(item?String(item).split('/').at(-1):observation.requested_object);
  const primary=(observation.images??[]).find(item=>item?.role==='primary');
  const service=primary?.form==='image_service'?primary.image_service:null;
  if(!primary)throw new Error(`${provider} ${inventory}: the observation has no primary image`);
  if(service&&!['2','3'].includes(service.api_version))throw new Error(`${provider} ${inventory}: IIIF Image API ${service.api_version} is not read by the cards`);
  if(!service&&!(primary.file_fingerprint&&primary.preview_fingerprint))throw new Error(`${provider} ${inventory}: a file image needs its file and its smaller copy fingerprinted; observe it with "files": true`);
  const right=(observation.rights??[]).find(item=>item?.scope==='image'
    &&(provider!=='rijksmuseum'||primary.rights_source&&item.stated_in===primary.rights_source));
  const rights=right?{statement:right.statement_uri??null,field:right.provider_field,value:right.value,label:right.label??null,stated_in:right.stated_in}:null;
  if(!openRights(rights))throw new Error(`${provider} ${inventory}: the provider's image rights do not let a stand show it`);
  const requestOf=role=>(observation.requests??[]).find(item=>item?.role===role);
  const workId=`${provider}:${item?String(item).split('/').at(-1):inventory}`,imageId=`${workId}/${view}`;
  // A provider that gives one title without a kind (the Met) gives its own title.
  const titles=(record.titles??[]).map(title=>({text:title.text,language:title.language??null,role:title.role===null||title.role===undefined?'title':TITLE_ROLE_URIS[title.role]??null})).filter(title=>title.role&&text(title.text));
  // Where the provider writes no production statement, its makers speak: "Artist: Eugène Delacroix".
  const made=statements(record.production_statements);
  if(!made.length)for(const maker of record.makers??[])if(text(maker?.label))made.push({text:text(maker.role)?`${maker.role}: ${maker.label}`:maker.label,language:null});
  const work={
    provider,record:record.record_url,holder:record.repository,inventory,
    titles,made,dating:datingOf(record.dating),
    classification:record.classifications??[],materials:record.materials??[],size_mm:sizeOf(record.dimensions),
    credit:statements(record.credit_lines),
    depicts:[...new Set((record.depicted??[]).flatMap(item=>item?.authority_uris??[]))],
    // A record read from a published release is its objects table at its commit.
    observed:{at:observation.observed_at,record_sha256:requestOf('object-record')?.sha256??(observation.datasets??[]).find(item=>/\/objects\.csv$/.test(item?.path??''))?.sha256},
  };
  const fileOf=print=>({source:print.final_url??print.url,sha256:print.sha256,bytes:print.byte_size,width:print.width_pixels,height:print.height_pixels,media_type:print.media_type});
  const image=service?{
    work:workId,api:`iiif-image-${service.api_version}`,service:service.service_id.replace(/\/$/,''),width:service.width_pixels,height:service.height_pixels,
    rights,observed:{at:service.info_fingerprint?.checked_at??observation.observed_at,info_sha256:service.info_fingerprint?.sha256},
  }:{
    work:workId,api:'file',file:fileOf(primary.file_fingerprint),preview:fileOf(primary.preview_fingerprint),
    width:primary.file_fingerprint.width_pixels,height:primary.file_fingerprint.height_pixels,
    rights,observed:{at:observation.observed_at},
  };
  return {workId,work,imageId,image};
}

/**
 * A media module and graft map with one observation added: its work and image
 * (replacing earlier entries of the same ids, as a newer observation does) and
 * a use of the image for `subject` in `role`. `artifact` is the Artifact's
 * path in the Tree, `<tradition>/<site>/<identity>`; `digest(text)` names the
 * representation after its service.
 */
export function addMedia({module,identities,observation,subject,role,artifact,view='whole',digest,bonsai}){
  if(!MEDIA_ROLES.includes(role))throw new Error(`role ${role}: one of ${MEDIA_ROLES.join(', ')}`);
  if(!ARTIFACT_PATH.test(artifact??''))throw new Error(`artifact ${artifact}: <tradition>/<site>/<identity> in lowercase words`);
  if(!text(subject))throw new Error('subject');
  const {workId,work,imageId,image}=mediaFromObservation(observation,{view});
  const next=structuredClone(module??{schema:MEDIA_SCHEMA,bonsai,works:{},images:{},uses:[]});
  next.works[workId]=work;next.images[imageId]=image;
  if(!next.uses.some(use=>use.subject===subject&&use.image===imageId&&use.role===role))next.uses.push({subject,image:imageId,role});
  const graft=structuredClone(identities??{});
  const keep=(section,id,tree,extra={})=>{
    const earlier=graft[section]?.[id];
    graft[section]={...(graft[section]??{}),[id]:{tree,...extra,status:earlier?.tree===tree?earlier.status:'to-mint'}};
  };
  keep('artifacts',workId,`tos.artifact.${artifact.replaceAll('/','.')}`,{path:artifact});
  keep('representations',imageId,image.api==='file'?`tos.file.sha256.${image.file.sha256}`:`tos.image-service.sha256.${digest(image.service)}`);
  return {module:next,identities:graft,workId,imageId,image};
}

/**
 * Read a media module: what a subject's card shows, in the order of the roles. `names` is the stand's
 * media names module (media-names.json), which each item carries for `workWords`.
 */
export function readMedia(module,names=null){
  const works=object(module?.works)?module.works:{},images=object(module?.images)?module.images:{},uses=Array.isArray(module?.uses)?module.uses:[];
  return {
    module,names,
    forSubject:subject=>uses
      .filter(use=>use.subject===subject&&images[use.image]&&works[images[use.image].work])
      .map(use=>({role:use.role,image:{id:use.image,...images[use.image]},work:{id:images[use.image].work,...works[images[use.image].work]},names}))
      .sort((a,b)=>MEDIA_ROLES.indexOf(a.role)-MEDIA_ROLES.indexOf(b.role)),
  };
}

// The provider's words of one part of a work, each in its language or else in the language its provider writes in.
function providerWords(work,part){
  const lines=part==='made'?work.made??[]:(work.titles??[]).filter(title=>title?.role===(part==='titles'?'title':'series'));
  return lines.filter(line=>text(line?.text)).map(line=>({text:line.text,language:line.language??PROVIDER_LANGUAGES[work.provider]??null}));
}

/**
 * A work's words for a reader of `locale`, part by part: `titles`, `series` and `made` as {language, lines},
 * `holder` as {language, text}; a part has none where neither the Tree nor the provider words it in the
 * reader's language. The reader's language is the one `translationLanguage` gives; in it the Tree's own name
 * stands where `names` (media-names.json) has one, else the provider's words. Nothing in another language is
 * shown, so a provider's catalogue title in its own language never reaches a reader of a different one.
 */
export function workWords(work,names,locale){
  const named=object(names?.works?.[work.id])?names.works[work.id]:{};
  const words={};
  for(const part of NAME_PARTS){
    const own=Object.fromEntries(Object.entries(named).filter(([,entry])=>Array.isArray(entry?.[part])&&entry[part].length).map(([language,entry])=>[language,entry[part]]));
    const provided=providerWords(work,part);
    const language=translationLanguage(locale,[...Object.keys(own),...provided.map(line=>line.language)].filter(Boolean));
    words[part]=language?{language,lines:own[language]??provided.filter(line=>line.language===language).map(line=>line.text)}:null;
  }
  const holders=object(names?.holders?.[work.holder])?names.holders[work.holder]:{},native=PROVIDER_LANGUAGES[work.provider]??null;
  const language=translationLanguage(locale,[...Object.keys(holders),native].filter(Boolean));
  words.holder=language?{language,text:holders[language]??work.holder}:null;
  return words;
}

/**
 * Where a stand's media names (media-names.json) depart from their contract or leave a work of `media`
 * unnamed in one of the languages the stands are written in; empty when they hold. `names` may be null.
 */
export function mediaNamesIssues(names,media){
  const issues=[],works=object(media?.works)?media.works:{};
  const languages=new Set(BONSAI_LOCALES),holders=new Set(Object.values(works).map(work=>work?.holder));
  if(names!==null&&names!==undefined){
    if(!object(names))return ['not an object'];
    if(names.schema!==MEDIA_NAMES_SCHEMA)issues.push('schema');
    if(names.bonsai!==media?.bonsai)issues.push('bonsai');
    if(Object.keys(names).some(key=>!['schema','bonsai','holders','works'].includes(key)))issues.push('unknown field');
    if(!object(names.holders))issues.push('holders');
    if(!object(names.works))issues.push('works');
    for(const [holder,named] of Object.entries(object(names.holders)?names.holders:{})){
      if(!holders.has(holder))issues.push(`holder ${holder}: holds no work of the media module`);
      if(!object(named)||!Object.keys(named).length||Object.entries(named).some(([language,value])=>!languages.has(language)||!text(value)))issues.push(`holder ${holder}: names`);
    }
    for(const [id,named] of Object.entries(object(names.works)?names.works:{})){
      if(!works[id])issues.push(`work ${id}: not a work of the media module`);
      if(!object(named)||!Object.keys(named).length){issues.push(`work ${id}: names`);continue;}
      for(const [language,entry] of Object.entries(named)){
        if(!languages.has(language))issues.push(`work ${id}: ${language} is not a language of the stands`);
        if(!object(entry)||!Object.keys(entry).length||Object.entries(entry).some(([part,lines])=>!NAME_PARTS.includes(part)||!Array.isArray(lines)||!lines.length||lines.some(line=>!text(line))))
          issues.push(`work ${id}: ${language} names`);
      }
    }
  }
  // Every work is named in each language of the stands: its title always, its series and makers where the provider gives them,
  // and its holder.
  const unnamed=new Set();
  for(const [id,work] of Object.entries(works)){
    if(!object(work))continue;
    for(const language of NAMED_LANGUAGES){
      const words=workWords({id,...work},names??null,language);
      for(const part of NAME_PARTS){
        if((part==='titles'||providerWords(work,part).length)&&words[part]?.language!==language)issues.push(`work ${id}: no ${part==='titles'?'title':part} in ${language}; name it in media-names.json`);
      }
      if(words.holder?.language!==language)unnamed.add(`holder ${work.holder}: no name in ${language}; name it in media-names.json`);
    }
  }
  return [...issues,...unnamed];
}
