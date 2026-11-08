// A bonsai's media: images of works that a stand shows on its cards.
//
// The module keeps three levels, as the Tree does. A work is the object its
// holder keeps (an engraving, a painting, a manuscript leaf), with the
// provider's own titles, makers, dating, size and record. An image is one
// reproduction of a work served by an image service, with its pixel size and
// the rights statement the provider gives for it. A use binds an image to a
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

import {MEDIA_HOSTS,MEDIA_ROLES} from './media-hosts.mjs';
import {dateIssues,yearLexeme} from './time.mjs';

export {MEDIA_HOSTS,MEDIA_ROLES};
export const MEDIA_SCHEMA='tos.bonsai.media.v1';
/** The kinds of title a provider gives: the preferred one, the one written on the object, the series. */
export const TITLE_ROLES=Object.freeze(['title','inscribed','series']);
/** The image APIs a card reads. */
export const IMAGE_APIS=Object.freeze(['iiif-image-3']);
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
const REPRESENTATION=/^tos\.image-service\.sha256\.[0-9a-f]{64}$/;
const STATUSES=Object.freeze(['to-mint','resolved']);

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const text=value=>typeof value==='string'&&/\S/.test(value);
const https=value=>typeof value==='string'&&/^https:\/\/\S+$/.test(value);
const uri=value=>typeof value==='string'&&/^https?:\/\/\S+$/.test(value);
const sha=value=>typeof value==='string'&&/^[0-9a-f]{64}$/.test(value);
const instant=value=>typeof value==='string'&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value);
const worded=value=>object(value)&&text(value.text)&&(value.language===null||text(value.language))&&Object.keys(value).every(key=>key==='text'||key==='language');
const hostOf=value=>{try{return new URL(value).host;}catch{return null;}};

/** Whether a rights entry lets a stand show the image. */
export const openRights=rights=>object(rights)&&(OPEN_STATEMENTS.includes(rights.statement)||rights.statement===null&&rights.field==='isPublicDomain'&&rights.value==='true');

/**
 * Where a media module departs from its own contract; empty when it holds.
 * `subjects` (a Set) are the stand's subject ids a use may name; `identities`
 * is the graft map, which must name a Tree identity for every work and image;
 * `digest(text)` (hex SHA-256) checks that a representation is named after
 * its image service, as the Tree names it.
 */
