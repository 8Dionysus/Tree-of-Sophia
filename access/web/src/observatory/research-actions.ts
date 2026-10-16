import {type ResearchWorkspace, type ResearchProposalInput} from '../research-workspace';
import {createBrowserResearchWorkspace} from '../research-workspace-rust';

/** Validate the complete proposal before changing the user's local journal. */
export function stageObservation(workspace: ResearchWorkspace, input: Omit<ResearchProposalInput, 'baseWorkspaceRevision'>) {
  const hypothesis = {id:input.parentHypothesisId,title:input.statement.slice(0,100),body:input.statement,
    targetId:input.targetId,fromId:input.fromId,toId:input.toId};
  const preview=createBrowserResearchWorkspace({persistence:false});
  let proposal:ResearchProposalInput;
  try{
    preview.importPacket(workspace.exportPacket());
    preview.addHypothesis(hypothesis);
    proposal={...input,createdAt:input.createdAt??new Date().toISOString(),baseWorkspaceRevision:preview.summary().revision};
    preview.stageProposal(proposal);
  }finally{if('dispose' in preview)preview.dispose();}
  workspace.addHypothesis(hypothesis);
  return workspace.stageProposal(proposal);
}
