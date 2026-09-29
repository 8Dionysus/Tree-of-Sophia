/** Source-owner I/O host around the Rust exact-command session. */
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
let SourceFormSession;
export function installSourceFormRules(runtime){
  if(typeof runtime?.BrowserSourceFormSession!=='function')throw new Error('Source form rules are unavailable.');
  SourceFormSession=runtime.BrowserSourceFormSession;
}

export function createSourceFormSession(client,{commandId=()=>`source-form:${crypto.randomUUID()}`}={}){
  if(!SourceFormSession)throw new Error('Source form rules are unavailable.');
  const machine=new SourceFormSession();let busy=false;
  const state=()=>({...JSON.parse(decoder.decode(machine.state_packet())),busy});
  async function exclusive(action){
    if(busy)throw new Error('A source-owner request is already in flight.');
    busy=true;try{return await action();}finally{busy=false;}
  }
  return Object.freeze({state,
    describe:()=>exclusive(async()=>{
      machine.ensure_no_pending();
      machine.accept_describe(encoder.encode(JSON.stringify(await client.execute({schema_version:'tos_local_source_command_v1',operation:'describe'}))));
      return state();
    }),
    prepare:({formId,fieldId})=>exclusive(async()=>{
      machine.prepare_allowed(formId,fieldId);
      machine.accept_prepare(formId,encoder.encode(JSON.stringify(await client.execute({schema_version:'tos_local_source_command_v1',operation:'prepare',form_id:formId,field_id:fieldId}))));
      return state();
    }),
    commit:()=>exclusive(async()=>{
      // Local construction or budget refusal cannot be an uncertain send.
      const pending=JSON.parse(decoder.decode(machine.begin_commit(machine.has_pending()?'':commandId())));
      try{
        // The pending bytes and ID are fixed synchronously before the first await.
        machine.accept_commit(encoder.encode(JSON.stringify(await client.execute(pending))));
        return state();
      }catch(error){machine.mark_uncertain();throw error;}
    }),
    retainedCommand:()=>JSON.parse(decoder.decode(machine.retained_command())),
    dispose:()=>machine.free(),
  });
}
