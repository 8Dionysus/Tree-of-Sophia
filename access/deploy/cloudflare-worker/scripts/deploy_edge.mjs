#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REVISION_QUERY = "SELECT json FROM edge_meta WHERE key = 'data_revision';";
const TABLE_QUERY = "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'edge_meta';";

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

function remoteRevision() {
  const tableRows = resultRows(
    wrangler(["d1", "execute", "DB", "--remote", "--command", TABLE_QUERY, "--json"], true),
  );
  if (tableRows.length === 0) return null;
  return revisionFromRows(
    resultRows(wrangler(["d1", "execute", "DB", "--remote", "--command", REVISION_QUERY, "--json"], true)),
  );
}

function main() {
  const manifest = JSON.parse(readFileSync(new URL("../runtime/manifest.json", import.meta.url), "utf8"));
  const localRevision = manifest.data_revision;
  const statementCount = manifest.counts?.sql_statements;
  if (typeof localRevision !== "string" || !localRevision) throw new Error("generated data revision is missing");

  const configuredMaximum = process.env.TOS_D1_MAX_SYNC_STATEMENTS;
  const maximum = configuredMaximum === undefined ? null : Number.parseInt(configuredMaximum, 10);
  const currentRevision = remoteRevision();
  const decision = syncDecision(localRevision, currentRevision, statementCount, maximum);
  console.log(
    decision.required
      ? `D1 data sync required (${decision.reason}, ${statementCount} statements).`
      : "D1 data sync skipped: deployed source revision is already current.",
  );

  if (process.argv.includes("--plan")) return;
  if (decision.required) {
    wrangler(["d1", "execute", "DB", "--remote", "--file=runtime/read-model.sql", "--yes"]);
  }
  wrangler(["deploy"]);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
