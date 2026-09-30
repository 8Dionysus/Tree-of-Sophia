// Standalone reader/Lens fixtures use the same generated rules as src/entry.ts.
// They intentionally do not mount the production application or select data.
import initRules,* as rules from '../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {installDraftRules} from '../src/observatory/lens-draft-rust.mjs';
import {installClientInspectionRules} from '../src/observatory/client-inspection-rules.mjs';

await initRules(new URL('../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url));
installDraftRules(rules);
installClientInspectionRules(rules);
