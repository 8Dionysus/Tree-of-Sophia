import {ContractError,RevisionError,RequestError,sameJson} from './knowledge-client.mjs';
import {t} from './ui-i18n.mjs';
import {withAbort} from './bounded-response.mjs';

export const SOURCE_READ_RESPONSE_BYTES=2*1024*1024;
export const SOURCE_READ_DEADLINE_MS=15000;
// Keep the complete validated delivery for deliberate inspection, excluding
// the transient handle used to request it. Never mutate the live result.
export function sourceReadExport(read){
  if(read===null||typeof read!=='object'||Array.isArray(read))throw new TypeError('An exact source read envelope is required.');
  const value=structuredClone(read);delete value.handle;return value;
}
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$(?![\s\S])/.test(value);
const digest=value=>typeof value==='string'&&value.startsWith('sha256:')&&hash(value.slice(7));
const id=value=>typeof value==='string'&&value.length<=2048&&/^tos\.[a-z0-9]+(?:[.-][a-z0-9]+)*$(?![\s\S])/.test(value);
const keys=(value,names)=>object(value)&&Object.keys(value).length===names.length&&names.every(name=>Object.hasOwn(value,name));
const statuses=new Set(['available','missing','stale','corrupt','access-restricted','over-budget','unsupported']);
const requireContract=condition=>{if(!condition)throw new ContractError(t('Неверный ответ чтения источника.'));};
const readonly=value=>value.grants_current_use===false&&value.performs_assessment===false&&value.writes_to_source===false;
const byteSize=value=>new TextEncoder().encode(JSON.stringify(value)).byteLength;

// Discovery only advertises transport choices. Each exact read still checks
// the selected record, current rights and (for local text) owner conditions.
async function sourceReadCapabilities(client,revision,signal){
  const packet=await client.request('/api/source/capabilities',{signal,maxResponseBytes:Math.min(65536,client.maxResponseBytes??65536)});
  requireContract(object(packet)&&packet.schema_version==='tos_source_read_capabilities_v1'
    &&typeof packet.available==='boolean');
  if(!packet.available)return packet;
  if(packet.source_epoch?.source_revision!==revision)throw new RevisionError();
  requireContract(packet.authority?.writes_to_source===false&&packet.authority?.grants_current_use===false);
  return packet;
}

export async function exactSourceRepresentations(client,revision,signal){
  const packet=await sourceReadCapabilities(client,revision,signal);
  if(!packet.available)return [];
  requireContract(Array.isArray(packet.representations)&&packet.representations.every(value=>typeof value==='string'));
  return packet.representations.filter(value=>['native_public_unit','native_local_unit'].includes(value));
}

function requireRepresentableNumbers(packet){
  // JSON.parse rounds integers outside JavaScript's safe range, including in
  // unknown extensions. Never present that decoded value as an exact record.
  // This conservative display limit does not change the owner source or grant
  // the browser authority to rewrite a number as a string.
  const pending=[packet];
  while(pending.length){
    const value=pending.pop();
    if(typeof value==='number'&&(!Number.isFinite(value)||(Number.isInteger(value)&&!Number.isSafeInteger(value))))
      throw new ContractError(t('Число в исходной записи нельзя показать без риска потери точности.'));
    if(value!==null&&typeof value==='object')for(const child of Object.values(value))pending.push(child);
  }
}

export function validateExactSourceTarget(target){
  requireContract(object(target)&&['metadata_record','claim_record','authored_csv_record'].includes(target.layer));
  if(target.layer==='authored_csv_record'){
    requireContract(keys(target,['layer','pack_id','edge_id','source_row','source_file_sha256','content_revision'])
      &&typeof target.pack_id==='string'&&target.pack_id.isWellFormed()&&new TextEncoder().encode(target.pack_id).length<=2048
      &&/^(canon\/relations\/|candidate-intake\/)/.test(target.pack_id)
      &&!target.pack_id.split('/').some(part=>!part||part.startsWith('.')||part==='payload')
      &&!/[\\\u0000]/.test(target.pack_id)&&typeof target.edge_id==='string'&&target.edge_id.length>0
      &&target.edge_id.isWellFormed()&&new TextEncoder().encode(target.edge_id).length<=2048&&!target.edge_id.includes('\0')
      &&Number.isSafeInteger(target.source_row)&&target.source_row>=1&&hash(target.source_file_sha256)
      &&digest(target.content_revision)&&byteSize(target)<=16384);
    return target;
  }
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
    &&handle.schema_version==='tos_source_read_handle_v1'
    &&handle.issuer===(target.layer==='authored_csv_record'?'Tree-of-Sophia/authored-corpus':'Tree-of-Sophia/source-witnesses')
    &&sameJson(handle.target,target)&&digest(handle.handle_digest)&&byteSize(handle)<=16384);
  const epoch=handle.epoch,access=handle.access;
  requireContract(object(epoch)&&epoch.source_revision===revision&&hash(epoch.catalog_root_sha256)
    &&typeof epoch.catalog_namespace==='string'&&object(epoch.source_publication)
    &&epoch.source_publication.protocol==='tos_selected_source_metadata_v1'
    &&digest(epoch.source_publication.token)&&Number.isSafeInteger(epoch.source_publication.generation)&&epoch.source_publication.generation>=0);
  requireContract(object(access)&&access.scope===({claim_record:'public-claim-record',metadata_record:'public-metadata-record',authored_csv_record:'public-authored-csv-record'})[target.layer]
    &&['public','public_metadata_only'].includes(access.visibility)&&access.visibility_verified===true
    &&access.rights_revalidated===false&&access.rights_scope==='metadata-disclosure-only'
    &&access.authority==='source-owner-public-metadata-contract');
}

