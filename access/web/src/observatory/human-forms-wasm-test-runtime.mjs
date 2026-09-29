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
