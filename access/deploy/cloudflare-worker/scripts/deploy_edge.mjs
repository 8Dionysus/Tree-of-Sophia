#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import { readFileSync, copyFileSync, mkdtempSync, rmSync, statSync, readdirSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createInterface } from "node:readline";
import * as deploymentRuntime from '../generated/tos_web_rules.js';
deploymentRuntime.initSync({module: new WebAssembly.Module(readFileSync(new URL('../generated/tos_web_rules_bg.wasm', import.meta.url)))});

export const REVISION_QUERY = "SELECT GROUP_CONCAT(json_chunk, '') AS json FROM (SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part);";
export const LEGACY_REVISION_QUERY = "SELECT json FROM edge_meta WHERE key = 'data_revision';";
const TABLE_QUERY = "PRAGMA table_info(edge_meta);";

// The installed native access product owns SQL algorithms. Node remains the
// platform process adapter for Wrangler; Cargo is never invoked during import.
function edgeSql(args, options = {}) {
  return spawnSync(process.env.TOS_ACCESS_BIN || 'tos', args, options);
}

export function revisionQueryForColumns(columns) {
  const names = new Set(columns);
  const session = new deploymentRuntime.DeployRevisionSession(0);
  try {
    for (;;) switch (session.need()) {
      case 'chunk-column': session.flag(names.has('json_chunk')); break;
      case 'part-column': session.flag(names.has('part')); break;
      case 'legacy-column': session.flag(names.has('json')); break;
      case 'query-return': return {chunk: REVISION_QUERY, legacy: LEGACY_REVISION_QUERY, none: null}[session.choice()];
    }
  } finally { session.free(); }
}

export function resultRows(stdout) {
  const payload = JSON.parse(stdout);
  const session = new deploymentRuntime.DeployRevisionSession(1);
  const rules = deploymentRuntime.DeployRevisionSession;
  try {
    for (;;) switch (session.need()) {
      case 'payload-array': session.flag(Array.isArray(payload)); break;
      case 'failed-results': session.flag(Boolean(payload.some(item => rules.failed_success(item?.success === true)))); break;
      case 'query-error': throw new Error('Cloudflare D1 revision query did not succeed');
      case 'rows-return': return payload.flatMap(item => rules.results_array(Array.isArray(item.results)) ? item.results : []);
    }
  } finally { session.free(); }
}

export function revisionFromRows(rows) {
  const session = new deploymentRuntime.DeployRevisionSession(2);
  let raw, value;
  try {
    for (;;) switch (session.need()) {
      case 'raw-read': raw = rows[0]?.json; session.flag(false); break;
      case 'raw-string': session.flag(typeof raw === 'string'); break;
      case 'parse-revision': value = JSON.parse(raw); session.flag(false); break;
      case 'digest-string': session.flag(typeof value?.sha256 === 'string'); break;
      case 'digest-truthy': session.flag(Boolean(value.sha256)); break;
      case 'digest-return': return value.sha256;
      case 'null-return': return null;
    }
  } finally { session.free(); }
}

export function syncDecision(localRevision, remoteRevision, statementCount, maximum = null) {
  const session = new deploymentRuntime.DeploySyncSession(false);
  try {
    for (;;) switch (session.need()) {
      case 'count': session.number(typeof statementCount === 'number', typeof statementCount === 'number' ? statementCount : 0); break;
      case 'maximum-null': session.flag(maximum === null); break;
      case 'maximum': session.number(typeof maximum === 'number', typeof maximum === 'number' ? maximum : 0); break;
      case 'revision': session.flag(localRevision === remoteRevision); break;
      case 'remote-truthy': session.flag(Boolean(remoteRevision)); break;
      case 'count-error': throw new Error('generated D1 statement count is missing or invalid');
      case 'ceiling-error': throw new Error(`generated D1 read model has ${statementCount} statements; safety ceiling is ${maximum}`);
      case 'decision-return': return {required: session.required(), reason: session.reason()};
    }
  } finally { session.free(); }
}

