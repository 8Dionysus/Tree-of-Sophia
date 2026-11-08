import {createPublicCorpusProvider} from './public-provider.mjs';
import {bindCorpusProvider,CorpusProviderError} from './provider.mjs';
import {readBoundedJSON,withAbort} from '../observatory/bounded-response.mjs';
import {validateReference} from './model.mjs';

// Resolve an exact source address across the current and explicitly retained
// immutable manifests. Each candidate remains an ordinary public provider;
// its manifest and every subsequent member are verified before use.
export function createRetainedPublicCorpusProvider({baseUrl='/static/public-corpus/',profile='zarathustra',fetch:fetcher=globalThis.fetch}={}){
  const options={baseUrl,profile,fetch:fetcher};
  const current=createPublicCorpusProvider(options);
  let retainedPromise=null;
  const retainedProviders=new Map();
  const unavailable=()=>new CorpusProviderError('retained-version-unavailable','The exact saved text version is not available in the current or retained public snapshots.');
  async function retained(signal){
    if(!retainedPromise){
      retainedPromise=(async()=>{
        const active=AbortSignal.timeout(15000);
        const response=await withAbort(fetcher(baseUrl+'retained.json',{signal:active,credentials:'same-origin',redirect:'error',headers:{Accept:'application/json'}}),active);
        if(response.status===404)return [];
        if(!response.ok)throw new CorpusProviderError('provider_unavailable','The retained snapshot index is unavailable.');
        if(!/^application\/json\b/i.test(response.headers.get('Content-Type')??''))throw new CorpusProviderError('invalid_response','The retained snapshot index is not JSON.');
        const value=await readBoundedJSON(response,1024*1024,active);
        if(!value||!Array.isArray(value.manifests)||value.manifests.length>256)throw new CorpusProviderError('invalid_response','Invalid retained snapshot index.');
        const seen=new Set();
        return value.manifests.map(item=>{
          if(item?.schema!=='tos_public_corpus_retained_manifest_v1'||!/^[a-f0-9]{64}$/.test(item.revision??'')||!/^[a-f0-9]{64}$/.test(item.sha256??'')||item.path!==`revisions/${item.revision}.json`||!Number.isSafeInteger(item.bytes)||item.bytes<1||item.bytes>1024*1024||seen.has(item.revision))throw new CorpusProviderError('invalid_response','Invalid retained snapshot descriptor.');
          seen.add(item.revision);return item;
        });
      })().catch(error=>{retainedPromise=null;throw error;});
    }
    return signal?withAbort(retainedPromise,signal):retainedPromise;
  }
  async function snapshot(descriptor,signal){
    let candidate=retainedProviders.get(descriptor.sha256);
    if(!candidate){candidate=createPublicCorpusProvider({...options,manifestPath:descriptor.path,expectedManifestSha256:descriptor.sha256,expectedManifestBytes:descriptor.bytes});retainedProviders.set(descriptor.sha256,candidate);}
    const manifest=await candidate.describe({signal});
    if(manifest.revision!==descriptor.revision)throw new CorpusProviderError('invalid_response','The retained manifest revision does not match its descriptor.');
    return candidate;
  }
  async function matches(provider,{documentId,versionId,revision,unitId,reference,signal}){
    let document;
    try{document=await provider.document({documentId,signal});}
    catch(error){if(['document-unavailable','version-unavailable'].includes(error?.code))return false;throw error;}
    const version=document.versions.find(item=>item.id===versionId);
    if(!version||(revision&&version.revision!==revision))return false;
    if(reference)return (await provider.resolve({reference,signal})).status==='exact';
    if(unitId){
      try{await provider.window({documentId,versionId,unitId,revision,limit:1,signal});}
      catch(error){if(['stale_reference','version-unavailable','document-unavailable'].includes(error?.code))return false;throw error;}
    }
    return true;
  }
  async function select(args){
    const reference=args.reference?.schemaVersion==='tos.corpus.reader.reference.v1'?validateReference(args.reference):null;
    const documentId=reference?.target.workId??args.documentId,versionId=reference?.versionId??args.versionId;
    const revision=reference?.target.textLayerSha256??args.revision,unitId=reference?.unitId??args.unitId;
    if(args.snapshotRevision){
      if(!/^[a-f0-9]{64}$/.test(args.snapshotRevision))throw new CorpusProviderError('invalid_request','Invalid public snapshot revision.');
      const manifest=await current.describe({signal:args.signal});
      let candidate=current;
      if(manifest.revision!==args.snapshotRevision){
        const descriptor=(await retained(args.signal)).find(item=>item.revision===args.snapshotRevision);
        if(!descriptor)throw unavailable();
        candidate=await snapshot(descriptor,args.signal);
      }
      if(documentId&&versionId&&revision&&!await matches(candidate,{documentId,versionId,revision,unitId,reference,signal:args.signal}))throw unavailable();
      return candidate;
    }
    if(!documentId||!versionId||!revision)return current;
    const target={documentId,versionId,revision,unitId,reference,signal:args.signal};
    if(await matches(current,target))return current;
    for(const descriptor of await retained(args.signal)){
      const candidate=await snapshot(descriptor,args.signal);
      if(await matches(candidate,target))return candidate;
    }
    throw unavailable();
  }
  const provider={capabilities:current.capabilities,
    catalog:args=>current.catalog(args),
    document:async args=>{
      const candidate=await select(args),document=await candidate.document(args),manifest=await candidate.describe({signal:args.signal});
      return {...document,versions:document.versions.map(version=>({...version,snapshotRevision:manifest.revision,retainedSnapshot:candidate!==current}))};
    },
    window:async args=>(await select(args)).window(args),
    search:async args=>(await select(args)).search(args),
    structure:async args=>(await select(args)).structure(args),
    resolve:async args=>{
      try{return await (await select(args)).resolve(args);}
      catch(error){if(error?.code==='retained-version-unavailable')return {status:'unavailable',reference:args.reference};throw error;}
    },
    analyze:async args=>(await select(args)).analyze(args),
    // Where a place of one edition stands in another: in the snapshot that holds both editions, the current one first.
    place:async args=>{
      const holds=async candidate=>{
        try{const ids=new Set((await candidate.document({documentId:args.documentId,signal:args.signal})).versions.map(version=>version.id));return ids.has(args.versionId)&&ids.has(args.targetVersionId);}
        catch{return false;}
      };
      if(await holds(current))return current.place?.(args)??null;
      for(const descriptor of await retained(args.signal)){
        const candidate=await snapshot(descriptor,args.signal);
        if(await holds(candidate))return candidate.place?.(args)??null;
      }
      throw unavailable();
    },
  };
  return Object.freeze({...bindCorpusProvider(provider),describe:args=>current.describe(args)});
}
