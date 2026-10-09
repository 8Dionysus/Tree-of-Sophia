import {DatabaseSync} from 'node:sqlite';
import {createHash} from 'node:crypto';
import {existsSync, readFileSync, realpathSync} from 'node:fs';
import {basename, dirname, relative, resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {brotliDecompressSync} from 'node:zlib';

const repoRoot = resolve(fileURLToPath(new URL('../../../../', import.meta.url)));
const metadataPath = fileURLToPath(new URL('./fixtures/python-oracles/frozen-oracles.v1.json.br', import.meta.url));
const outputPath = fileURLToPath(new URL('./fixtures/python-oracles/frozen-oracle-output.v1.br', import.meta.url));
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const metadata = JSON.parse(brotliDecompressSync(readFileSync(metadataPath)).toString('utf8'));
if (metadata.schema !== 'tos_worker_frozen_python_oracles_v2') {
  throw new Error('unsupported frozen Python oracle fixture schema');
}
const compressedOutput = readFileSync(outputPath);
if (compressedOutput.length !== metadata.output_blob.compressed_bytes ||
    hash(compressedOutput) !== metadata.output_blob.compressed_sha256) {
  throw new Error('frozen Python oracle output bank compressed integrity mismatch');
}
const outputBank = brotliDecompressSync(compressedOutput);
if (outputBank.length !== metadata.output_blob.bytes || hash(outputBank) !== metadata.output_blob.sha256) {
  throw new Error('frozen Python oracle output bank integrity mismatch');
}
const normalizedPathSources = new Set([
  'access/deploy/cloudflare-worker/test/native-inspection.test.mjs',
  'access/deploy/cloudflare-worker/test/native-temporal.test.mjs',
  'access/deploy/cloudflare-worker/test/native-exploration.test.mjs',
]);
const sequenceCursors = new Map();

function inputBytes(value) {
  if (value === undefined) return Buffer.alloc(0);
  if (Buffer.isBuffer(value) || value instanceof Uint8Array) return Buffer.from(value);
  if (typeof value === 'string') return Buffer.from(value);
  throw new TypeError('frozen Python oracle input must be the exact string or bytes passed to the former subprocess');
}

function canonicalInput(raw, sourcePath) {
  if (!normalizedPathSources.has(sourcePath)) return raw;
  let payload;
  try { payload = JSON.parse(raw.toString('utf8')); } catch { return raw; }
  if (!payload || typeof payload !== 'object' || typeof payload.path !== 'string') return raw;
  const fragment = `"path":${JSON.stringify(payload.path)}`;
  const at = raw.toString('utf8').indexOf(fragment);
  if (at < 0) throw new Error(`cannot locate captured path input for ${sourcePath}`);
  const text = raw.toString('utf8');
  if (text.indexOf(fragment, at + fragment.length) >= 0) throw new Error(`ambiguous path input for ${sourcePath}`);
  return Buffer.from(text.replace(fragment, '"path":"<external-sqlite>"'));
}

function externalState(raw) {
  let payload;
  try { payload = JSON.parse(raw.toString('utf8')); } catch { return []; }
  if (!payload || typeof payload.path !== 'string') return [];
  const databasePath = resolve(payload.path);
  const state = [];
  for (const suffix of ['', '-wal', '-shm', '-journal']) {
    const candidate = `${databasePath}${suffix}`;
    if (!existsSync(candidate)) continue;
    const actual = realpathSync(candidate);
    const bytes = readFileSync(actual);
    state.push({basename: basename(actual), bytes: bytes.length, sha256: hash(bytes)});
  }
  return state;
}

function sameState(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function resultBytes(row) {
  const output = metadata.outputs[row.output_id];
  if (!output || output.sha256 !== row.result_sha256 || output.bytes !== row.result_bytes ||
      !Number.isSafeInteger(output.offset) || output.offset < 0 || output.offset + output.bytes > outputBank.length) {
    throw new Error(`invalid frozen Python oracle output descriptor ${row.output_id}`);
  }
  const bytes = outputBank.subarray(output.offset, output.offset + output.bytes);
  if (hash(bytes) !== row.result_sha256) throw new Error(`frozen Python oracle output hash mismatch ${row.output_id}`);
  return bytes;
}

/** Execute the frozen Python result for the exact former execFileSync argv and stdin. */
export function frozenPythonOracleExec(sourceUrl, args, options = {}) {
  if (!Array.isArray(args)) throw new TypeError('frozen Python oracle argv must be an array');
  const codeIndex = args.indexOf('-c');
  if (codeIndex < 0 || typeof args[codeIndex + 1] !== 'string') {
    throw new Error('frozen Python oracle requires the exact former -c program argument');
  }
  const code = args[codeIndex + 1];
  const codeHash = hash(Buffer.from(code, 'utf8'));
  const rawInput = inputBytes(options.input);
  const sourcePath = relative(repoRoot, fileURLToPath(sourceUrl)).split(sep).join('/');
  if (!Object.hasOwn(metadata.source_hashes, sourcePath)) {
    throw new Error(`no frozen Python oracle source provenance for ${sourcePath}`);
  }
  const canonical = canonicalInput(rawInput, sourcePath);
  const inputHash = hash(canonical);
  const candidates = metadata.records.filter((row) => row.source_path === sourcePath &&
    row.code_sha256 === codeHash && row.canonical_input_sha256 === inputHash);
  if (!candidates.length) {
    throw new Error(`no frozen Python oracle outcome for ${sourcePath}, code=${codeHash}, input=${inputHash}`);
  }
  const currentState = externalState(rawInput);
  const matching = candidates.filter((row) => sameState(row.external_files, currentState));
  if (!matching.length) {
    throw new Error(`no frozen Python oracle outcome for ${sourcePath}, code=${codeHash}, input=${inputHash}, SQLite-state=${JSON.stringify(currentState)}`);
  }
  let selected;
  const sequenced = matching.filter((row) => row.sequence_group !== null).sort((a, b) => a.sequence_index - b.sequence_index);
  if (sequenced.length) {
    if (sequenced.length !== matching.length) throw new Error(`mixed ordered and single frozen outcomes for ${sourcePath}, code=${codeHash}`);
    const cursor = sequenceCursors.get(sequenced[0].sequence_group) ?? 0;
    selected = sequenced[cursor];
    if (!selected) throw new Error(`frozen Python oracle sequence exhausted for ${sourcePath}, code=${codeHash}`);
    sequenceCursors.set(sequenced[0].sequence_group, cursor + 1);
  } else {
    const outputIds = new Set(matching.map((row) => row.output_id));
    if (outputIds.size !== 1 || matching.length !== 1) {
      throw new Error(`ambiguous frozen Python oracle state for ${sourcePath}, code=${codeHash}`);
    }
    selected = matching[0];
  }
  const output = resultBytes(selected);
  if (selected.expected_error === true) throw new Error(output.toString('utf8'));
  if (selected.sqlite_mutation) {
    const mutation = selected.sqlite_mutation, payload = JSON.parse(rawInput.toString('utf8'));
    if (sourcePath !== 'access/deploy/cloudflare-worker/test/native-temporal.test.mjs' ||
        codeHash !== '6131a87f57f68e9cffa53aadfd4a9ccf8301832ef6256a3a27f0de1bfc98f03b' ||
        mutation.before.length !== 2 || mutation.after.length !== 2) {
      throw new Error('unexpected frozen SQLite mutation profile');
    }
    const db = new DatabaseSync(payload.path);
    try {
      db.exec('BEGIN IMMEDIATE');
      for (let index = 0; index < mutation.before.length; index++) {
        const before = mutation.before[index], after = mutation.after[index];
        if (before.id !== after.id ||
            db.prepare('SELECT json FROM knowledge_nodes WHERE id=?').get(before.id)?.json !== before.json ||
            db.prepare('SELECT json_chunk FROM edge_meta WHERE key=? AND part=0').get('knowledge_node_digest:'+before.id)?.json_chunk !== before.metadata ||
            hash(Buffer.from(after.json)) !== JSON.parse(after.metadata).sha256) {
          throw new Error('frozen SQLite mutation predecessor or checksum differs');
        }
        const row = db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').run(after.json, after.id);
        const meta = db.prepare('UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0').run(after.metadata, 'knowledge_node_digest:'+after.id);
        if (row.changes !== 1 || meta.changes !== 1) throw new Error('frozen SQLite mutation row count');
      }
      db.exec('COMMIT');
    } catch (error) {
      try { db.exec('ROLLBACK'); } catch {}
      throw error;
    } finally { db.close(); }
    if (!sameState(externalState(rawInput), mutation.external_after)) {
      throw new Error('frozen SQLite mutation external result differs');
    }
  }
  return output.toString(options.encoding || 'utf8');
}
