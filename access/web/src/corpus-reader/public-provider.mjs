import {bindCorpusProvider,CorpusProviderError,CorpusProviderCursorError,CorpusProviderUnsupportedScopeError,throwIfAborted} from './provider.mjs';
import {createReference,validateReference,referenceKey} from './model.mjs';
import {findInParagraphs} from '../reader/occurrences-rust.mjs';
import {readBoundedJSON,withAbort} from '../observatory/bounded-response.mjs';
import {corpusWorkTitle} from '../observatory/human-presentation.mjs';
import {editorialKindForMatch} from './editorial.mjs';

// A delivery adapter for an explicitly selected, immutable public export.
// Source identities, order, rights, wording and structure come from the native
// producer. Literal matching and reference rules remain with shared Rust/WASM.
const SCHEMA='tos_public_corpus_snapshot_v1';
const MANIFEST_BYTES=1024*1024, DOCUMENT_BYTES=4*1024*1024, INDEX_BYTES=8*1024*1024, MAPPING_BYTES=1024*1024;
const PAGE_BYTES=262144, DEADLINE_MS=15000;
const fail=(code,message)=>{throw new CorpusProviderError(code,message);};
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const digest=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const points=text=>Array.from(text);
const clone=value=>structuredClone(value);
// Search aliases are presentation metadata only; the manifest's author and
// every work identity remain untouched. Keep aliases for the public catalog's
// actual author names, not arbitrary transliteration of source strings.
const AUTHOR_ALIASES={
  aristotle:'Аристотель', cicero:'Цицерон Марк Туллий', 'diogenes laertius':'Диоген Лаэртский',
  epictetus:'Эпиктет', 'friedrich nietzsche':'Фридрих Ницше', 'hippocratic corpus':'Гиппократ Гиппократов корпус',
  lucretius:'Лукреций', 'marcus aurelius':'Марк Аврелий', plato:'Платон', plotinus:'Плотин',
  plutarch:'Плутарх', porphyry:'Порфирий', seneca:'Сенека',
};
const authorSearchAliases=author=>typeof author==='string' ? (AUTHOR_ALIASES[author.trim().toLowerCase()] || '') : '';

