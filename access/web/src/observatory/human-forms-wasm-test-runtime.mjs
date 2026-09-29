// These focused source tests consume the actual generated browser binding.
// Build ownership and artifact admission remain with the matched OPS lane.
import {readFileSync} from 'node:fs';
import * as runtime from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {installHumanFormRules} from './human-form-rules.mjs';
import {installReadingRules} from './reading-resume-rust.mjs';

runtime.initSync({module:new WebAssembly.Module(Uint8Array.from(readFileSync(new URL(
  '../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url,
))))});
installHumanFormRules(runtime);
installReadingRules(runtime);

import {installBrowserWorkspaceMachine} from '../research-workspace-rust.ts';
import {installConstructorRules} from '../../constructor/model.mjs';
import {installSourceFormRules} from '../../constructor/source-form-session-rust.mjs';
import {installResearchShelfRules} from '../research-shelf/rules.mjs';
import {installClaimReferenceRules} from './claim-reference-rust.mjs';
import {installConditionRules} from './lens-conditions-rust.mjs';
import {installDraftRules} from './lens-draft-rust.mjs';
installBrowserWorkspaceMachine(runtime.BrowserWorkspaceSession);
installConstructorRules(runtime);
installSourceFormRules(runtime);
installResearchShelfRules(runtime);
installClaimReferenceRules(runtime);
installConditionRules(runtime);
installDraftRules(runtime);
