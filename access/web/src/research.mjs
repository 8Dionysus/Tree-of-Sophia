import {installQueryRequestRules} from './query-operations.ts';
import {installWebMcpRules} from './webmcp.ts';
import {installSourceDossierRules} from './observatory/source-dossier-rules.mjs';
import {installClientPacketRules} from './observatory/client-packet-rules.mjs';
import {installClientInspectionRules} from './observatory/client-inspection-rules.mjs';
import {installKnowledgeSearchRules} from './knowledge-search.ts';
import {installClaimReadingRules} from './observatory/claim-reading-rules.mjs';
import {installRecordContextRules} from './observatory/record-context.mjs';
import '../constructor/style.css';
import {mountLiveResearch} from '../constructor/live-controller.mjs';
import {installBrowserWorkspaceMachine} from './research-workspace-rust.ts';
import {installResearchShelfRules} from './research-shelf/rules.mjs';
import {installWorkspaceCopyRules} from './observatory/workspace-copy-rust.mjs';
import {installInterfaceRules} from './observatory/interface-model-rust.mjs';
import {installPoseRules} from './observatory/view-state-rust.mjs';
import {installClaimReferenceRules} from './observatory/claim-reference-rust.mjs';
import {installHumanFormRules} from './observatory/human-form-rules.mjs';
import {installReadingRules} from './observatory/reading-resume-rust.mjs';
import {installLiveResumeRules} from '../constructor/live-resume.mjs';
import {installSourceFormRules} from '../constructor/source-form-session-rust.mjs';
import {installConditionRules} from './observatory/lens-conditions-rust.mjs';
import {installDraftRules} from './observatory/lens-draft-rust.mjs';
import initRules,* as rules from '../../deploy/cloudflare-worker/generated/tos_web_rules.js';

await initRules(new URL('../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url));
installBrowserWorkspaceMachine(rules.BrowserWorkspaceSession);
installResearchShelfRules(rules);
installWorkspaceCopyRules(rules);
installInterfaceRules(rules);
installPoseRules(rules);
installClaimReferenceRules(rules);
installHumanFormRules(rules);
installRecordContextRules(rules);
installKnowledgeSearchRules(rules);
installQueryRequestRules(rules);
installClaimReadingRules(rules);
installClientInspectionRules(rules);
installSourceDossierRules(rules);
installClientPacketRules(rules);
installWebMcpRules(rules);
installConditionRules(rules);
installDraftRules(rules);
installReadingRules(rules);
installSourceFormRules(rules);
installLiveResumeRules(rules);

// Installable real-data entry. It does not import the constructor demo or its
// authored fixture catalog. The selected service owns discovery and reading.
const host=document.getElementById('tree');
void mountLiveResearch(host).catch(error=>{
  const message=document.createElement('p');message.className='loading';message.setAttribute('role','alert');
  message.textContent=error?.message??'Не удалось открыть исследование.';host.replaceChildren(message);
});