export function syncFile(manifest, currentRevision) {
  const session = new deploymentRuntime.DeploySyncSession(true);
  let delta, statements;
  try {
    for (;;) switch (session.need()) {
      case 'delta-read': delta = manifest.counts?.delta; session.flag(false); break;
      case 'available': session.flag(delta?.available === true); break;
      case 'base': session.flag(delta.base_revision === currentRevision); break;
      case 'target': session.flag(delta.target_revision === manifest.data_revision); break;
      case 'full-count': statements = manifest.counts?.sql_statements; session.flag(false); break;
      case 'delta-count': statements = delta.sql_statements; session.flag(false); break;
      case 'file-return': return {file: session.file(), statements, mode: session.mode()};
    }
  } finally { session.free(); }
}

function wrangler(args, capture = false) {
  const command = process.platform === "win32" ? "npx.cmd" : "npx";
  const result = spawnSync(command, ["--no-install", "wrangler", ...args], {
    cwd: resolve(fileURLToPath(new URL("..", import.meta.url))),
    encoding: "utf8",
    stdio: capture ? ["inherit", "pipe", "pipe"] : "inherit",
  });
  if (result.status !== 0) {
    if (capture && result.stderr) process.stderr.write(result.stderr);
    throw new Error(`wrangler ${args[0] ?? "command"} failed with exit code ${result.status ?? "unknown"}`);
  }
  return result.stdout ?? "";
}

function remoteRevision(location = "--remote") {
  const tableRows = resultRows(
    wrangler(["d1", "execute", "DB", location, "--command", TABLE_QUERY, "--json"], true),
  );
  const session = new deploymentRuntime.DeployRevisionSession(3);
  const rules = deploymentRuntime.DeployRevisionSession;
  let query;
  try {
    for (;;) switch (session.need()) {
      case 'table-length': {
        const length = tableRows.length;
        session.table_length(typeof length === 'number', typeof length === 'number' ? length : 0); break;
      }
      case 'columns-read': query = revisionQueryForColumns(tableRows.map(row => row.name).filter(name => rules.string_column(typeof name === 'string'))); session.flag(false); break;
      case 'query-present': session.flag(Boolean(query)); break;
      case 'schema-error': throw new Error('Cloudflare D1 edge_meta schema is unsupported');
      case 'execute-revision': return revisionFromRows(resultRows(wrangler(["d1", "execute", "DB", location, "--command", query, "--json"], true)));
      case 'null-return': return null;
    }
  } finally { session.free(); }
}

// Rust owns SQL framing, sequencing, budget and held source currentness. The
// platform sends one request after consuming each file; no next file is eager.
export async function* sqlImportChunks(path, maximumBytes = 16 * 1024 * 1024,
  maximumSeconds = process.env.TOS_D1_SQL_STREAM_MAX_SECONDS) {
  const directory = mkdtempSync(join(dirname(resolve(path)), '.tos-import-'));
  let child;
  let lines;
  let closed;
  let stderr = '';
  let finished = false;
  try {
    child = spawn(process.env.TOS_ACCESS_BIN || 'tos', ['edge-sql-stream',
      '--source', resolve(path), '--directory', directory,
      '--maximum-bytes', String(maximumBytes), '--maximum-bytes-type', typeof maximumBytes,
      '--max-seconds', String(maximumSeconds)], {stdio: ['pipe', 'pipe', 'pipe']});
    closed = new Promise((done) => {
      child.once('error', (error) => { stderr = error.message; });
      child.once('close', (code, signal) => done({code, signal}));
    });
    child.stderr.on('data', (data) => { stderr = (stderr + data.toString()).slice(-8192); });
    child.stdin.on('error', () => {}); // Exit/close carries any EPIPE failure.
    lines = createInterface({input: child.stdout});
    const frames = lines[Symbol.asyncIterator]();
    while (true) {
      child.stdin.write('next\n');
      const response = await frames.next();
      if (response.done) {
        const status = await closed;
        throw new Error(`SQL import framing failed: ${stderr.trim() || `exit ${status.code}, signal ${status.signal}`}`);
      }
      const frame = JSON.parse(response.value);
      if (frame.kind === 'done') { finished = true; break; }
      // This validates the local process protocol's path envelope, not SQL.
      if (frame.kind !== 'chunk' || typeof frame.file !== 'string'
          || !/^part-[0-9]+\.sql$/.test(frame.file)) throw new Error('SQL import framing returned an invalid file envelope');
      yield join(directory, frame.file);
    }
  } finally {
    if (child) {
      child.stdin.end();
      // EOF releases the held native reader. Reap this exact child before
      // removing its owned district, with a platform cancellation grace.
      let timer;
      const graceful = await Promise.race([closed.then(() => true),
        new Promise((done) => { timer = setTimeout(() => done(false), 5000); })]);
      clearTimeout(timer);
      if (!graceful) child.kill('SIGTERM');
      const status = await closed;
      lines?.close();
      rmSync(directory, { recursive: true });
      if (finished && status.code !== 0) throw new Error(`SQL import framing failed: ${stderr.trim()}`);
    } else rmSync(directory, { recursive: true });
  }
}

