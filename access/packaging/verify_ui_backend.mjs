#!/usr/bin/env node
// Compare the selected installed native CLI with its already-running HTTP peer.
// Query semantics stay with the native owner; this host only sends the same
// explicit requests over two transports and checks their packets.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {statSync} from 'node:fs';
import {isAbsolute} from 'node:path';

function usage() {
  return 'usage: node verify_ui_backend.mjs --native-executable ABS --prepared-read-model ABS --prepared-binding ABS --http-base LOOPBACK_URL';
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
  for (const key of ['--native-executable', '--prepared-read-model', '--prepared-binding', '--http-base']) {
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

function nativePacket(executable, model, binding, command) {
  const result = spawnSync(executable, ['--prepared-read-model', model, '--prepared-binding', binding, ...command], {
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
  if (!isAbsolute(executable) || !['--prepared-read-model', '--prepared-binding'].every(key => isAbsolute(args[key]))) {
    throw new Error('--native-executable, --prepared-read-model, and --prepared-binding must be absolute paths');
  }
  const executableInfo = statSync(executable);
  if (!executableInfo.isFile() || (executableInfo.mode & 0o111) === 0) {
    throw new Error('--native-executable must select an executable file');
  }
  const model = args['--prepared-read-model'];
  const binding = args['--prepared-binding'];
  const base = new URL(args['--http-base']);
  if (base.protocol !== 'http:' || !['127.0.0.1', 'localhost', '::1'].includes(base.hostname)
      || base.username || base.password || base.search || base.hash || !['', '/'].includes(base.pathname)) {
    throw new Error('--http-base must be an HTTP loopback origin without path or credentials');
  }

  const requests = [
    {name: 'catalog', command: ['knowledge', 'catalog'], path: '/api/knowledge/catalog'},
    {name: 'compressed-search', command: ['knowledge', 'search', 'Заратустра', '--mode', 'compressed', '--limit', '6'],
      path: '/api/knowledge/search?query=%D0%97%D0%B0%D1%80%D0%B0%D1%82%D1%83%D1%81%D1%82%D1%80%D0%B0&mode=compressed&limit=6'},
  ];
  const results = [];
  for (const request of requests) {
    const cli = nativePacket(executable, model, binding, request.command);
    const http = await httpPacket(base, request.path);
    if (request.name === 'compressed-search') {
      // Tokens and physical-work observations belong to each process, not
      // the semantic result shared by the two transports.
      for (const packet of [cli, http]) {
        assert.equal(typeof packet.page?.has_more, 'boolean');
        assert.ok(packet.page.has_more
          ? typeof packet.page.next_cursor === 'string' && packet.page.next_cursor.length > 0
          : packet.page.next_cursor === null, 'invalid continuation token');
        assert.ok(packet.work && typeof packet.work === 'object' && !Array.isArray(packet.work), 'work observation missing');
        delete packet.work;
        delete packet.page.next_cursor;
      }
    }
    assert.equal(canonical(http), canonical(cli), `${request.name} differs between installed CLI and live HTTP`);
    assert.ok(typeof cli.source_revision === 'string' && cli.source_revision.length > 0, 'source revision missing');
    results.push({name: request.name, source_revision: cli.source_revision});
  }
  const revisions = new Set(results.map(item => item.source_revision).filter(Boolean));
  assert.equal(revisions.size, 1, 'CLI and HTTP did not retain one selected source revision');
  process.stdout.write(`${JSON.stringify({ok: true, source_revision: [...revisions][0], comparisons: results})}\n`);
}

main().catch(error => {
  process.stderr.write(`${error?.stack ?? error}\n`);
  process.exitCode = 1;
});
