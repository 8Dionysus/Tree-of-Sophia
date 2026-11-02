import {describe,expect,it} from 'vitest';
import {commitSourceRequest,prepareSourceRequest} from '../../../../bonsai/lib/source-requests.mjs';

const candidate=()=>({kind:'claims',claims:[
  {claim_id:'tos.claim.first',review_status:'unreviewed'},
  {claim_id:'tos.claim.second',review_status:'unreviewed'},
],forms:[
  {claim_id:'tos.claim.first',form_id:'tos.form.first',field_id:'claim.statement'},
  {claim_id:'tos.claim.second',form_id:'tos.form.second',field_id:'claim.statement'},
]});
const preview={result:{owner_configuration:'owner-digest',expected_dependencies:'dependencies',source_bindings:[{id:'bound'}]}};

describe('source-owner Claim request selection',()=>{
  it('keeps the public default wire and record wire unchanged',()=>{
    const c=candidate();
    expect(prepareSourceRequest(c)).toEqual({schema_version:'tos_local_source_command_v1',operation:'prepare-create',claims:c.claims});
    expect(commitSourceRequest(c,preview,'command')).toEqual({
      schema_version:'tos_local_source_command_v1',operation:'claims.create',claims:c.claims,
      command_id:'command',expected_configuration:'owner-digest',expected_revision:null,
      expected_dependencies:'dependencies',expected_inputs:[{id:'bound'}],
    });
    const record={kind:'record',record:{record_id:'tos.occurrence.example'},forms:[{form_id:'tos.form.example',field_id:'metadata.preferred-name'}]};
    expect(prepareSourceRequest(record,{owner:'private'})).toEqual(prepareSourceRequest(record));
    expect(commitSourceRequest(record,preview,'command',{owner:'private'})).toEqual(commitSourceRequest(record,preview,'command'));
  });

  it('passes explicitly selected private forms into preview and exact create',()=>{
    const c=candidate(),options={owner:'private'};
    const prepared=prepareSourceRequest(c,options);
    expect(prepared).toEqual({schema_version:'tos_local_source_command_v1',operation:'prepare-create',claims:c.claims,forms:c.forms});
    const request=commitSourceRequest(c,preview,'command',options);
    expect(request).toEqual({...prepared,operation:'claims.create',command_id:'command',expected_configuration:'owner-digest',
      expected_source:null,expected_revision:null,expected_dependencies:'dependencies',expected_inputs:[{id:'bound'}]});
    c.forms[0].form_id='tos.form.changed';
    preview.result.source_bindings[0].id='changed';
    expect(request.forms[0].form_id).toBe('tos.form.first');
    expect(request.expected_inputs[0].id).toBe('bound');
    preview.result.source_bindings[0].id='bound';
  });

  it('refuses missing, orphaned or duplicate identities and incomplete selectors',()=>{
    const bad=[
      c=>delete c.forms,
      c=>c.forms.pop(),
      c=>{c.forms[1].claim_id='tos.claim.orphan';},
      c=>{c.forms[1].form_id=c.forms[0].form_id;},
      c=>{c.claims[1].claim_id=c.claims[0].claim_id;},
      c=>{c.forms[0].field_id='claim.summary';},
      c=>{c.forms[0].extra=true;},
      c=>{c.forms=Array.from({length:33},(_,i)=>({claim_id:'tos.claim.first',form_id:`tos.form.f${i}`,field_id:'claim.statement'}));},
    ];
    for(const mutate of bad){const c=candidate();mutate(c);expect(()=>prepareSourceRequest(c,{owner:'private'})).toThrow();}
    expect(()=>prepareSourceRequest(candidate(),{owner:'other'})).toThrow();
    expect(()=>commitSourceRequest(candidate(),preview,'command',{owner:'other'})).toThrow();
  });
});
