import initRules, * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {installBrowserWorkspaceMachine} from '../research-workspace-rust.ts';
import {installCorpusReferenceRules} from '../corpus-reader/corpus-reference-rules.mjs';
import {installReaderOccurrenceRules} from '../reader/occurrences-rust.mjs';
import {installResearchShelfRules} from '../research-shelf/rules.mjs';
import {installDraftRules} from '../observatory/lens-draft-rust.mjs';
import {installConditionRules} from '../observatory/lens-conditions-rust.mjs';
import {installHumanFormRules} from '../observatory/human-form-rules.mjs';
import {installReadingRules} from '../observatory/reading-resume-rust.mjs';
import {installConstructorRules} from '../../constructor/model.mjs';
import {installConstructorReadingRules} from '../../constructor/reading-rules-rust.mjs';

let ready;

/** Bind every owner used by the public entry, including saved lens conditions. */
export function installPublicReaderRules(runtime) {
  installBrowserWorkspaceMachine(runtime.BrowserWorkspaceSession);
  installCorpusReferenceRules(runtime);
  installReaderOccurrenceRules(runtime);
  installResearchShelfRules(runtime);
  installHumanFormRules(runtime);
  installReadingRules(runtime);
  installConditionRules(runtime);
  installDraftRules(runtime);
  installConstructorRules(runtime);
  installConstructorReadingRules(runtime);
}

export function initializePublicReaderRules() {
  ready ??= (async () => {
    await initRules(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm', import.meta.url));
    installPublicReaderRules(rules);
  })();
  return ready;
}
