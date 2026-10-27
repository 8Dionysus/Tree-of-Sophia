import initRules, * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {installBrowserWorkspaceMachine} from '../research-workspace-rust.ts';
import {installCorpusReferenceRules} from '../corpus-reader/corpus-reference-rules.mjs';
import {installReaderOccurrenceRules} from '../reader/occurrences-rust.mjs';
import {installResearchShelfRules} from '../research-shelf/rules.mjs';
import {installDraftRules} from '../observatory/lens-draft-rust.mjs';
import {installConstructorRules} from '../../constructor/model.mjs';
import {installConstructorReadingRules} from '../../constructor/reading-rules-rust.mjs';

let ready;

export function initializePublicReaderRules() {
  ready ??= (async () => {
    await initRules(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm', import.meta.url));
    installBrowserWorkspaceMachine(rules.BrowserWorkspaceSession);
    installCorpusReferenceRules(rules);
    installReaderOccurrenceRules(rules);
    installResearchShelfRules(rules);
    installDraftRules(rules);
    installConstructorRules(rules);
    installConstructorReadingRules(rules);
  })();
  return ready;
}