function cacheSet(cache,key,value,limit){
  cache.delete(key);cache.set(key,value);
  while(cache.size>limit)cache.delete(cache.keys().next().value);
  return value;
}
function cacheGet(cache,key){
  const value=cache.get(key);
  if(value!==undefined){cache.delete(key);cache.set(key,value);}
  return value;
}
function publicPath(path){
  if(typeof path!=='string'||!path.length||path.length>2048||path.startsWith('/')||path.includes('\\')||path.split('/').some(part=>!part||part==='.'||part==='..')||/[?#%\x00-\x20]/.test(path))
    fail('invalid_response','The public export contains an invalid member path.');
  return path;
}
function cursor(operation,revision,binding,position){
  return 'tos-public-1.'+encodeURIComponent(JSON.stringify({operation,revision,binding,position}));
}
function position(token,operation,revision,binding,initial=0){
  if(token===null||token===undefined)return initial;
  try{
    if(typeof token!=='string'||token.length>8192||!token.startsWith('tos-public-1.'))throw new Error();
    const value=JSON.parse(decodeURIComponent(token.slice(13)));
    if(value.operation!==operation||value.revision!==revision||JSON.stringify(value.binding)!==JSON.stringify(binding))throw new Error();
    if(typeof initial==='number'&&(!Number.isSafeInteger(value.position)||value.position<0))throw new Error();
    return value.position;
  }catch{throw new CorpusProviderCursorError('This public corpus cursor no longer matches the selected source or query.');}
}
function referenceFor(version,unit,start=0,end=points(unit.text??'').length){
  return createReference({source:version.source,versionId:version.id,unitId:unit.id??unit.unitId,start,end});
}
function documentView(document){
  return {id:document.id,title:document.title,author:document.author,status:'available',visibility:'public',
    unitCount:null,versions:document.versions.map(version=>{
      const {pages,units,structure,searchIndex,analysis,...metadata}=version;
      return {...metadata,status:'available',visibility:'public',available:true,textAvailable:true};
    })};
}

export function createPublicCorpusProvider({baseUrl='/static/public-corpus/',profile='zarathustra',fetch:fetcher=globalThis.fetch}={}){
  if(typeof fetcher!=='function')throw new TypeError('A public corpus fetch transport is required.');
  if(!['zarathustra','antiquity'].includes(profile))throw new TypeError('An explicit public corpus profile is required.');
  if(typeof baseUrl!=='string'||!baseUrl.startsWith('/')||baseUrl.startsWith('//')||!baseUrl.endsWith('/')||baseUrl.includes('..')||/[?#\\]/.test(baseUrl))throw new TypeError('The public corpus base must be a same-origin directory.');
  let manifest=null;
  const documents=new Map(),pages=new Map(),indices=new Map(),analyses=new Map(),mappings=new Map();

  async function read(path,descriptor,maximum,signal){
    throwIfAborted(signal);
    const timeout=AbortSignal.timeout(DEADLINE_MS),active=signal?AbortSignal.any([signal,timeout]):timeout;
    const response=await withAbort(fetcher(baseUrl+publicPath(path),{signal:active,credentials:'same-origin',redirect:'error',headers:{Accept:'application/json'}}),active);
    if(!response.ok)fail('provider_unavailable','The selected public corpus member is unavailable.');
    if(!/^application\/json\b/i.test(response.headers.get('Content-Type')??''))fail('invalid_response','The public corpus member is not JSON.');
    if(descriptor){
      if(!digest(descriptor.sha256)||!Number.isSafeInteger(descriptor.bytes)||descriptor.bytes<1||descriptor.bytes>maximum)fail('invalid_response','Invalid public corpus member descriptor.');
      const {packet,bytes}=await readBoundedJSON(response,descriptor.bytes,active,{includeBytes:true});
      if(bytes.byteLength!==descriptor.bytes)fail('invalid_response','The public corpus member size changed.');
      const actual=Array.from(new Uint8Array(await withAbort(crypto.subtle.digest('SHA-256',bytes),active)),value=>value.toString(16).padStart(2,'0')).join('');
      if(actual!==descriptor.sha256)fail('invalid_response','The public corpus member failed its source digest check.');
      return packet;
    }
    return readBoundedJSON(response,maximum,active);
  }
  async function describe({signal}={}){
    throwIfAborted(signal);
    if(!manifest){
      const value=await read('manifest.json',null,MANIFEST_BYTES,signal);
      if(value.schema_version!==SCHEMA||!digest(value.revision)||!object(value.profiles)||!Array.isArray(value.documents)||value.documents.length>1000)fail('invalid_response','Unsupported public corpus manifest.');
      const selected=value.profiles[profile];
      if(!object(selected)||!Array.isArray(selected.documentIds)||selected.documentIds.length>1000||new Set(selected.documentIds).size!==selected.documentIds.length)fail('invalid_response','The selected public profile is missing.');
      const ids=new Set();
      for(const entry of value.documents){
        if(!object(entry)||typeof entry.id!=='string'||!entry.id||typeof entry.title!=='string'||!entry.title||ids.has(entry.id)||!object(entry.manifest))fail('invalid_response','Invalid public document entry.');
        ids.add(entry.id);publicPath(entry.manifest.path);
      }
      if(selected.documentIds.some(id=>!ids.has(id)))fail('invalid_response','The public profile refers to an absent source.');
      manifest=value;
    }
    return clone(manifest);
  }
  async function documentData(id,signal,{outsideProfile=false}={}){
    await describe({signal});
    // Profiles choose the catalog, not access rights. A shared notebook can
    // reopen an exact public passage from either profile without substitution.
    const entry=manifest.documents.find(value=>value.id===id);
    if(!entry)fail('document-unavailable','The exact work is not in this public release.');
    let document=cacheGet(documents,id);
    if(!document){
      document=await read(entry.manifest.path,entry.manifest,DOCUMENT_BYTES,signal);
      if(document.id!==id||!Array.isArray(document.versions)||!document.versions.length||document.versions.length>100)fail('invalid_response','The public document manifest does not match its source.');
      const ids=new Set();
      for(const version of document.versions){
        if(!version.id||ids.has(version.id)||!digest(version.revision)||!object(version.source)||version.source.workId!==id||version.source.textLayerSha256!==version.revision||!Array.isArray(version.pages)||!Array.isArray(version.units)||version.units.length!==version.unitCount)fail('invalid_response','The public version has invalid source metadata.');
        ids.add(version.id);
        const unitIds=new Set();
        for(let i=0;i<version.units.length;i++){
          const unit=version.units[i];
          if(!unit.id||unitIds.has(unit.id)||unit.ordinal!==i+1||!Number.isSafeInteger(unit.page)||unit.page<0||unit.page>=version.pages.length)fail('invalid_response','Invalid public unit index.');
          unitIds.add(unit.id);
        }
      }
      cacheSet(documents,id,document,3);
    }
    return document;
  }
  async function versionData(args,{outsideProfile=false}={}){
    const document=await documentData(args.documentId,args.signal,{outsideProfile});
    const version=document.versions.find(value=>value.id===args.versionId);
    if(!version)fail('version-unavailable','The exact edition is not in this public release.');
    return {document,version};
  }
  async function pageData(document,version,index,signal){
    const descriptor=version.pages[index];
    if(!descriptor)fail('invalid_response','The source unit points to an absent page.');
    const key=descriptor.sha256;
    let value=cacheGet(pages,key);
    if(!value){
      value=await read(descriptor.path,descriptor,PAGE_BYTES,signal);
      if(value.documentId!==document.id||value.versionId!==version.id||value.revision!==version.revision||!Array.isArray(value.units)||value.units.length>40||value.units.length!==descriptor.count)fail('invalid_response','The public text page does not match its manifest.');
      for(const unit of value.units){
        const expected=version.units[unit.ordinal-1];
        if(!expected||expected.id!==unit.id||expected.page!==index||typeof unit.text!=='string'||points(unit.text).length>65536)fail('invalid_response','The public text unit does not match its source index.');
      }
      cacheSet(pages,key,value,4);
    }
    return value;
  }
  async function occurrenceMappings(document,version,reference,form,signal){
    const none={mappings:[],mappingStatus:'none'};
    if(!document.mappings)return none;
    const descriptor=document.mappings;
    const key=descriptor.sha256;
    let packet=cacheGet(mappings,key);
    if(!packet){
      packet=await read(descriptor.path,descriptor,MAPPING_BYTES,signal);
      if(packet.schema_version!=='tos_public_translation_mappings_v1'||packet.documentId!==document.id||!Array.isArray(packet.entries)||packet.entries.length>128)fail('invalid_response','The translation mapping carrier does not match the selected work.');
      cacheSet(mappings,key,packet,2);
    }
    const sourceKey=referenceKey(reference),result=[];
    for(const rawEntry of packet.entries){
      if(!object(rawEntry)||rawEntry.status!=='proposed'||rawEntry.reviewStatus!=='model_proposal_owner_review_pending')fail('invalid_response','The translation mapping is not a bounded proposal.');
      let left,right;
      try{left=validateReference(rawEntry.sourceReference);right=validateReference(rawEntry.targetReference);}
      catch{fail('invalid_response','The translation mapping contains an invalid exact reference.');}
      let entry;
      if(referenceKey(left)===sourceKey)entry={...rawEntry,navigationDirection:'source_to_target'};
      else if(referenceKey(right)===sourceKey){
        entry={...rawEntry,navigationDirection:'target_to_source'};
        for(const field of ['Reference','Quote','VersionId','VersionLabel','Language','Revision']){
          entry['source'+field]=rawEntry['target'+field];entry['target'+field]=rawEntry['source'+field];
        }
      }else continue;
      if(entry.sourceQuote!==form)continue;
      const source=validateReference(entry.sourceReference),target=validateReference(entry.targetReference);
      if(entry.sourceVersionId!==version.id||entry.sourceRevision!==version.revision||entry.sourceLanguage!==version.language||entry.targetVersionId!==target.versionId||entry.targetRevision!==target.target.textLayerSha256||target.target.workId!==document.id)fail('invalid_response','The translation mapping does not match its selected editions.');
      const targetResolved=await resolve({reference:target,signal});
      if(targetResolved.status!=='exact')continue;
      const targetVersion=document.versions.find(item=>item.id===target.versionId);
      if(!targetVersion||targetVersion.revision!==entry.targetRevision)continue;
      const targetUnit=targetVersion.units.find(item=>item.id===target.unitId);
      if(!targetUnit)continue;
      const targetPage=await pageData(document,targetVersion,targetUnit.page,signal);
      const targetText=targetPage.units.find(item=>item.id===target.unitId)?.text;
      const targetQuote=points(targetText??'').slice(target.selector.start,target.selector.end).join('');
      if(targetQuote!==entry.targetQuote)continue;
      result.push(entry);
    }
    return result.length?{mappings:result,mappingStatus:'available'}:none;
  }
  const citation=(version,unit)=>unit.reference??`${version.label} · ${unit.ordinal}`;
  async function window(args){
    const {document,version}=await versionData(args);
    const binding={documentId:document.id,versionId:version.id};
    let offset=position(args.cursor,'window',version.revision,binding);
    if(!args.cursor){
      if(args.revision&&args.revision!==version.revision)fail('stale_reference','The exact text layer changed.');
      if(args.reference?.schemaVersion){
        const resolved=await resolve({reference:args.reference,signal:args.signal});
        if(resolved.status!=='exact')fail('stale_reference','The saved passage belongs to a different text layer.');
      }
      const unitId=args.unitId??args.reference?.unitId;
      if(unitId){offset=version.units.findIndex(value=>value.id===unitId);if(offset<0)fail('stale_reference','The requested unit does not belong to this version.');}
      const revision=args.reference?.revision;
      if(revision&&revision!==version.revision)fail('stale_reference','The source revision changed.');
    }
    if(offset>=version.units.length)fail('stale_reference','This text window no longer exists.');
    const page=await pageData(document,version,version.units[offset].page,args.signal);
    const requestedLimit=Number.isSafeInteger(args.limit)&&args.limit>0?args.limit:40;
    const limit=Math.min(40,requestedLimit);
    const units=page.units.filter(unit=>unit.ordinal>offset).slice(0,limit).map(unit=>({...unit,...(unit.kind==='section'?{heading:false}:{}),label:unit.kind==='section'?citation(version,unit):String(unit.ordinal),language:version.language,reference:citation(version,unit)}));
    const next=units.at(-1)?.ordinal??offset;
    const pageIndex=version.units[offset].page;
    const pageStart=version.pages[pageIndex].firstOrdinal-1;
    const previous=offset>pageStart
      ?Math.max(pageStart,offset-limit)
      :pageIndex>0?Math.max(version.pages[pageIndex-1].firstOrdinal-1,offset-limit):null;
    return {documentId:document.id,versionId:version.id,revision:version.revision,units,total:version.unitCount,
      nextCursor:next<version.unitCount?cursor('window',version.revision,binding,next):null,
      previousCursor:offset>0?cursor('window',version.revision,binding,previous??0):null};
  }
  async function resolve({reference,signal}){
    const checked=validateReference(reference);
    if(checked.schemaVersion!=='tos.corpus.reader.reference.v1')return {status:'unavailable',reference:checked};
    const id=checked.target.workId;
    let document;
    try{document=await documentData(id,signal,{outsideProfile:true});}
    catch(error){if(['document-unavailable','version-unavailable'].includes(error?.code))return {status:'unavailable',reference:checked};throw error;}
    const version=document.versions.find(value=>value.id===checked.versionId);
    if(!version)return {status:'unavailable',reference:checked};
    const expected=referenceFor(version,{id:checked.unitId,text:''},checked.selector.start,checked.selector.end);
    if(referenceKey(expected)!==referenceKey(checked))return {status:'changed',reference:checked};
    const unit=version.units.find(value=>value.id===checked.unitId);
    if(!unit)return {status:'unavailable',reference:checked};
    const page=await pageData(document,version,unit.page,signal),text=page.units.find(value=>value.id===unit.id)?.text;
    if(typeof text!=='string'||checked.selector.end>points(text).length)return {status:'changed',reference:checked};
    return {status:'exact',reference:checked};
  }
  async function analyze({reference,form,language,signal}){
    const checked=validateReference(reference),dictionaryLinks=[];
    if(language==='de')dictionaryLinks.push({label:{ru:'Поиск формы в DWDS',en:'Search this form in DWDS'},url:'https://www.dwds.de/wb/'+encodeURIComponent(form)});
    if(['grc','lat'].includes(language))dictionaryLinks.push({label:{ru:'Поиск формы в Logeion',en:'Search this form in Logeion'},url:'https://logeion.uchicago.edu/'+encodeURIComponent(form)});
    const unavailable={status:'unavailable',reason:'unsupported_language',reference:checked,candidates:[],dictionaryLinks,mappings:[],mappingStatus:'none'};
    if((await resolve({reference:checked,signal})).status!=='exact')return {...unavailable,reason:'source_changed'};
    const {document,version}=await versionData({documentId:checked.target.workId,versionId:checked.versionId,signal});
    const meta=version.units.find(value=>value.id===checked.unitId),page=await pageData(document,version,meta.page,signal),unit=page.units.find(value=>value.id===meta.id);
    if(language!==version.language||points(unit.text).slice(checked.selector.start,checked.selector.end).join('')!==form)return {...unavailable,reason:'form_mismatch'};
    const mappingFields=await occurrenceMappings(document,version,checked,form,signal);
    if(version.analysis){
      const key=version.analysis.sha256;let data=cacheGet(analyses,key);
      if(!data){data=await read(version.analysis.path,version.analysis,INDEX_BYTES,signal);if(data.schema_version!=='tos_public_morphology_candidates_v1'||data.textLayerSha256!==version.revision||data.language!==language||!object(data.forms))fail('invalid_response','The analysis carrier does not match the source.');cacheSet(analyses,key,data,2);}
      const result=Object.hasOwn(data.forms,form)?data.forms[form]:null;
      const candidates=(result?.candidates??[]).map(candidate=>{
        if(candidate?.dictionaryUrl||candidate?.dictionary?.url||candidate?.articleUrl)return candidate;
        const lemma=typeof candidate?.lemma==='string'?candidate.lemma:candidate?.lemma?.[language]??candidate?.lemma?.en??candidate?.lemma?.ru;
        const dictionaryUrl=typeof lemma==='string'&&lemma?language==='de'?'https://www.dwds.de/wb/'+encodeURIComponent(lemma):['grc','lat'].includes(language)?'https://logeion.uchicago.edu/'+encodeURIComponent(lemma):null:null;
        return dictionaryUrl?{...candidate,dictionaryUrl}:candidate;
      });
      return {...unavailable,status:candidates.length?'available':'unavailable',reason:candidates.length?null:result?.failure?'provider_error':(result||data.unknownForms?.includes(form))?'unrecognized':'not_processed',analysisVersion:data.analysisVersion,source:data.source,...(result??{}),candidates,...mappingFields};
    }
    return {...unavailable,...mappingFields};
  }
  async function search(args){
    if(!['document','version'].includes(args.scope))throw new CorpusProviderUnsupportedScopeError(args.scope);
    const {document,version}=await versionData(args),binding={documentId:document.id,versionId:version.id,query:args.query,scope:args.scope};
    // A word-form index is a compact source-derived carrier, not source text.
    // Prove the literal query fits inside an indexed form before using the
    // optimization. Otherwise scan a bounded page without duplicating the
    // publisher's delimiter rules (including punctuation and spelling variants).
    let index=null;
    if(version.searchIndex&&!/\s/.test(args.query)){
      const key=version.searchIndex.sha256;
      index=cacheGet(indices,key);
      if(!index){index=await read(version.searchIndex.path,version.searchIndex,INDEX_BYTES,args.signal);if(!Array.isArray(index.terms))fail('invalid_response','Invalid public surface index.');cacheSet(indices,key,index,2);}
    }
    if(index&&index.terms.some(term=>typeof term?.form==='string'&&term.form.includes(args.query))){
      const saved=position(args.cursor,'search',version.revision,binding,{term:0,match:0,occurrence:0});
      if(!object(saved)||![saved.term,saved.match,saved.occurrence].every(value=>Number.isSafeInteger(value)&&value>=0)||saved.term>index.terms.length)throw new CorpusProviderCursorError();
      const matched=findInParagraphs(index.terms.slice(saved.term).map(value=>value.form),args.query,{limit:500});
      const items=[];
      // A small page of index hits bounds the source-page reads needed for
      // meaningful context. The continuation retains the exact index cursor.
      const pageLimit=Number.isSafeInteger(args.limit)?Math.max(1,Math.min(args.limit,8)):8;
      const finish=async nextCursor=>{
        const pageNumbers=[...new Set(items.map(item=>item.page))];
        const sourcePages=await Promise.all(pageNumbers.map(index=>pageData(document,version,index,args.signal)));
        const byPage=new Map(pageNumbers.map((index,i)=>[index,sourcePages[i]]));
        for(const item of items){
          const unit=byPage.get(item.page)?.units.find(value=>value.id===item.unitId);
          if(!unit || item.end>points(unit.text).length)fail('invalid_response','The surface index refers to an absent source range.');
          item.reference=citation(version,unit);
          item.excerpt=points(unit.text).slice(Math.max(0,item.start-20),item.end+60).join('');
          item.sourceLayer=editorialKindForMatch(version,unit,item.start,item.end);
          item.ordinal=unit.ordinal;
          delete item.page;
        }
        return {items,total:null,nextCursor};
      };
      for(const [matchIndex,match] of matched.matches.entries()){
        const termIndex=saved.term+match.paragraph,term=index.terms[termIndex];
        if(termIndex===saved.term&&match.index<saved.match)continue;
        const first=termIndex===saved.term&&match.index===saved.match?saved.occurrence:0;
        for(let i=first;i<term.occurrences.length;i++){
          const occurrence=term.occurrences[i],ordinal=Array.isArray(occurrence)?occurrence[0]:occurrence.ordinal,offset=Array.isArray(occurrence)?occurrence[1]:occurrence.start,meta=version.units[ordinal-1];
          if(!meta||!Number.isSafeInteger(offset)||offset<0||(!Array.isArray(occurrence)&&meta.id!==occurrence.unitId))fail('invalid_response','The surface index refers to a different unit.');
          const start=offset+points(term.form.slice(0,match.index)).length;
          const end=start+points(term.form.slice(match.index,match.index+match.length)).length;
          items.push({unitId:meta.id,page:meta.page,reference:occurrence.reference??citation(version,meta),
            start,end,exactReference:referenceFor(version,meta,start,end)});
          if(items.length===pageLimit){
            const more=i+1<term.occurrences.length || matchIndex+1<matched.matches.length || matched.truncated;
            return finish(more?cursor('search',version.revision,binding,{term:termIndex,match:match.index,occurrence:i+1}):null);
          }
        }
      }
      const last=matched.matches.at(-1);
      return finish(matched.truncated&&last?cursor('search',version.revision,binding,{term:saved.term+last.paragraph,match:last.index+last.length,occurrence:0}):null);
    }
    const saved=position(args.cursor,'search-page',version.revision,binding,{page:0,unit:0,index:0});
    if(!object(saved)||![saved.page,saved.unit,saved.index].every(value=>Number.isSafeInteger(value)&&value>=0))throw new CorpusProviderCursorError();
    const pageIndex=saved.page;
    if(pageIndex>=version.pages.length)return {items:[],total:null,nextCursor:null};
    const page=await pageData(document,version,pageIndex,args.signal);
    if(saved.unit>page.units.length||saved.index>(page.units[saved.unit]?.text.length??0))throw new CorpusProviderCursorError();
    const searchable=page.units.slice(saved.unit).map((value,index)=>index===0?value.text.slice(saved.index):value.text);
    const matches=findInParagraphs(searchable,args.query,{limit:args.limit});
    const items=matches.matches.map(match=>{
      const unit=page.units[saved.unit+match.paragraph],index=match.index+(match.paragraph===0?saved.index:0),start=points(unit.text.slice(0,index)).length,end=start+points(unit.text.slice(index,index+match.length)).length;
      return {unitId:unit.id,ordinal:unit.ordinal,reference:citation(version,unit),excerpt:points(unit.text).slice(Math.max(0,start-20),end+60).join(''),start,end,
        sourceLayer:editorialKindForMatch(version,unit,start,end),exactReference:referenceFor(version,unit,start,end)};
    });
    const last=matches.matches.at(-1),next=matches.truncated&&last
      ?{page:pageIndex,unit:saved.unit+last.paragraph,index:last.index+last.length+(last.paragraph===0?saved.index:0)}
      :pageIndex+1<version.pages.length?{page:pageIndex+1,unit:0,index:0}:null;
    return {items,total:null,nextCursor:next?cursor('search-page',version.revision,binding,next):null};
  }
  const provider={capabilities:{structure:true,searchScopes:['document','version']},
    async catalog(args){
      await describe({signal:args.signal});
      const selected=new Set(manifest.profiles[profile].documentIds);
      let items=manifest.documents.filter(value=>selected.has(value.id));
      if(args.query){const found=new Set(findInParagraphs(items.map(value=>`${value.title} ${corpusWorkTitle(value,'ru')} ${corpusWorkTitle(value,'en')} ${value.author??''} ${authorSearchAliases(value.author)}`),args.query,{limit:500}).matches.map(value=>value.paragraph));items=items.filter((_,index)=>found.has(index));}
      const binding={profile,query:args.query},offset=position(args.cursor,'catalog',manifest.revision,binding);
      return {items:items.slice(offset,offset+args.limit).map(({id,title,author})=>({id,title,author,status:'available',visibility:'public',textAvailable:true,unitCount:null})),total:items.length,
        nextCursor:offset+args.limit<items.length?cursor('catalog',manifest.revision,binding,offset+args.limit):null};
    },
    async document(args){return documentView(await documentData(args.documentId,args.signal));},
    window,search,resolve,analyze,
    async structure(args){
      const {document,version}=await versionData(args),binding={documentId:document.id,versionId:version.id};
      let values=version.structure??[];
      // The selected German source has this chapter heading as an existing
      // unit, omitted by its generated contents. This is navigation metadata,
      // not a replacement anchor or a change to the source text.
      if(version.id==='gutenberg-7205-de' && version.revision==='4f71adfeeb39e64055acc2fc3f58964cf39f03d27bf2264d25313bc45d8658ec'){
        const unitId='tos.text-unit.sid-06ad3f6ff0154946aba2f52258b13be9';
        if(version.units.some(unit=>unit.id===unitId) && !values.some(item=>item.unitId===unitId)){
          const order=new Map(version.units.map(unit=>[unit.id,unit.ordinal]));
          values=[...values,{id:'presentation-seven-seals',unitId,label:'Die sieben Siegel (Oder: das Ja- und Amen-Lied)',level:1,parentId:null}]
            .sort((a,b)=>(order.get(a.unitId)??Infinity)-(order.get(b.unitId)??Infinity));
        }
      }
      const offset=position(args.cursor,'structure',version.revision,binding);
      return {documentId:document.id,versionId:version.id,revision:version.revision,items:values.slice(offset,offset+args.limit),total:values.length,
        nextCursor:offset+args.limit<values.length?cursor('structure',version.revision,binding,offset+args.limit):null};
    },
  };
  return Object.freeze({...bindCorpusProvider(provider),describe});
}