export function mediaIssues(module,{subjects=null,identities=null,digest=null}={}){
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
    if(!https(image.service))issues.push(`${at}: service`);
    else if(!MEDIA_HOSTS.includes(hostOf(image.service)))issues.push(`${at}: ${hostOf(image.service)} is not a media host; add it to bonsai/lib/media-hosts.mjs`);
    if(/\/$/.test(image.service??''))issues.push(`${at}: service ends with a slash`);
    for(const key of ['width','height'])if(!Number.isInteger(image[key])||image[key]<=0)issues.push(`${at}: ${key}`);
    const rights=image.rights;
    if(!object(rights)||!(rights.statement===null||https(rights.statement))||!text(rights.field)||!text(rights.value)||!(rights.label===null||text(rights.label))||!https(rights.stated_in))issues.push(`${at}: rights`);
    else if(!openRights(rights))issues.push(`${at}: its rights do not let a stand show it`);
    if(!object(image.observed)||!instant(image.observed.at)||!sha(image.observed.info_sha256))issues.push(`${at}: observed`);
    if(identities){
      const graft=identities.representations?.[id];
      if(!object(graft)||!REPRESENTATION.test(graft.tree??'')||!STATUSES.includes(graft.status))issues.push(`${at}: no Tree representation in the graft map`);
      else if(digest&&https(image.service)&&graft.tree!==`tos.image-service.sha256.${digest(image.service)}`)issues.push(`${at}: the graft map's representation is not named after its service`);
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
  const {statement=null,begin_year:begin=null,end_year:end=null}=dating??{};
  if(!text(statement)||!Number.isInteger(begin)&&!Number.isInteger(end))return null;
  const certainty=/\b(?:c|ca|circa)\b\.?|\?/i.test(statement)?'approximate':'exact';
  const base={role:'historical-time',calendar:null,year_numbering:'astronomical',certainty,source_wording:{text:statement,language:null}};
  if(begin===end)return {kind:'date-assertion',...base,value:yearLexeme(begin)};
  return {kind:'interval-assertion',...base,interval:{...(Number.isInteger(begin)?{start:yearLexeme(begin)}:{}),...(Number.isInteger(end)?{end:yearLexeme(end)}:{})}};
}

function sizeOf(dimensions){
  const value=kind=>{const found=(dimensions??[]).find(item=>item?.kind===kind&&item.unit_uri===MILLIMETRES);const number=Number(found?.value);return Number.isFinite(number)&&number>0?number:null;};
  const height=value('height'),width=value('width');
  return height&&width?[height,width]:null;
}

/**
 * The work and image entries of one provider observation (`media.observe`):
 * the work keyed `<provider>:<inventory>`, its primary image `<work>/<view>`.
 * Refuses an observation without an image service or whose image rights do
 * not let a stand show it.
 */
export function mediaFromObservation(observation,{view='whole'}={}){
  if(observation?.schema_version!=='tos_media_provider_observation_v1')throw new Error('not a media provider observation');
  const record=observation.object??{},provider=observation.provider;
  const inventory=(record.identifiers??[]).find(item=>/object number|accession/i.test(item?.scheme??''))?.value??observation.requested_object;
  const service=(observation.images??[]).find(item=>item?.role==='primary'&&item.form==='image_service')?.image_service;
  if(!service)throw new Error(`${provider} ${inventory}: the observation has no image service; a card reads IIIF Image API 3 services only`);
  if(service.api_version!=='3')throw new Error(`${provider} ${inventory}: IIIF Image API ${service.api_version} is not read by the cards`);
  const right=(observation.rights??[]).find(item=>item?.scope==='image');
  const rights=right?{statement:right.statement_uri??null,field:right.provider_field,value:right.value,label:right.label??null,stated_in:right.stated_in}:null;
  if(!openRights(rights))throw new Error(`${provider} ${inventory}: the provider's image rights do not let a stand show it`);
  const requestOf=role=>(observation.requests??[]).find(item=>item?.role===role);
  const workId=`${provider}:${inventory}`,imageId=`${workId}/${view}`;
  const titles=(record.titles??[]).map(title=>({text:title.text,language:title.language??null,role:TITLE_ROLE_URIS[title.role]??null})).filter(title=>title.role&&text(title.text));
  const work={
    provider,record:record.record_url,holder:record.repository,inventory,
    titles,made:statements(record.production_statements),dating:datingOf(record.dating),
    classification:record.classifications??[],materials:record.materials??[],size_mm:sizeOf(record.dimensions),
    credit:statements(record.credit_lines),
    depicts:[...new Set((record.depicted??[]).flatMap(item=>item?.authority_uris??[]))],
    observed:{at:observation.observed_at,record_sha256:requestOf('object-record')?.sha256},
  };
  const image={
    work:workId,api:'iiif-image-3',service:service.service_id.replace(/\/$/,''),width:service.width_pixels,height:service.height_pixels,
    rights,observed:{at:service.info_fingerprint?.checked_at??observation.observed_at,info_sha256:service.info_fingerprint?.sha256},
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
  keep('representations',imageId,`tos.image-service.sha256.${digest(image.service)}`);
  return {module:next,identities:graft,workId,imageId};
}

/** Read a media module: what a subject's card shows, in the order of the roles. */
export function readMedia(module){
  const works=object(module?.works)?module.works:{},images=object(module?.images)?module.images:{},uses=Array.isArray(module?.uses)?module.uses:[];
  return {
    module,
    forSubject:subject=>uses
      .filter(use=>use.subject===subject&&images[use.image]&&works[images[use.image].work])
      .map(use=>({role:use.role,image:{id:use.image,...images[use.image]},work:{id:images[use.image].work,...works[images[use.image].work]}}))
      .sort((a,b)=>MEDIA_ROLES.indexOf(a.role)-MEDIA_ROLES.indexOf(b.role)),
  };
}
