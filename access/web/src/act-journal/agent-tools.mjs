// Addressed act commands for agents (access/contracts/act-commands.v1.json), offered as WebMCP tools beside the
// page's pointing commands. They name their targets, so many agents can use them at once without cancelling one
// another. What an agent may append is bounded here; the journal's Rust rules check every record.
import actCommands from '../../../contracts/act-commands.v1.json' with {type:'json'};
import pageCommands from '../../../contracts/page-commands.v1.json' with {type:'json'};
import fieldShapes from '../../../contracts/field-shapes.v1.json' with {type:'json'};
import {draftAct} from './drafts.mjs';

export const MAX_AGENT_BATCH=100;
// Decisions, assignments, squads, change packets and mirrored workspace events belong to the person.
const PERSON_ONLY=new Set(['review','assignment','squad','change-packet','workspace-event']);
const PROPOSING=new Set(['ticket','proposal']);
export const AGENT_TIERS=Object.freeze(['read','draft','propose']);

export class AgentActRefused extends Error{
  constructor(code,message){super(message);this.name='AgentActRefused';this.code=code;}
}

/** The manifest an agent reads before it acts: what exists, what each kind requires, what it may do. */
export async function actManifest(journal,{tier='propose'}={}){
  const kinds=await journal.query({view:'kinds'});
  return {
    schema:'tos.act.manifest.v1',
    posture:actCommands.authority_limit,
    tier,
    invariants:kinds.invariants,
    kinds:kinds.kinds.map(kind=>({...kind,agent:PERSON_ONLY.has(kind.name)?'person-only':PROPOSING.has(kind.name)?'propose-tier':'draft-tier'})),
    categories:kinds.categories,
    decisions:kinds.decisions,
    limits:{targets:64,grounds:64,parents:32,intent_chars:280,record_bytes:65536,batch:MAX_AGENT_BATCH},
    addressed_commands:actCommands.commands,
    pointing_commands:pageCommands.commands.map(({command_id,availability,effect})=>({command_id,availability,effect})),
    permission_tiers:actCommands.permission_tiers,
    // The shapes the interface draws for a declared kind: thoughts, links, trial lenses and routes of the field.
    field_shapes:fieldShapes,
  };
}

/** Check one act an agent asks to append and turn it into a draft. */
export async function agentDraft(journal,input,{tier='propose'}={}){
  if(!AGENT_TIERS.includes(tier)||tier==='read')throw new AgentActRefused('act_tier','This agent may read the journal but not append to it.');
  const author=input?.author;
  if(!author||typeof author!=='object'||!['agent','mixed'].includes(author.type))throw new AgentActRefused('act_author','An agent appends as an agent or a mixed author, never in the person\'s name.');
  const kind=String(input.kind??'');
  if(PERSON_ONLY.has(kind))throw new AgentActRefused('act_person_only',`A ${kind} act belongs to the person.`);
  if(PROPOSING.has(kind)&&tier!=='propose')throw new AgentActRefused('act_tier',`A ${kind} needs the propose tier.`);
  if(kind==='withdrawal'){
    for(const target of input.targets??[]){
      const act=await journal.query({view:'act',id:String(target).replace(/^act:/,'')}).catch(()=>null);
      if(!act||act.author.id!==author.id)throw new AgentActRefused('act_not_own','An agent withdraws only its own acts.');
    }
  }
  const {id,targets,grounds=[],intent,body={},parents=[],replaces}=input;
  return draftAct({...(id?{id}:{}),kind,author,targets,grounds,intent,body,parents,replaces});
}

const summary=result=>({seq:result.seq,id:result.record.id,digest:result.record.digest,collisions:result.collisions});
const text=value=>({content:[{type:'text',text:JSON.stringify(value)}]});
const schema=(properties,required=[])=>({type:'object',properties,required,additionalProperties:false});
const str={type:'string'},list={type:'array',items:str},act={type:'object'};
const actInput=schema({id:str,kind:str,author:{type:'object'},targets:list,grounds:list,intent:str,body:act,parents:list,replaces:str},['kind','author','targets','intent']);

/** The WebMCP tools of the act journal. `tier` is the person's choice for agents connected to this page. */
export function actJournalTools(journal,{tier=()=> 'propose'}={}){
  const tool=(name,description,inputSchema,run,readOnlyHint=false)=>({name,description,inputSchema,annotations:{readOnlyHint,untrustedContentHint:true},
    execute:async input=>text(await run(input??{}))});
  return [
    tool('tos.act.manifest','Read first: the kinds of act you may append, what each requires, the categories of tickets and proposals, the decisions, the limits, every addressed and pointing command and your permission tier. Acts are the person\'s local work; none of them writes the Tree.',schema({}),()=>actManifest(journal,{tier:tier()}),true),
    tool('tos.act.query','Read one fold of the act journal: summary, act (id), target (address), board, review, decided, leases (now_ms), presence (now_ms, window_ms), history (id) or kinds. Folds name acts by id; read whole records with tos.act.record.',
      schema({view:str,id:str,address:str,now_ms:{type:'integer'},window_ms:{type:'integer'},limit:{type:'integer',minimum:1,maximum:1000}},['view']),input=>journal.query(input),true),
    tool('tos.act.record','Read the whole stored act record by id.',schema({id:str},['id']),async input=>(await journal.record(input.id))??{missing:input.id},true),
    tool('tos.act.append','Append one act as an agent: author {type:"agent",id,model,role?,squad?}, at least one target address, grounds (exact places) unless the kind makes them optional, one line of intent and the kind\'s body. Take a lease before long work on a place; collisions report another author\'s live lease. Decisions, assignments and squads belong to the person.',
      actInput,async input=>summary(await journal.append(await agentDraft(journal,input,{tier:tier()})))),
    tool('tos.act.append-batch',`Append up to ${MAX_AGENT_BATCH} acts in order, each as for tos.act.append. Stops at the first refusal and reports it with the acts already appended.`,
      schema({acts:{type:'array',maxItems:MAX_AGENT_BATCH,items:act}},['acts']),async input=>{
        const items=input.acts??[];if(items.length>MAX_AGENT_BATCH)throw new AgentActRefused('act_batch',`At most ${MAX_AGENT_BATCH} acts at once.`);
        const drafts=[];for(const item of items)drafts.push(await agentDraft(journal,item,{tier:tier()}));
        try{return {appended:(await journal.appendMany(drafts)).map(summary),failure:null};}
        catch(error){return {appended:(error.appended??[]).map(summary),failure:{index:error.index??0,error:error.code??String(error.message)}};}
      }),
  ];
}
