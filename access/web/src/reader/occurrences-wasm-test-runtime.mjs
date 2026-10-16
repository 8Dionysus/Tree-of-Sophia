// Vitest runs in Node, so initialize the same generated binding with its bytes
// instead of the browser-only ?url asset handled by reader-rules.mjs.
import initRules, * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {readFile} from 'node:fs/promises';
import {installReaderOccurrenceRules} from './occurrences-rust.mjs';

await initRules({
  module_or_path: await readFile(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm', import.meta.url)),
});
installReaderOccurrenceRules(rules);