async function main() {
  const manifest = JSON.parse(readFileSync(new URL("../runtime/manifest.json", import.meta.url), "utf8"));
  const localRevision = manifest.data_revision;
  if (typeof localRevision !== "string" || !localRevision) throw new Error("generated data revision is missing");

  const configuredMaximum = process.env.TOS_D1_MAX_SYNC_STATEMENTS;
  const maximum = configuredMaximum === undefined ? null : Number.parseInt(configuredMaximum, 10);
  const location = process.argv.includes("--local") ? "--local" : "--remote";
  const currentRevision = remoteRevision(location);
  const selected = syncFile(manifest, currentRevision);
  const statementCount = selected.statements;
  const decision = syncDecision(localRevision, currentRevision, statementCount, maximum);
  console.log(
    decision.required
      ? `D1 data sync required (${decision.reason}, ${selected.mode}, ${statementCount} statements).`
      : "D1 data sync skipped: deployed source revision is already current.",
  );

  if (process.argv.includes("--plan")) return;
  if (decision.required) {
    const input = fileURLToPath(new URL('../' + selected.file, import.meta.url));
    const localStore = fileURLToPath(new URL('../.wrangler/state/v3/d1/miniflare-D1DatabaseObject/', import.meta.url));
    const candidates = location === '--local' && selected.mode === 'full' && process.env.TOS_D1_LOCAL_SQLITE !== '0'
      ? readdirSync(localStore).filter((name) => /^[0-9a-f]{64}\.sqlite$/.test(name)) : [];
    if (candidates.length === 1) {
      console.log('Streaming local bootstrap in one SQLite transaction; remote imports always use Wrangler.');
      const imported = edgeSql(['edge-import-local',
        '--database', join(localStore, candidates[0]), '--sql', input,
        '--base', currentRevision ?? 'null', '--target', localRevision], {stdio: 'inherit'});
      if (imported.status !== 0) throw new Error('local SQLite bootstrap failed; transaction rolled back');
    } else if (statSync(input).size <= 16 * 1024 * 1024) {
      wrangler(["d1", "execute", "DB", location, `--file=${input}`, "--yes"]);
    } else {
      let part = 0;
      for await (const file of sqlImportChunks(input)) {
        console.log(`Importing bounded SQL part ${++part}.`);
        wrangler(["d1", "execute", "DB", location, `--file=${file}`, "--yes"], true);
      }
    }
    if (remoteRevision(location) !== localRevision) throw new Error("D1 publication did not produce the expected data revision");
  }
  // Also install on delta/no-op paths; no request performs DDL. This migration
  // is additive and idempotent and never clears existing checkpoints.
  wrangler(["d1", "execute", "DB", location, `--file=${fileURLToPath(new URL("../migrations/0001-exploration.sql", import.meta.url))}`, "--yes"], true);
  copyFileSync(new URL("../runtime/read-model.rows.json", import.meta.url), new URL("../runtime/read-model.deployed.rows.json", import.meta.url));
  if (!process.argv.includes("--data-only") && location !== "--local") wrangler(["deploy"]);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
