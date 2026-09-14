import {parseNativeJson,nativeChild,nativeKeys,nativeJson,nativeNumberInfo} from '../../../shared/native-semantics.ts';
import {FormContractError,exactFormRef} from './human-forms.mjs';

// A transport consumer, not a context classifier. Labels/categories come from
// the source-owned sidecar. Exact canonical materials preserve number lexemes.
const verified=new WeakMap(),own=(value,key)=>value!==null&&typeof value==='object'&&Object.hasOwn(value,key);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const requireContext=condition=>{if(!condition)throw new FormContractError();};
const digest=value=>typeof value==='string'&&/^sha256:[a-f0-9]{64}$(?![\s\S])/.test(value);
const pointer=value=>typeof value==='string'&&value.length<=2048&&/^(?:\/(?:[^~/]|~[01])*)*$(?![\s\S])/.test(value);
const parts=value=>value===''?[]:value.slice(1).split('/').map(key=>key.replace(/~1/g,'/').replace(/~0/g,'~'));
function at(value,path){
  requireContext(pointer(path));
  for(const key of parts(path)){
    requireContext(own(value,key)&&(!Array.isArray(value)||/^(0|[1-9]\d*)$/.test(key)));
    value=value[key];
  }
  return value;
}
function nativeAt(value,path){
  requireContext(pointer(path));
  for(const key of parts(path)){requireContext(nativeKeys(value).includes(key));value=nativeChild(value,key);}
  return value;
}
function sameDelivered(a,b){
  if(a===null||b===null||typeof a!=='object'||typeof b!=='object')return Object.is(a,b);
  if(Array.isArray(a)!==Array.isArray(b))return false;
  const keys=Object.keys(a);return keys.length===Object.keys(b).length&&keys.every(key=>own(b,key)&&sameDelivered(a[key],b[key]));
}
function localized(value,nullable=false){
  if(nullable&&value===null)return true;
  return object(value)&&Object.keys(value).length>=1&&Object.keys(value).length<=8
    &&Object.entries(value).every(([key,text])=>/^[A-Za-z][A-Za-z0-9-]*$/.test(key)&&typeof text==='string'&&text.length>0&&text.length<=1024);
}
function displayed(ref){
  const value=ref.value;
  return {type:value===null?'null':Array.isArray(value)?'array':typeof value,
    text:typeof value==='string'?value:typeof value==='number'?nativeNumberInfo(ref).lexeme:nativeJson(ref,32768)};
}
const formKey=ref=>ref?JSON.stringify([ref.id,ref.version,ref.digest]):null;
const escape=key=>key.replace(/~/g,'~0').replace(/\//g,'~1');
function expectedContexts(raw){
  const result=[],attributes=raw.attributes??{},forms=attributes.human_forms??[];
  requireContext(Array.isArray(forms));
  const assertions=raw.semantics?.assertion_contexts??[];requireContext(Array.isArray(assertions));
  if(!forms.some(packet=>packet.state==='ready'))for(const key of ['source_record','source_claim']){
    if(object(attributes[key]))result.push({path:'/attributes/'+key,form:null,values:Object.keys(attributes[key]).map(field=>'/attributes/'+key+'/'+escape(field))});
  }
  assertions.forEach((context,index)=>{
    const path='/semantics/assertion_contexts/'+index,values=[];requireContext(object(context.fields)&&Array.isArray(context.conflicts));
    for(const [key,field]of Object.entries(context.fields)){
      const base=path+'/fields/'+escape(key)+'/value';
      if(key==='record'&&object(field.value))values.push(...Object.keys(field.value).map(name=>base+'/'+escape(name)));
      else values.push(base);
    }
    if(context.conflicts.length)values.push(path+'/conflicts');
    result.push({path,form:null,values});
  });
  forms.forEach((packet,index)=>{
    if(packet.state!=='ready')return;
    const base='/attributes/human_forms/'+index,path=base+'/context',values=[];requireContext(Array.isArray(packet.context));
    packet.context.forEach((entry,ordinal)=>{
      const valuePath=path+'/'+ordinal+'/value';
      if(entry.binding?.pointer===''&&object(entry.value))values.push(...Object.keys(entry.value).map(key=>valuePath+'/'+escape(key)));
      else values.push(valuePath);
    });
    for(const key of ['subject_assessment','assessment_snapshot','language_context'])if(own(packet,key))values.push(base+'/'+key);
    result.push({path,form:packet.form,values});
  });
  return result;
}

export async function verifyReadableContext(raw){
  if(!own(raw,'readable_context'))return null;
  verified.delete(raw);
  // JSON.stringify would conflate -0 with 0 and an overflowed source integer
  // with null. A bounded structural copy keeps those delivery distinctions.
  const stamp=structuredClone([raw.readable_context,raw.attributes,raw.semantics]);
  const remember=result=>{
    requireContext(sameDelivered(stamp,[raw.readable_context,raw.attributes,raw.semantics]));
    verified.set(raw,{result,stamp});return structuredClone(result);
  };
  const input=raw.readable_context;
  requireContext(object(input)&&new TextEncoder().encode(JSON.stringify(input)).length<=32768
    &&input.schema_version==='tos_readable_context_v1'
    &&['complete','requires-exact-context','unavailable'].includes(input.state)
    &&typeof input.reason==='string'&&input.reason.length>0&&input.reason.length<=128
    &&input.performs_semantic_assessment===false&&input.performs_translation===false);
  const vocabulary=input.vocabulary,coverage=input.coverage;
  requireContext(object(vocabulary)&&vocabulary.id==='tos.context-presentation.governing'
    &&Number.isSafeInteger(vocabulary.version)&&vocabulary.version>=1&&digest(vocabulary.digest)
    &&vocabulary.source_ref==='ToS/doctrine/semantic-interchange/entity-types.v1.json'
    &&object(coverage)&&['input_contexts','returned_contexts','entries','unclassified_entries'].every(key=>Number.isSafeInteger(coverage[key])&&coverage[key]>=0)
    &&Array.isArray(input.contexts)&&input.contexts.length<=64
    &&Array.isArray(input.exact_materials)&&input.exact_materials.length<=256
    &&Array.isArray(input.exact_context_pointers)&&input.exact_context_pointers.length<=64
    &&input.exact_context_pointers.every(pointer));
  const result={state:input.state,reason:input.reason,vocabulary:structuredClone(vocabulary),contexts:[],coverage:structuredClone(coverage)};
  if(input.state!=='complete'){
    requireContext(!input.contexts.length&&!input.exact_materials.length&&coverage.returned_contexts===0&&coverage.entries===0&&coverage.unclassified_entries===0);
    // Missing context remains an explicit gap, not a successful empty view.
    if(input.state==='requires-exact-context')for(const path of input.exact_context_pointers)at(raw,path);
    return remember(result);
  }
  const expected=expectedContexts(raw);
  requireContext(coverage.input_contexts===expected.length&&input.contexts.length===expected.length
    &&input.exact_context_pointers.length===expected.length&&new Set(input.exact_context_pointers).size===expected.length);
  const materials=new Map(),origins=[];
  for(const material of input.exact_materials){
    requireContext(object(material)&&digest(material.digest)&&!materials.has(material.digest)
      &&typeof material.canonical_json==='string'&&material.canonical_json.length>0
      &&Array.isArray(material.origin_pointers)&&material.origin_pointers.length>0&&material.origin_pointers.length<=256
      &&new Set(material.origin_pointers).size===material.origin_pointers.length&&material.origin_pointers.every(pointer));
    const bytes=new TextEncoder().encode(material.canonical_json);
    const actual='sha256:'+Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),byte=>byte.toString(16).padStart(2,'0')).join('');
    requireContext(actual===material.digest);
    const ref=parseNativeJson(material.canonical_json,{maxBytes:32768,maxDepth:64,maxMembers:30000});
    materials.set(material.digest,ref);
    for(const path of material.origin_pointers){
      requireContext(!origins.some(origin=>origin.path===path)&&sameDelivered(at(raw,path),ref.value));
      origins.push({path,ref});
    }
  }
  // Overlapping material roots must preserve the same exact number lexemes,
  // not merely compare equal after JSON.parse rounded a large integer.
  for(const child of origins)for(const parent of origins)if(child.path.startsWith(parent.path+'/'))
    requireContext(nativeJson(nativeAt(parent.ref,child.path.slice(parent.path.length)))===nativeJson(child.ref));
  function sourceAt(path,binding){
    const matches=origins.filter(origin=>path===origin.path||path.startsWith(origin.path+'/')).sort((a,b)=>b.path.length-a.path.length);
    // Ordinary forms reference fields of a retained source record; the owner
    // deliberately does not duplicate each field as another exact material.
    requireContext(matches.length>0||binding.kind==='record'&&exactFormRef(binding.record)&&materials.has(binding.record.digest));
    const ref=matches.length?nativeAt(matches[0].ref,path.slice(matches[0].path.length))
      :nativeAt(materials.get(binding.record.digest),binding.source_pointer);
    requireContext(sameDelivered(ref.value,at(raw,path)));return ref;
  }
  let entries=0,unknown=0;
  const contextKeys=new Set();
  for(const context of input.contexts){
    requireContext(object(context)&&pointer(context.origin_pointer)&&input.exact_context_pointers.includes(context.origin_pointer)
      &&(context.form===null||exactFormRef(context.form))&&Array.isArray(context.entries)&&context.entries.length<=256);
    at(raw,context.origin_pointer);
    const key=JSON.stringify([context.origin_pointer,formKey(context.form)]);requireContext(!contextKeys.has(key));contextKeys.add(key);
    const owner=expected.find(value=>value.path===context.origin_pointer&&formKey(value.form)===formKey(context.form));
    requireContext(owner&&context.entries.length===owner.values.length);
    const seenValues=new Set();
    const mapped={origin_pointer:context.origin_pointer,form:structuredClone(context.form),entries:[]};
    for(const entry of context.entries){
      requireContext(++entries<=256&&object(entry)&&typeof entry.key==='string'
        &&['governing','technical','unclassified'].includes(entry.category)&&localized(entry.label)&&localized(entry.explanation,true)
        &&pointer(entry.value_pointer)&&object(entry.binding)&&pointer(entry.binding.source_pointer)
        &&(entry.language===null||typeof entry.language==='string')&&(entry.script===null||typeof entry.script==='string'));
      const binding=entry.binding,ref=sourceAt(entry.value_pointer,binding);
      requireContext(owner.values.includes(entry.value_pointer)&&!seenValues.has(entry.value_pointer));seenValues.add(entry.value_pointer);
      if(binding.kind==='record'){
        requireContext(exactFormRef(binding.record)&&materials.has(binding.record.digest));
        requireContext(nativeJson(nativeAt(materials.get(binding.record.digest),binding.source_pointer))===nativeJson(ref));
        const record=materials.get(binding.record.digest).value;
        const identity=['record_id','claim_id','form_id','artifact_id','composite_id','node_id'].find(key=>own(record,key));
        const version=own(record,'claim_id')?'claim_version':own(record,'form_id')?'form_version':'record_version';
        requireContext(identity&&record[identity]===binding.record.id&&record[version]===binding.record.version);
        if(context.form){
          const suffix=parts(entry.value_pointer.slice(context.origin_pointer.length)),original=at(raw,context.origin_pointer)[suffix[0]];
          requireContext(suffix[1]==='value'&&formKey(original.binding.record)===formKey(binding.record)
            &&binding.source_pointer===original.binding.pointer+(suffix.length>2?'/'+suffix.slice(2).map(escape).join('/'):''));
        }
      }else if(binding.kind==='assertion-context'){
        const source=at(raw,context.origin_pointer);
        requireContext(context.form===null&&source.schema_version==='tos_assertion_context_v1'
          &&typeof binding.source_record_digest==='string'&&/^[a-f0-9]{64}$/.test(binding.source_record_digest)
          &&binding.source_record_digest===source.source_record_digest);
        if(entry.value_pointer===context.origin_pointer+'/conflicts')requireContext(binding.source_pointer==='');
        else{
          const suffix=parts(entry.value_pointer.slice(context.origin_pointer.length));
          requireContext(suffix[0]==='fields'&&suffix[2]==='value');
          const field=source.fields[suffix[1]],nested=suffix.slice(3).map(escape);
          requireContext(binding.source_pointer===field.source_pointer+(nested.length?'/'+nested.join('/'):''));
        }
      }else if(binding.kind==='form-materialization'){
        const packetPath=context.origin_pointer.slice(0,-'/context'.length),packet=at(raw,packetPath);
        requireContext(exactFormRef(binding.form)&&exactFormRef(binding.subject)
          &&formKey(binding.form)===formKey(context.form)&&formKey(binding.subject)===formKey(packet.subject)
          &&entry.value_pointer===packetPath+binding.source_pointer
          &&typeof binding.packet_digest==='string'&&/^[a-f0-9]{64}$/.test(binding.packet_digest));
      }else requireContext(false);
      if(entry.category==='technical')requireContext(entry.value_mode==='exact-reference'&&!own(entry,'value')&&entry.value_label===null);
      else{
        requireContext(['source-value','vocabulary-value'].includes(entry.value_mode)&&own(entry,'value')&&sameDelivered(entry.value,ref.value));
        requireContext(entry.value_mode==='vocabulary-value'?localized(entry.value_label):entry.value_label===null);
      }
      if(entry.category==='unclassified')unknown++;
      mapped.entries.push({...structuredClone(entry),display:displayed(ref)});
    }
    result.contexts.push(mapped);
  }
  requireContext(coverage.returned_contexts===result.contexts.length&&coverage.entries===entries&&coverage.unclassified_entries===unknown);
  return remember(result);
}

export function readableContextFor(raw){
  if(!own(raw,'readable_context'))return null;
  const saved=verified.get(raw);
  requireContext(saved&&sameDelivered(saved.stamp,[raw.readable_context,raw.attributes,raw.semantics]));
  return structuredClone(saved.result);
}

export function formContexts(view,form){
  return view?.contexts.filter(context=>formKey(context.form)===formKey(form))??[];
}
