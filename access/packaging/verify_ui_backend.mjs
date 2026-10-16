#!/usr/bin/env node
// Compare the selected installed native CLI with its already-running HTTP peer.
// Query semantics stay with the native owner; this host only sends the same
// explicit requests over two transports and checks their packets.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {statSync} from 'node:fs';
import {isAbsolute} from 'node:path';

function usage() {
  return 'usage: node verify_ui_backend.mjs --native-executable ABS --root ABS --release-root ABS --http-base LOOPBACK_URL';
}

function argumentsOf(argv) {
  const result = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i];
    const value = argv[i + 1];
    if (!key?.startsWith('--') || !value || value.startsWith('--') || result[key]) {
      throw new Error(usage());
    }
    result[key] = value;
  }
  for (const key of ['--native-executable', '--root', '--release-root', '--http-base']) {
    if (!result[key]) throw new Error(`${usage()}\nmissing ${key}`);
  }
  return result;
}

function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function nativePacket(executable, root, releaseRoot, command) {
  const result = spawnSync(executable, ['--release-root', releaseRoot, '--root', root, ...command], {
    encoding: 'utf8', timeout: 60_000, maxBuffer: 32 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`native CLI failed (${result.status}): ${result.stderr.slice(0, 2048)}`);
  return JSON.parse(result.stdout);
}

async function httpPacket(base, path) {
  const response = await fetch(new URL(path, base), {signal: AbortSignal.timeout(60_000)});
  if (!response.ok) throw new Error(`native HTTP request failed (${response.status}) for ${path}`);
  return response.json();
}

async function main() {
  const args = argumentsOf(process.argv.slice(2));
  const executable = args['--native-executable'];
  if (!isAbsolute(executable) || !['--root', '--release-root'].every(key => isAbsolute(args[key]))) {
    throw new Error('--native-executable, --root, and --release-root must be absolute paths');
  }
  const executableInfo = statSync(executable);
  if (!executableInfo.isFile() || (executableInfo.mode & 0o111) === 0) {
    throw new Error('--native-executable must select an executable file');
  }
  const root = args['--root'];
  const releaseRoot = args['--release-root'];
  const base = new URL(args['--http-base']);
  if (base.protocol !== 'http:' || !['127.0.0.1', 'localhost', '::1'].includes(base.hostname)
      || base.username || base.password || base.search || base.hash || !['', '/'].includes(base.pathname)) {
    throw new Error('--http-base must be an HTTP loopback origin without path or credentials');
  }

  const requests = [
    {name: 'catalog', command: ['knowledge', 'catalog'], path: '/api/knowledge/catalog'},
    {name: 'indexed-search', command: ['knowledge', 'search', 'Заратустра', '--mode', 'indexed', '--limit', '6'],
      path: '/api/knowledge/search?query=%D0%97%D0%B0%D1%80%D0%B0%D1%82%D1%83%D1%81%D1%82%D1%80%D0%B0&mode=indexed&limit=6'},
  ];
  const results = [];
  for (const request of requests) {
    const cli = nativePacket(executable, root, releaseRoot, request.command);
    const http = await httpPacket(base, request.path);
    assert.equal(canonical(http), canonical(cli), `${request.name} differs between installed CLI and live HTTP`);
    results.push({name: request.name, source_revision: cli.source_revision ?? null});
  }
  const revisions = new Set(results.map(item => item.source_revision).filter(Boolean));
  assert.equal(revisions.size, 1, 'CLI and HTTP did not retain one selected release revision');
  process.stdout.write(`${JSON.stringify({ok: true, source_revision: [...revisions][0], comparisons: results})}\n`);
}

main().catch(error => {
  process.stderr.write(`${error?.stack ?? error}\n`);
  process.exitCode = 1;
});