// Source targets come only from exact backend inspection. Never reconstruct a
// path, strip a graph ID prefix or select a newer source when this card is stale.
// The owner verifies canonical source bytes; the browser checks the delivered
// binding and public-disclosure contract, not a second Python JSON serializer.
export async function readExactSource(client,selection,{signal,timeoutMs=SOURCE_READ_DEADLINE_MS,representation='record'}={}){
  requireContract(['record','native_public_unit','native_local_unit'].includes(representation));
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
    const capabilities=await withAbort(sourceReadCapabilities(client,expected.source_revision,controller.signal),controller.signal);
    if(!capabilities.available)return {status:'unsupported',reason:'source-owner-reader-not-configured',selection:expected,record:null};
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
    requireContract(sameJson(read.handle,discovered.handle)&&sameJson(read.record_ref,target.record_ref??null)&&read.layer===target.layer);
    if(read.status==='available'){
      requireRepresentableNumbers(read);
      requireContract(object(read.record)&&object(read.provenance)&&sameJson(read.access,discovered.handle.access)
        &&byteSize(read.record)<=1024*1024);
      if(target.layer==='authored_csv_record'){
        requireContract(Object.values(read.record).every(value=>value===null||typeof value==='string')
          &&read.record_kind==='authored_csv'&&read.provenance.source_row===target.source_row
          &&read.provenance.source_file_sha256===target.source_file_sha256);
        // CSV cells have only string/null values. Serialize sorted code-point
        // keys directly, preserving numeric-looking keys and unknown columns.
        const compare=(a,b)=>{const x=Array.from(a),y=Array.from(b);for(let i=0;i<Math.min(x.length,y.length);i++){
          const diff=x[i].codePointAt(0)-y[i].codePointAt(0);if(diff)return diff;}return x.length-y.length;};
        const exact='{'+Object.keys(read.record).sort(compare).map(key=>JSON.stringify(key)+':'+JSON.stringify(read.record[key])).join(',')+'}';
        const raw=await withAbort(crypto.subtle.digest('SHA-256',new TextEncoder().encode(exact)),controller.signal);
        requireContract('sha256:'+Array.from(new Uint8Array(raw),b=>b.toString(16).padStart(2,'0')).join('')===target.content_revision);
      }else{
      const metadata=target.layer==='metadata_record';
      const nativeFields={tos_scholarly_composite_witness_v1:['composite','composite_id'],
        tos_artifact_source_witness_v1:['artifact','artifact_id'],tos_artifact_source_witness_v2:['artifact','artifact_id']};
      const native=metadata&&Object.hasOwn(nativeFields,read.record.schema_version)?nativeFields[read.record.schema_version]:null;
      if(native)requireContract(!Object.hasOwn(read.record,'record_id')&&!Object.hasOwn(read.record,'record_type')
        &&target.record_type===native[0]&&target.record_ref.id.startsWith('tos.'+native[0]+'.'));
      requireContract(read.record[metadata?(native?.[1]??'record_id'):'claim_id']===target.record_ref.id
        &&read.record[metadata?'record_version':'claim_version']===target.record_ref.version);
      if(metadata&&!native)requireContract(read.record.record_type===target.record_type);
      }
    }else requireContract(read.record===null);
    if(representation!=='record'&&read.status==='available'){
      const unitRead=await withAbort(client.request('/api/source/read',{...options,
        body:{handle:discovered.handle,representation}}),controller.signal);
      controller.signal.throwIfAborted();
      validateStatus(unitRead,'tos_source_native_unit_read_result_v1',expected.source_revision,target);
      requireContract(sameJson(unitRead.handle,discovered.handle)&&sameJson(unitRead.record_ref,target.record_ref??null)
        &&unitRead.layer===target.layer&&unitRead.record===null);
      if(unitRead.status==='available'){
        requireRepresentableNumbers(unitRead);
        const unit=unitRead.native_unit,binding=read.record.native_text_binding,access=unitRead.text_access;
        const local=representation==='native_local_unit';
        requireContract(object(binding)&&keys(unit,['schema_version','summary','packet','layer_record_sha256','representation_sha256','spans','closure_fingerprint',...(local?['local_conditions']:[])])
          &&unit.schema_version===(local?'tos_native_local_unit_return_v1':'tos_native_public_unit_return_v1')&&byteSize(unit)<=65536
          &&sameJson(unitRead.access,discovered.handle.access)
          &&keys(access,['scope','recorded_rights_verified','conditional_rights','grants_current_use',...(local?['external_publication_authorized']:[])])
          &&access.scope===(local?'local-native-unit':'public-native-unit')&&access.recorded_rights_verified===true
          &&access.conditional_rights===local&&access.grants_current_use===false);
        if(local){
          const c=unit.local_conditions;
          requireContract(access.external_publication_authorized===false
            &&keys(c,['selection_sha256','expires_at','condition_review','notices'])&&hash(c.selection_sha256)
            &&typeof c.expires_at==='string'&&Number.isFinite(Date.parse(c.expires_at))&&Date.parse(c.expires_at)>Date.now()
            &&typeof c.condition_review==='string'&&c.condition_review.length>0&&c.condition_review.length<=4096
            &&Array.isArray(c.notices)&&c.notices.length>=2&&c.notices.length<=16
            &&['license','attribution'].every(role=>c.notices.some(n=>n?.role===role)));
          let noticeBytes=0;
          for(const n of c.notices){
            requireContract(keys(n,['ref','sha256','role','text'])&&typeof n.ref==='string'&&n.ref.length>0&&n.ref.length<=2048
              &&hash(n.sha256)&&['license','attribution','notice'].includes(n.role)&&typeof n.text==='string');
            const bytes=new TextEncoder().encode(n.text);noticeBytes+=bytes.byteLength;
            requireContract(noticeBytes<=32768);
            const raw=await withAbort(crypto.subtle.digest('SHA-256',bytes),controller.signal);
            requireContract(Array.from(new Uint8Array(raw),b=>b.toString(16).padStart(2,'0')).join('')===n.sha256);
          }
        }
        requireContract(object(unit.summary)&&unit.summary.unit_id===binding.unit_id&&unit.summary.unit_version===binding.unit_version
          &&unit.summary.layer_id===binding.text_layer.layer_id&&unit.summary.layer_version===binding.text_layer.layer_version
          &&unit.summary.segmentation_id===binding.segmentation_id&&unit.summary.segmentation_version===binding.segmentation_version
          &&unit.summary.content_verified===true&&unit.summary.public_content_available===true&&unit.summary.assessment_applied===false
          &&sameJson(unit.packet,{id:binding.packet_id,version:binding.packet_version,sha256:binding.packet_sha256})
          &&unit.layer_record_sha256===binding.text_layer.record_sha256&&hash(unit.representation_sha256)&&digest(unit.closure_fingerprint)
          &&Array.isArray(unit.spans)&&sameJson(unit.spans.map(span=>span?.anchor_ref),binding.ordered_anchor_refs));
        for(const span of unit.spans){
          const s=span.selector;
          requireContract(keys(span,['anchor_ref','selector','exact_sha256','text'])&&typeof span.text==='string'&&hash(span.exact_sha256)
            &&keys(s,['type','position_unit','interval','start','end'])&&s.type==='text_position'&&s.position_unit==='unicode_code_point'
            &&s.interval==='half_open'&&Number.isSafeInteger(s.start)&&Number.isSafeInteger(s.end)&&s.start>=0&&s.end>=s.start
            &&Array.from(span.text).length===s.end-s.start);
          const raw=await withAbort(crypto.subtle.digest('SHA-256',new TextEncoder().encode(span.text)),controller.signal);
          requireContract(Array.from(new Uint8Array(raw),byte=>byte.toString(16).padStart(2,'0')).join('')===span.exact_sha256);
        }
      }else requireContract(unitRead.native_unit===null&&unitRead.text_access===null);
      return {...unitRead,selection:expected};
    }
    return {...read,selection:expected};
  }catch(error){
    if(timedOut)throw new RequestError(504,t('Чтение источника превысило время ожидания.'));
    throw error;
  }finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
}
