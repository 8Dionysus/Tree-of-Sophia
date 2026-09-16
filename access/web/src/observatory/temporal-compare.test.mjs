import {afterEach,test} from 'vitest';
import assert from 'node:assert/strict';
import {temporalComparisonRequest,compareExactDates,temporalComparisonRelationLabel} from './temporal-compare.mjs';
import {ContractError,RevisionError} from './knowledge-client.mjs';
import {setUiLanguage} from './ui-i18n.mjs';
const H='a'.repeat(64),C='b'.repeat(64),left={kind:'node',sourceRevision:H,raw:{id:'claim:left',content_revision:C}},right={...left,raw:{id:'claim:right',content_revision:H}};
const fixture=()=>({schema_version:'tos_temporal_comparison_result_v1',source_revision:H,request:temporalComparisonRequest(left,right),
  comparison:{status:'comparable',relation:'before',reasons:[],basis:'normalized-source-date-envelopes'},
  left:{claim:left.raw,value:null,normalized_time:{year:1}},right:{claim:right.raw,value:null,normalized_time:{year:2}},source_refs:['ToS/source-witnesses/fixture.json'],
  authority_boundary:{is_source:false,writes_to_tree:false,performs_assessment:false,creates_inferred_claim:false,comparison_basis:'normalized-source-date-envelopes',note:'Source envelopes only.'}});

afterEach(()=>setUiLanguage('ru'));

test.each([
  ['before','Раньше','Before','Antes'],['after','Позже','After','Después'],['equal','Совпадают','Equal','Coinciden'],
  ['contains','Первый диапазон включает второй','The first envelope contains the second','El primer intervalo contiene al segundo'],
  ['contained-by','Второй диапазон включает первый','The second envelope contains the first','El segundo intervalo contiene al primero'],
  ['overlaps','Пересекаются','Overlap','Se superponen'],
])('comparable relation label is localized: %s',(relation,ru,en,es)=>{
  setUiLanguage('ru');assert.equal(String(temporalComparisonRelationLabel(relation)),ru);
  setUiLanguage('en');assert.equal(String(temporalComparisonRelationLabel(relation)),en);
  setUiLanguage('es');assert.equal(String(temporalComparisonRelationLabel(relation)),es);
});

test('exact date comparison preserves selected nodes, content and source revision',async()=>{
  let sent;const response=fixture();assert.deepEqual(await compareExactDates({request:async(path,options)=>{sent={path,...options};return response;}},left,right),response);
  assert.equal(sent.path,'/temporal/compare');assert.deepEqual(sent.body,response.request);assert.equal(sent.maxResponseBytes,2097152);
  assert.throws(()=>temporalComparisonRequest(left,{...right,sourceRevision:C}),RevisionError);
  assert.throws(()=>temporalComparisonRequest(left,{...right,kind:'relation'}),ContractError);
});
test('changed subjects, content, authority and unsafe normalization cannot become an exact comparison',async()=>{
  for(const mutate of [p=>p.left.claim={...left.raw,id:'foreign'},p=>p.right.claim={...right.raw,content_revision:C},
    p=>p.request={...p.request,left:{...p.request.left,node_id:'foreign'}},p=>p.authority_boundary.creates_inferred_claim=true,
    p=>p.comparison.relation=null,p=>p.comparison.reasons=[{side:'pair',code:'unexpected'}],p=>p.left.normalized_time.year=Number.MAX_SAFE_INTEGER+1]){
    const p=structuredClone(fixture());mutate(p);await assert.rejects(compareExactDates({request:async()=>p},left,right),ContractError);
  }
});
test('undetermined and unsupported outcomes retain reasons and never invent an ordering',async()=>{
  for(const status of ['undetermined','unsupported']){const p=fixture();p.comparison={...p.comparison,status,relation:null,reasons:[{side:'left',code:'source-date-unavailable'}]};
    assert.equal((await compareExactDates({request:async()=>p},left,right)).comparison.status,status);
    p.comparison.relation='before';await assert.rejects(compareExactDates({request:async()=>p},left,right),ContractError);
  }
});
