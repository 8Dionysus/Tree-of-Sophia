// Standalone Observatory fixtures use the same generated rules as src/entry.ts.
// They intentionally do not mount the production application or select data.
import initRules,* as rules from '../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import rulesWasmUrl from '../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm?url';
import {installClientPacketRules} from '../src/observatory/client-packet-rules.mjs';
import {installDraftRules} from '../src/observatory/lens-draft-rust.mjs';
import {installConditionRules} from '../src/observatory/lens-conditions-rust.mjs';
import {installClientInspectionRules} from '../src/observatory/client-inspection-rules.mjs';
import {installPoseRules} from '../src/observatory/view-state-rust.mjs';
import {installReaderOccurrenceRules} from '../src/reader/occurrences-rust.mjs';
import {installCorpusReferenceRules} from '../src/corpus-reader/corpus-reference-rules.mjs';

await initRules(rulesWasmUrl);
installCorpusReferenceRules(rules);
installClientPacketRules(rules);
installDraftRules(rules);
installConditionRules(rules);
installClientInspectionRules(rules);
installPoseRules(rules);
installReaderOccurrenceRules(rules);
