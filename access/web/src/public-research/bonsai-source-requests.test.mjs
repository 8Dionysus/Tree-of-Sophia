import {describe,expect,it} from 'vitest';
import {collectSourceOccurrences,commitSourceRequest,occurrenceSourceCandidate,prepareSourceRequest,unitOccurrenceSourceCandidate} from '../../../../bonsai/lib/source-requests.mjs';

const candidate=()=>({kind:'claims',claims:[
  {claim_id:'tos.claim.first',review_status:'unreviewed'},
  {claim_id:'tos.claim.second',review_status:'unreviewed'},
],forms:[
  {claim_id:'tos.claim.first',form_id:'tos.form.first',field_id:'claim.statement'},
  {claim_id:'tos.claim.second',form_id:'tos.form.second',field_id:'claim.statement'},
]});
const preview={result:{owner_configuration:'owner-digest',expected_dependencies:'dependencies',source_bindings:[{id:'bound'}]}};

describe('lexical Occurrence source context',()=>{
  const lexical=()=>({id:'stand-use',form:'Brüder',cite:{w:'also-sprach-zarathustra',c:'I.01',u:['unit-1','unit-1']},
    native_candidate:{record_id:'tos.occurrence.example'},semantic_use:{meaning:{en:'A broader group reading across several units.'},assessment:{verdict:'retain'}}});
  const options={binding:{schema_version:'tos_native_text_unit_binding_v1',unit_id:'unit-1',packet_ref:'packet',packet_sha256:'digest',
    packet_id:'packet-id',packet_version:1,segmentation_id:'segmentation',segmentation_version:1,unit_version:1,ordered_anchor_refs:['anchor'],
    text_layer:{record_ref:'layer',record_sha256:'layer-digest',layer_id:'layer-id',layer_version:1},source_record_refs:{work:'work',expression:'expression',edition:'edition',item:'item'}},
    sourceRefs:['source'],formId:'tos.form.example'};
  const packet=use=>({schema:'tos.bonsai.philosophy-export.v1',semantic_uses:{terms:[{occurrences:[use]}]}});

  it('keeps broader meaning in the retained use, not the single-unit context',()=>{
    const use=lexical();
    const direct=occurrenceSourceCandidate(use,options);
    const {subjects}=collectSourceOccurrences(packet(use));
    const collected=unitOccurrenceSourceCandidate(subjects[0],options);
    for(const result of [direct,collected])expect(result.record.semantic_content.context_account).toBe('Chapter I.01 of also-sprach-zarathustra.');
    expect(direct.record.extensions.bonsai_uses).toEqual([use]);
    expect(collected.record.extensions.bonsai_uses).toEqual([{kind:'lexical-use',use}]);
    expect(collected.record.extensions.bonsai_uses[0].use.semantic_use).toEqual(use.semantic_use);
  });

  it('refuses absent or blank chapter or work in either lexical route',()=>{
    for(const [field,value] of [['c',undefined],['w',undefined],['c',' '],['w','']]){
      const use=lexical();use.cite[field]=value;
      expect(()=>occurrenceSourceCandidate(use,options)).toThrow();
      expect(()=>collectSourceOccurrences(packet(use))).toThrow();
    }
  });
});

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
