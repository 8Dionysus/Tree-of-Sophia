import {createResearchWorkspace, type ResearchWorkspace, type ResearchProposalInput} from '../research-workspace';

/** Validate the complete proposal before changing the user's local journal. */
export function stageObservation(workspace: ResearchWorkspace, input: Omit<ResearchProposalInput, 'baseWorkspaceRevision'>) {
  const hypothesis = {id:input.parentHypothesisId,title:input.statement.slice(0,100),body:input.statement,
    targetId:input.targetId,fromId:input.fromId,toId:input.toId};
  const preview=createResearchWorkspace({persistence:false});
  preview.importPacket(workspace.exportPacket());
  preview.addHypothesis(hypothesis);
  const proposal={...input,createdAt:input.createdAt??new Date().toISOString(),baseWorkspaceRevision:preview.summary().revision};
  preview.stageProposal(proposal);
  workspace.addHypothesis(hypothesis);
  return workspace.stageProposal(proposal);
}
