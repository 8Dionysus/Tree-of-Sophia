#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync, copyFileSync, createReadStream, mkdtempSync, openSync, writeSync, closeSync, rmSync, statSync, readdirSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

export const REVISION_QUERY = "SELECT GROUP_CONCAT(json_chunk, '') AS json FROM (SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part);";
export const LEGACY_REVISION_QUERY = "SELECT json FROM edge_meta WHERE key = 'data_revision';";
const TABLE_QUERY = "PRAGMA table_info(edge_meta);";

export function revisionQueryForColumns(columns) {
  const names = new Set(columns);
  if (names.has("json_chunk") && names.has("part")) return REVISION_QUERY;
  if (names.has("json")) return LEGACY_REVISION_QUERY;
  return null;
}

export function resultRows(stdout) {
  const payload = JSON.parse(stdout);
  if (!Array.isArray(payload) || payload.some((item) => item?.success !== true)) {
    throw new Error("Cloudflare D1 revision query did not succeed");
  }
  return payload.flatMap((item) => (Array.isArray(item.results) ? item.results : []));
}

export function revisionFromRows(rows) {
  const raw = rows[0]?.json;
  if (typeof raw !== "string") return null;
  const value = JSON.parse(raw);
  return typeof value?.sha256 === "string" && value.sha256 ? value.sha256 : null;
}

export function syncDecision(localRevision, remoteRevision, statementCount, maximum = null) {
  if (!Number.isSafeInteger(statementCount) || statementCount < 1) {
    throw new Error("generated D1 statement count is missing or invalid");
  }
  if (maximum !== null && (!Number.isSafeInteger(maximum) || maximum < 1 || statementCount > maximum)) {
    throw new Error(`generated D1 read model has ${statementCount} statements; safety ceiling is ${maximum}`);
  }
  if (localRevision === remoteRevision) return { required: false, reason: "revision-match" };
  return { required: true, reason: remoteRevision ? "revision-changed" : "database-empty" };
}

export function syncFile(manifest, currentRevision) {
  const delta = manifest.counts?.delta;
  return delta?.available === true && delta.base_revision === currentRevision && delta.target_revision === manifest.data_revision
    ? { file: "runtime/read-model.delta.sql", statements: delta.sql_statements, mode: "delta" }
    : { file: "runtime/read-model.sql", statements: manifest.counts?.sql_statements, mode: "full" };
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
  if (tableRows.length === 0) return null;
  const revisionQuery = revisionQueryForColumns(
    tableRows.map((row) => row.name).filter((name) => typeof name === "string"),
  );
  if (!revisionQuery) throw new Error("Cloudflare D1 edge_meta schema is unsupported");
  return revisionFromRows(
    resultRows(wrangler(["d1", "execute", "DB", location, "--command", revisionQuery, "--json"], true)),
  );
}

// Producer SQL has exactly one complete statement per line, including triggers.
// Bound Wrangler's input string, not just individual database statements.
export async function* sqlImportChunks(path, maximumBytes = 16 * 1024 * 1024) {
  const directory = mkdtempSync(join(dirname(path), '.tos-import-'));
  let descriptor = null;
  let target = null;
  let size = 0;
  let part = 0;
  const lines = createInterface({ input: createReadStream(path), crlfDelay: Infinity });
  try {
    for await (const line of lines) {
      const statement = line + '\n';
      const bytes = Buffer.byteLength(statement);
      if (descriptor !== null && size + bytes > maximumBytes) {
        closeSync(descriptor);
        descriptor = null;
        yield target;
      }
      if (descriptor === null) {
        target = join(directory, `part-${part++}.sql`);
        descriptor = openSync(target, 'wx');
        size = 0;
      }
      writeSync(descriptor, statement);
      size += bytes;
    }
    if (descriptor !== null) {
      closeSync(descriptor);
      descriptor = null;
      yield target;
    }
  } finally {
    lines.close();
    if (descriptor !== null) closeSync(descriptor);
    // Only this invocation's mkdtemp directory, never a caller path.
    rmSync(directory, { recursive: true });
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
      const imported = spawnSync('python', [fileURLToPath(new URL('./import_local_sqlite.py', import.meta.url)),
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
