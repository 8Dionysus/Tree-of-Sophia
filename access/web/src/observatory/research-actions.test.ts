import {describe,it,expect} from 'vitest';
import {createResearchWorkspace,type ResearchProposalInput} from '../research-workspace';
import {stageObservation} from './research-actions';
const proposal:Omit<ResearchProposalInput,'baseWorkspaceRevision'>={id:'proposal:1',kind:'interpretation',parentHypothesisId:'hypothesis:1',
  targetId:'source:work',statement:'A reading anchored to its source.',sourceRefs:['ToS/fixture/source.json'],evidenceRefs:['ToS/fixture/source.json'],
  confidencePosture:{value:'unknown',meaning:'maker_declared_uncertainty_not_truth_probability'},actorOrigin:'human',basePageRevision:4,dataFingerprint:'a'.repeat(64)};
describe('observatory research adapter',()=>{
  it('leaves no orphan hypothesis or journal entry when proposal validation fails',()=>{
    const workspace=createResearchWorkspace({persistence:false});workspace.addNote({id:'note:existing',body:'Keep my history'});
    const before=workspace.exportPacket();
    expect(()=>stageObservation(workspace,{...proposal,evidenceRefs:[]})).toThrow();
    expect(workspace.exportPacket()).toBe(before);expect(workspace.canUndo()).toBe(true);
  });
  it('preserves exact identity, snapshot and non-canonical posture through export/import',()=>{
    const workspace=createResearchWorkspace({persistence:false});const id='graph:'+ 'a-long-opaque-id/'.repeat(12);
    const result=stageObservation(workspace,{...proposal,targetId:id});
    expect(result.targetId).toBe(id);expect(result.dataFingerprint).toBe(proposal.dataFingerprint);expect(result.reviewStatus).toBe('pending_review');expect(result.reviewRequirement).toBe('human_or_authorized_agent');expect(result.canon).toBe(false);
    const loaded=createResearchWorkspace({persistence:false});loaded.importPacket(workspace.exportPacket());
    expect(loaded.getState().proposals[0]).toEqual(result);
  });
});
