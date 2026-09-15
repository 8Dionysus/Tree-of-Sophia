import {KnowledgeClient} from '../src/observatory/knowledge-client.mjs';
import {createCorpusNotebook} from '../src/corpus-reader/notebook.mjs';
import {mountNativeReader} from '../src/corpus-reader/native-reader.mjs';

const R='a'.repeat(64),C='b'.repeat(64),D='sha256:'+'c'.repeat(64),H='d'.repeat(64);
const selection={kind:'node',id:'source-claims:identity:tos.text-unit.fixture',source_revision:R,content_revision:C};
const target={layer:'metadata_record',record_type:'text-unit',record_ref:{id:'tos.text-unit.fixture',version:1,digest:D},content_revision:D};
const access={scope:'public-metadata-record',visibility:'public_metadata_only',visibility_verified:true,rights_revalidated:false,
  rights_scope:'metadata-disclosure-only',authority:'source-owner-public-metadata-contract'};
let revision=R,version=1,restricted=false,local=false,requests=0,returned=0;
const hash=async value=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(value))),b=>b.toString(16).padStart(2,'0')).join('');
const texts=['A😀e\u0301 — заметка должна остаться у точных исходных символов.\n\nТекст сохраняет пробелы, строки и знаки источника.','אבג — هذا مقطع مستقل.\n\nЭто отдельный фрагмент, между ним и предыдущим есть разрыв в источнике.'];
const spans=[];let start=20;
for(let i=0;i<texts.length;i++){const text=texts[i],end=start+Array.from(text).length;spans.push({anchor_ref:'tos.anchor.fixture-'+i,selector:{type:'text_position',position_unit:'unicode_code_point',interval:'half_open',start,end},exact_sha256:await hash(text),text});start=end+40;}
const noticeText='Synthetic local license and attribution. Do not use this fixture as a rights assertion.',noticeHash=await hash(noticeText);
const flags={grants_current_use:false,performs_assessment:false,writes_to_source:false};
function packets(){
  const binding={unit_id:'tos.text-unit.fixture',unit_version:version,segmentation_id:'tos.text-segmentation.fixture',segmentation_version:version,
    packet_id:'tos.source-text-unit-packet.fixture',packet_version:version,packet_sha256:H,text_layer:{layer_id:'tos.text-layer.fixture',layer_version:1,record_sha256:H},ordered_anchor_refs:spans.map(s=>s.anchor_ref)};
  const handle={schema_version:'tos_source_read_handle_v1',issuer:'Tree-of-Sophia/source-witnesses',epoch:{source_revision:revision,catalog_root_sha256:H,
    catalog_namespace:'tos.synthetic',source_publication:{protocol:'tos_selected_source_metadata_v1',token:'sha256:'+H,generation:1}},target,access,handle_digest:'sha256:'+H};
  const base={source_revision:revision,content_revision:D,handle,record_ref:target.record_ref,layer:'metadata_record',access,...flags};
  const unit={schema_version:local?'tos_native_local_unit_return_v1':'tos_native_public_unit_return_v1',summary:{unit_id:binding.unit_id,unit_version:version,
    segmentation_id:binding.segmentation_id,segmentation_version:version,layer_id:binding.text_layer.layer_id,layer_version:1,language:'ru',content_verified:true,public_content_available:true,assessment_applied:false},
    packet:{id:binding.packet_id,version,sha256:H},layer_record_sha256:H,representation_sha256:H,closure_fingerprint:'sha256:'+H,spans};
  if(local)unit.local_conditions={selection_sha256:H,expires_at:new Date(Date.now()+90000).toISOString(),condition_review:'Synthetic 90-second local conditions.',
    notices:['license','attribution'].map(role=>({role,ref:role+'.md',sha256:noticeHash,text:noticeText}))};
  return {
    inspection:{schema:'tos_knowledge_node_packet_v1',source_revision:revision,matches:[{id:selection.id,content_revision:C,display:{title:{ru:'Искусственная текстовая единица'}},source_refs:['ToS/synthetic.json']}],source_read_targets:{[selection.id]:{source_revision:revision,target}}},
    capabilities:{schema_version:'tos_source_read_capabilities_v1',available:true,source_epoch:{source_revision:revision},representations:['record','native_public_unit','native_local_unit'],authority:{writes_to_source:false,grants_current_use:false}},
    discovery:{schema_version:'tos_source_handle_discovery_v1',status:'available',reason:'synthetic',source_revision:revision,content_revision:D,target,handle,access,...flags},
    record:{schema_version:'tos_source_read_result_v1',status:'available',reason:'synthetic',...base,record:{record_type:'text-unit',record_id:target.record_ref.id,record_version:1,native_text_binding:binding},provenance:{}},
    unit:{schema_version:'tos_source_native_unit_read_result_v1',status:restricted?'access-restricted':'available',reason:restricted?'synthetic-restriction':'synthetic',...base,record:null,native_unit:restricted?null:unit,
      text_access:restricted?null:{scope:local?'local-native-unit':'public-native-unit',recorded_rights_verified:true,conditional_rights:local,grants_current_use:false,...(local?{external_publication_authorized:false}:{})}}
  };
}
const output=document.querySelector('output');
const client=new KnowledgeClient({fetcher:async(path,options)=>{
  const data=packets(),body=options.body?JSON.parse(options.body):null;requests++;
  const packet=path.startsWith('/api/knowledge/')?data.inspection:path.endsWith('/capabilities')?data.capabilities:path.endsWith('/handles')?data.discovery:body.representation==='record'?data.record:data.unit;
  output.textContent=`Requests: ${requests}; graph returns: ${returned}; source version: ${version}`;
  return new Response(JSON.stringify(packet));
}});
const notebook=createCorpusNotebook({dbName:'tos-native-reader-validation-v1'});
const reader=mountNativeReader({client,notebook,onGraphRequest:()=>{returned++;output.textContent=`Requests: ${requests}; graph returns: ${returned}`;}});
const add=(label,run)=>{const b=document.createElement('button');b.textContent=label;b.type='button';b.onclick=run;document.querySelector('nav').append(b);};
add('Открыть текст',()=>reader.open({selection:{...selection,source_revision:revision},representation:local?'native_local_unit':'native_public_unit'}));
add('Мои записи',()=>reader.openNotes());
add('Обновить корпус',()=>{revision=revision===R?H:R;output.textContent='Unrelated graph revision changed.';});
add('Сменить сегментацию',()=>{version++;output.textContent='Native segmentation changed.';});
add('Ограничить доступ',()=>{restricted=!restricted;output.textContent='Restricted: '+restricted;});
add('Локальные условия',()=>{local=!local;output.textContent='Local reading: '+local;});
// Keep this standalone fixture on the same best-effort pagehide flush route as
// the real host, so the browser regression can exercise the native position
// write without introducing a second lifecycle implementation.
window.addEventListener('pagehide',event=>{if(!event.persisted)void reader.flush();});
