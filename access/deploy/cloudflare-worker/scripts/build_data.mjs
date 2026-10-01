#!/usr/bin/env node
// Platform invocation only; the installed Rust product owns public D1 build.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const worker = fileURLToPath(new URL('../', import.meta.url));
const result = spawnSync(process.env.TOS_ACCESS_BIN || 'tos', [
  'build-data',
  '--source-root', fileURLToPath(new URL('../../../../', import.meta.url)),
  '--output', 'dist', '--runtime', 'runtime',
], { cwd: worker, stdio: 'inherit' });
if (result.error) console.error(`native public D1 build failed: ${result.error.message}`);
process.exitCode = result.status ?? 1;
