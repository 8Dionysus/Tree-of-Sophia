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
import initRules,* as rules from '../../deploy/cloudflare-worker/generated/tos_web_rules.js';

await initRules(new URL('../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url));
installBrowserWorkspaceMachine(rules.BrowserWorkspaceSession);
installResearchShelfRules(rules);
installWorkspaceCopyRules(rules);
installInterfaceRules(rules);
installPoseRules(rules);
installClaimReferenceRules(rules);
installHumanFormRules(rules);
installConditionRules(rules);
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
