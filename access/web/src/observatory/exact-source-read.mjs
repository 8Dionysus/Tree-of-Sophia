import {ContractError,RevisionError,RequestError,sameJson} from './knowledge-client.mjs';
import {t} from './ui-i18n.mjs';
import {withAbort} from './bounded-response.mjs';

export const SOURCE_READ_RESPONSE_BYTES=2*1024*1024;
export const SOURCE_READ_DEADLINE_MS=15000;
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$(?![\s\S])/.test(value);
const digest=value=>typeof value==='string'&&value.startsWith('sha256:')&&hash(value.slice(7));
const id=value=>typeof value==='string'&&value.length<=2048&&/^tos\.[a-z0-9]+(?:[.-][a-z0-9]+)*$(?![\s\S])/.test(value);
const keys=(value,names)=>object(value)&&Object.keys(value).length===names.length&&names.every(name=>Object.hasOwn(value,name));
const statuses=new Set(['available','missing','stale','corrupt','access-restricted','over-budget','unsupported']);
const requireContract=condition=>{if(!condition)throw new ContractError(t('Неверный ответ чтения источника.'));};
const readonly=value=>value.grants_current_use===false&&value.performs_assessment===false&&value.writes_to_source===false;
const byteSize=value=>new TextEncoder().encode(JSON.stringify(value)).byteLength;

export function validateExactSourceTarget(target){
  requireContract(object(target)&&['metadata_record','claim_record'].includes(target.layer));
  const metadata=target.layer==='metadata_record';
  requireContract(keys(target,metadata?['layer','record_type','record_ref','content_revision']:['layer','record_ref','content_revision']));
  const ref=target.record_ref;
  requireContract(keys(ref,['id','version','digest'])&&id(ref.id)&&Number.isSafeInteger(ref.version)&&ref.version>=1
    &&digest(ref.digest)&&target.content_revision===ref.digest&&byteSize(target)<=16384);
  requireContract(metadata?!ref.id.startsWith('tos.claim.'):ref.id.startsWith('tos.claim.'));
  if(metadata)requireContract(typeof target.record_type==='string'&&/^[a-z][a-z0-9-]{0,63}$(?![\s\S])/.test(target.record_type));
  return target;
}

function validateStatus(packet,schema,revision,target){
  requireContract(object(packet)&&packet.schema_version===schema&&typeof packet.status==='string'&&statuses.has(packet.status)
    &&typeof packet.reason==='string'&&packet.reason.length>0&&packet.reason.length<=256&&readonly(packet));
  if(packet.source_revision!==revision||packet.status==='stale')throw new RevisionError();
  requireContract(packet.content_revision===target.content_revision);
}

function validateHandle(handle,target,revision){
  requireContract(keys(handle,['schema_version','issuer','epoch','target','access','handle_digest'])
    &&handle.schema_version==='tos_source_read_handle_v1'&&handle.issuer==='Tree-of-Sophia/source-witnesses'
    &&sameJson(handle.target,target)&&digest(handle.handle_digest)&&byteSize(handle)<=16384);
  const epoch=handle.epoch,access=handle.access;
  requireContract(object(epoch)&&epoch.source_revision===revision&&hash(epoch.catalog_root_sha256)
    &&typeof epoch.catalog_namespace==='string'&&object(epoch.source_publication)
    &&epoch.source_publication.protocol==='tos_selected_source_metadata_v1'
    &&digest(epoch.source_publication.token)&&Number.isSafeInteger(epoch.source_publication.generation)&&epoch.source_publication.generation>=0);
  requireContract(object(access)&&access.scope===(target.layer==='claim_record'?'public-claim-record':'public-metadata-record')
    &&['public','public_metadata_only'].includes(access.visibility)&&access.visibility_verified===true
    &&access.rights_revalidated===false&&access.rights_scope==='metadata-disclosure-only'
    &&access.authority==='source-owner-public-metadata-contract');
}

// Source targets come only from exact backend inspection. Never reconstruct a
// path, strip a graph ID prefix or select a newer source when this card is stale.
// The owner verifies canonical source bytes; the browser checks the delivered
// binding and public-disclosure contract, not a second Python JSON serializer.
export async function readExactSource(client,selection,{signal,timeoutMs=SOURCE_READ_DEADLINE_MS}={}){
  requireContract(keys(selection,['kind','id','source_revision','content_revision'])&&['node','relation'].includes(selection.kind)
    &&typeof selection.id==='string'&&selection.id.length>0&&selection.id.length<=2048
    &&hash(selection.source_revision)&&hash(selection.content_revision));
  requireContract(Number.isSafeInteger(timeoutMs)&&timeoutMs>0&&timeoutMs<=SOURCE_READ_DEADLINE_MS);
  const expected=structuredClone(selection),controller=new AbortController();let timedOut=false;
  const abort=()=>controller.abort(signal?.reason);
  if(signal?.aborted)abort();else signal?.addEventListener('abort',abort,{once:true});
  const timer=setTimeout(()=>{timedOut=true;controller.abort();},timeoutMs);
  try{
    controller.signal.throwIfAborted();
    const {packet}=await withAbort(client.inspect(expected.kind,expected.id,controller.signal,expected.source_revision,expected.content_revision),controller.signal);
    controller.signal.throwIfAborted();
    if(packet.source_revision!==expected.source_revision)throw new RevisionError();
    const targets=packet.source_read_targets;
    requireContract(targets===undefined||object(targets));
    const selected=targets&&Object.hasOwn(targets,expected.id)?targets[expected.id]:null;
    if(!selected)return {status:'unsupported',reason:'no-exact-source-target',selection:expected,record:null};
    requireContract(keys(selected,['source_revision','target']));
    if(selected.source_revision!==expected.source_revision)throw new RevisionError();
    const target=structuredClone(validateExactSourceTarget(selected.target));
    const options={signal:controller.signal,maxResponseBytes:Math.min(SOURCE_READ_RESPONSE_BYTES,client.maxResponseBytes??SOURCE_READ_RESPONSE_BYTES)};
    const discovered=await withAbort(client.request('/api/source/handles',{...options,body:{target}}),controller.signal);
    controller.signal.throwIfAborted();
    validateStatus(discovered,'tos_source_handle_discovery_v1',expected.source_revision,target);
    requireContract(sameJson(discovered.target,target));
    if(discovered.status!=='available'){
      requireContract(discovered.handle===null);
      return {...discovered,selection:expected,record:null};
    }
    validateHandle(discovered.handle,target,expected.source_revision);
    const read=await withAbort(client.request('/api/source/read',{...options,body:{handle:discovered.handle,representation:'record'}}),controller.signal);
    controller.signal.throwIfAborted();
    validateStatus(read,'tos_source_read_result_v1',expected.source_revision,target);
    requireContract(sameJson(read.handle,discovered.handle)&&sameJson(read.record_ref,target.record_ref)&&read.layer===target.layer);
    if(read.status==='available'){
      requireContract(object(read.record)&&object(read.provenance)&&sameJson(read.access,discovered.handle.access)
        &&byteSize(read.record)<=1024*1024);
      const metadata=target.layer==='metadata_record';
      requireContract(read.record[metadata?'record_id':'claim_id']===target.record_ref.id
        &&read.record[metadata?'record_version':'claim_version']===target.record_ref.version);
      if(metadata)requireContract(read.record.record_type===target.record_type);
    }else requireContract(read.record===null);
    return {...read,selection:expected};
  }catch(error){
    if(timedOut)throw new RequestError(504,t('Чтение источника превысило время ожидания.'));
    throw error;
  }finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
}
