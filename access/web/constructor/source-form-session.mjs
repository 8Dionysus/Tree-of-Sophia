/** One real owner-delegated source-copy form workflow, not semantic review. */
const clone=value=>JSON.parse(JSON.stringify(value));
const request=operation=>({schema_version:'tos_local_source_command_v1',operation});

function context(value){
  if(value?.schema_version!=='tos_local_source_command_result_v1'||
    !Array.isArray(value.command_operations)||!['describe','prepare','apply'].every(op=>value.command_operations.includes(op))||
    !Array.isArray(value.source_fields)||!Array.isArray(value.allowed_form_ids)||
    !value.source||typeof value.owner_configuration!=='string')
    throw new Error('The selected owner does not provide the source-copy form workflow.');
  return clone(value);
}

export function createSourceFormSession(client,{commandId=()=>`source-form:${crypto.randomUUID()}`}={}){
  let current=null,prepared=null,pending=null,result=null,busy=false,uncertain=false;
  const state=()=>clone({current,prepared,pending,result,busy,uncertain});
  async function exclusive(action){
    if(busy)throw new Error('A source-owner request is already in flight.');
    busy=true;try{return await action();}finally{busy=false;}
  }
  return Object.freeze({state,
    describe:()=>exclusive(async()=>{
      if(pending)throw new Error('Reconcile or retain the existing exact command before opening another draft.');
      current=context(await client.execute(request('describe')));prepared=null;result=null;return state();
    }),
    prepare:({formId,fieldId})=>exclusive(async()=>{
      if(pending)throw new Error('An exact submitted command is still retained.');
      if(!current?.allowed_form_ids.includes(formId)||!current?.source_fields.some(field=>field.field_id===fieldId))
        throw new Error('Choose an owner-declared field and delegated form identity.');
      const value=context(await client.execute({...request('prepare'),form_id:formId,field_id:fieldId}));
      const change=value.prepared_change;
      if(!change||!['form.create','form.revise'].includes(change.operation)||
        change.form?.form_id!==formId||change.form?.content?.kind!=='source-copy')
        throw new Error('The owner did not return the requested exact source-copy change.');
      // Capture every expected value from this preparation, not a later describe.
      current=value;prepared=clone(change);result=null;return state();
    }),
    commit:()=>exclusive(async()=>{
      if(!prepared)throw new Error('Prepare and inspect a source-copy change before applying it.');
      if(!pending)pending={...request('apply'),command_id:commandId(),expected_source:clone(current.source),
        expected_revision:current.revision,expected_configuration:current.owner_configuration,changes:[clone(prepared)]};
      try{
        result=await client.execute(clone(pending));
        // A success result must still be this owner protocol, not a generic 2xx.
        if(result?.schema_version!=='tos_local_source_command_result_v1'||!result.receipt||
          result.receipt.command_id!==pending.command_id)throw new Error('Owner receipt did not confirm this command.');
        uncertain=false;return state();
      }catch(error){uncertain=true;throw error;}
    }),
    retainedCommand:()=>pending?clone(pending):null,
  });
}
