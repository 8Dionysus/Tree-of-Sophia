import {installInterpretationComparisonRules} from '../interpretation-comparison.mjs';
import {installQueryRequestRules} from '../query-operations.ts';
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
import {installWorkspaceCopyRules} from './workspace-copy-rust.mjs';
installWorkspaceCopyRules(runtime);

import {installRecordContextRules} from './record-context.mjs';
installRecordContextRules(runtime);

import {installClaimReadingRules} from './claim-reading-rules.mjs';
installClaimReadingRules(runtime);

import {installKnowledgeSearchRules} from '../knowledge-search.ts';
installKnowledgeSearchRules(runtime);
installQueryRequestRules(runtime);
installInterpretationComparisonRules(runtime);

import {installClientInspectionRules} from './client-inspection-rules.mjs';
installClientInspectionRules(runtime);

import {installSourceDossierRules} from './source-dossier-rules.mjs';
import {installClientPacketRules} from './client-packet-rules.mjs';
installSourceDossierRules(runtime);
installClientPacketRules(runtime);

import {installWebMcpRules} from '../webmcp.ts';
installWebMcpRules(runtime);
import {installWorkerClassicRules} from "../../../deploy/cloudflare-worker/src/worker-classic.ts";
installWorkerClassicRules(runtime);

import {installNativePythonRuntime} from '../../../shared/native-semantics.ts';
installNativePythonRuntime(runtime);

import {installSourceNavigationRules} from "../../../deploy/cloudflare-worker/src/source-navigation-rules.ts";
installSourceNavigationRules(runtime);

import {installConstructorReadingRules} from '../../constructor/reading-rules-rust.mjs';
installConstructorReadingRules(runtime);
