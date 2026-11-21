// The research workspace keeps owning its thoughts, proposals and its own undo. Its journal is mirrored into the
// act journal as workspace-event acts, so the board, history and agent replay see the same work.
// Removal condition: this adapter goes when the workspace reads its state from the act journal.
import {draftAct,LOCAL_PERSON} from './drafts.mjs';

const ACTION=/^[A-Za-z0-9._:-]{1,64}$/;

/** The act a workspace journal entry becomes. Its ID is deterministic, so mirroring twice adds nothing. */
export function workspaceEventAct(entry,{sessionId,author=LOCAL_PERSON,at=new Date()}){
  const session=String(sessionId).replace(/[^A-Za-z0-9._:-]/g,'-').slice(0,64)||'local';
  const action=ACTION.test(String(entry.action))?String(entry.action):'change';
  return draftAct({id:`ws-${session}-${entry.sequence}`,kind:'workspace-event',author,
    targets:[entry.targetId?String(entry.targetId).slice(0,1024):`workspace:${session}`],
    intent:`Workspace: ${action}`,body:{action,sequence:entry.sequence,...(entry.detail?{detail:entry.detail}:{})},at});
}

/** Mirror new workspace journal entries. Returns a function that stops mirroring. */
export function mirrorWorkspace(workspace,journal,{sessionId='local',author=LOCAL_PERSON}={}){
  let last=-1,queue=Promise.resolve();
  const pending=state=>(state.journal??[]).filter(entry=>Number.isInteger(entry.sequence)&&entry.sequence>last);
  const push=state=>{
    const entries=pending(state);if(!entries.length)return;
    last=Math.max(last,...entries.map(entry=>entry.sequence));
    queue=queue.then(async()=>{
      for(const entry of entries){
        try{await journal.append(workspaceEventAct(entry,{sessionId,author}));}
        catch(error){if(error?.code!=='act_duplicate_id')console.error('Workspace mirror',error);}
      }
    });
  };
  push(workspace.getState());
  const stop=workspace.subscribe(push);
  return ()=>{stop?.();return queue;};
}
