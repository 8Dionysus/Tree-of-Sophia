#!/usr/bin/env node
// Reader-only transport supervisor. The native owner must retain the actual
// capture/model/currentness callback across this entire call. Input pins are
// observations, never model admission, semantic authority or a native factory.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {constants, fstatSync} from 'node:fs';
import {lstat, realpath, open, writeFile, unlink} from 'node:fs/promises';
import {isAbsolute, resolve, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawn} from 'node:child_process';
import {compareNativePackets} from './verify_native_packets.mjs';
const sha256 = value => createHash('sha256').update(value).digest('hex');
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const positive = value => Number.isSafeInteger(value) && value > 0;
const remaining = deadline => {if (Date.now() >= deadline) throw new Error('public D1 verification deadline exceeded');};
function nodeDeadline(originalNs, maximumSeconds, outerDeadline) {
  assert.ok(typeof originalNs === 'string' && /^[0-9]+$/.test(originalNs) && positive(maximumSeconds));
  const beganWall = Date.now(), beganMono = process.hrtime.bigint();
  const span = BigInt(originalNs) - beganMono;
  assert.ok(span > 0n, 'original work deadline expired before transport I/O');
  const milliseconds = Number(span / 1_000_000n);
  assert.ok(Number.isSafeInteger(milliseconds));
  assert.ok(outerDeadline === undefined || Number.isSafeInteger(outerDeadline));
  const cutoff = Math.min(beganWall + maximumSeconds * 1000, beganWall + milliseconds, outerDeadline ?? Infinity);
  assert.ok(Number.isSafeInteger(cutoff)); remaining(cutoff); return cutoff;
}
const loopback = value => {
  const url = new URL(value);
  assert.ok(url.protocol === 'http:' && url.hostname === '127.0.0.1' && url.port
    && !url.username && !url.password && url.pathname === '/' && !url.search && !url.hash, 'explicit loopback origin required');
  return url.origin;
};
async function heldFile(path, maxBytes, deadline, consume) {
  remaining(deadline);
  assert.ok(isAbsolute(path), 'selected file requires absolute path');
  const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const before = await file.stat({bigint: true});
    assert.ok(before.isFile() && before.size > 0n && before.size <= BigInt(maxBytes), 'selected file absent or oversized');
    const result = await consume(file, before);
    const after = await file.stat({bigint: true}), named = await lstat(path, {bigint: true});
    for (const field of ['dev','ino','size','mtimeNs','ctimeNs']) {
      assert.equal(after[field], before[field], 'selected file changed during read');
      assert.equal(named[field], before[field], 'selected file namespace changed during read');
    }
    remaining(deadline); return result;
  } finally {await file.close();}
}
async function boundedRead(path, maxBytes, deadline) {
  return heldFile(path, maxBytes, deadline, async file => {
    const chunks = []; let count = 0;
    for (;;) {
      remaining(deadline);
      const buffer = Buffer.allocUnsafe(Math.min(64 * 1024, maxBytes + 1 - count));
      const {bytesRead} = await file.read(buffer, 0, buffer.length, null);
      if (!bytesRead) break;
      count += bytesRead; assert.ok(count <= maxBytes, 'selected file grew beyond bound');
      chunks.push(buffer.subarray(0, bytesRead));
    }
    return Buffer.concat(chunks, count);
  });
}
async function pin(path, expected, maximumBytes, deadline) {
  assert.ok(expected && positive(expected.bytes) && hex(expected.sha256) && expected.bytes <= maximumBytes, 'selected input pin invalid');
  await heldFile(path, expected.bytes, deadline, async (file, before) => {
    assert.equal(before.size, BigInt(expected.bytes), 'selected input size differs');
    const digest = createHash('sha256'); let bytes = 0;
    const buffer = Buffer.allocUnsafe(64 * 1024);
    for (;;) {
      remaining(deadline); const {bytesRead} = await file.read(buffer, 0, Math.min(buffer.length, expected.bytes + 1 - bytes), null);
      if (!bytesRead) break;
      bytes += bytesRead; assert.ok(bytes <= expected.bytes, 'selected input grew'); digest.update(buffer.subarray(0, bytesRead));
    }
    assert.equal(bytes, expected.bytes, 'selected input byte count differs');
    assert.equal(digest.digest('hex'), expected.sha256, 'selected input digest differs');
  });
}
async function completion(request, deadline) {
  const paths = [join(request.worker_root, 'runtime/manifest.json'), join(request.worker_root, 'dist/__edge/build-manifest.json')];
  const packets = [];
  for (const path of paths) packets.push(await boundedRead(path, request.maximum_marker_bytes, deadline));
  assert.ok(packets.every(bytes => bytes.length <= request.maximum_marker_bytes), 'completion marker grew beyond bound');
  assert.deepStrictEqual(packets[0], packets[1], 'public D1 paired completion marker differs');
  assert.equal(sha256(packets[0]), request.completion_marker_sha256, 'public D1 completion marker differs from held native selection');
  const manifest = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(packets[0])); // transport observation only
  assert.equal(manifest.schema, 'tos_cloudflare_edge_build_v1');
  assert.equal(manifest.read_model_schema, 'tos_cloudflare_edge_read_model_v9');
  assert.equal(manifest.data_revision, request.data_revision);
  if (request.expected_delta_from !== undefined) {
    assert.ok(request.profile === 'representative' && hex(request.expected_delta_from), 'delta selection invalid');
    assert.equal(manifest.counts?.delta?.available, true);
    assert.equal(manifest.counts.delta.base_revision, request.expected_delta_from);
    assert.equal(manifest.counts.delta.target_revision, request.data_revision);
  }
  return packets[0];
}
export async function verifyHeldNativeWorker(request, wholeDeadline) {
  // These are transport selection fields. The native callback owner remains
  // responsible for authentic factory/resource admission and delivery leases.
  assert.equal(request.schema, 'tos_native_worker_reader_verification_v1');
  assert.ok(['representative','production'].includes(request.profile));
  assert.ok(positive(request.maximum_seconds) && positive(request.maximum_response_bytes)
    && positive(request.maximum_marker_bytes) && positive(request.maximum_source_bytes));
  assert.ok(request.json_limits && ['maxDepth','maxMembers','maxIntegerDigits'].every(key => positive(request.json_limits[key])), 'explicit native JSON structural bounds required');
  assert.ok(typeof request.source_revision === 'string' && request.source_revision);
  assert.ok(hex(request.data_revision) && hex(request.completion_marker_sha256));
  assert.ok(isAbsolute(request.source_root) && isAbsolute(request.worker_root));
  const ownDeadline = Date.now() + request.maximum_seconds * 1000;
  assert.ok(wholeDeadline === undefined || Number.isSafeInteger(wholeDeadline), 'outer whole deadline invalid');
  const deadline = wholeDeadline === undefined ? ownDeadline : Math.min(wholeDeadline, ownDeadline);
  remaining(deadline);
  assert.ok(Number.isSafeInteger(deadline), 'whole deadline overflow');
  const sourceRoot = await realpath(request.source_root);
  const workerRoot = await realpath(request.worker_root);
  assert.equal(sourceRoot, request.source_root, 'source root must be canonical');
  assert.equal(workerRoot, request.worker_root, 'Worker root must be canonical');
  assert.ok(isAbsolute(request.failure_directory), 'explicit private paired-failure directory required');
  const failureDirectory = await realpath(request.failure_directory);
  const failureStat = await lstat(failureDirectory);
  assert.ok(failureDirectory === request.failure_directory && failureStat.isDirectory() && (failureStat.mode & 0o077) === 0, 'paired-failure directory must be canonical private directory');
  assert.ok(isAbsolute(request.native_binary), 'explicit native comparator binary required');
  assert.ok(positive(request.maximum_native_state_bytes), 'native parser state admission required');
  let sequence = 0;
  const compareRawPackets = async ({label, actual, expected, shape}) => {
    assert.ok(actual.length <= request.maximum_response_bytes && expected.length <= request.maximum_response_bytes, 'paired evidence exceeds admitted response bound');
    const stem = sha256(Buffer.from(label)) + '-' + sequence++;
    const actualPath = join(failureDirectory, stem + '.actual.packet'), expectedPath = join(failureDirectory, stem + '.expected.packet');
    await writeFile(actualPath, actual, {flag: 'wx', mode: 0o600});
    await writeFile(expectedPath, expected, {flag: 'wx', mode: 0o600});
    remaining(deadline);
    const command = ['verify-public-packets', '--actual', actualPath, '--expected', expectedPath,
      '--shape', shape, '--source-root', sourceRoot, '--max-bytes', String(request.maximum_response_bytes),
      '--max-depth', String(request.json_limits.maxDepth), '--max-visits', String(request.json_limits.maxMembers),
      '--max-integer-digits', String(request.json_limits.maxIntegerDigits), '--max-state-bytes', String(request.maximum_native_state_bytes),
      '--max-milliseconds', String(Math.max(1, deadline - Date.now()))];
    const child = spawn(request.native_binary, command, {stdio: ['ignore', 'pipe', 'pipe']});
    const closed = new Promise(resolve => child.once('close', resolve));
    let output = '', diagnostics = '', refusal, stopStarted = false;
    const stop = () => {
      if (stopStarted || !child.pid || child.exitCode !== null || child.signalCode !== null) return;
      stopStarted = true;
      try {child.kill('SIGKILL');} catch (error) {refusal ??= error;}
    };
    const timer = setTimeout(() => {refusal = new Error('native comparator exceeds whole deadline'); stop();}, Math.max(1, deadline - Date.now()));
    child.stdout.on('data', bytes => {output += bytes.toString(); if (Buffer.byteLength(output) > 4096) {refusal = new Error('native comparator receipt exceeds bound'); stop();}});
    child.stderr.on('data', bytes => {diagnostics += bytes.toString(); if (Buffer.byteLength(diagnostics) > 4096) {refusal = new Error('native comparator diagnostics exceed bound'); stop();}});
    try {
      const code = await new Promise((resolve, reject) => {child.once('error', reject); child.once('close', resolve);});
      if (refusal) throw refusal;
      if (code !== 0) throw new Error(`${label}: native comparator failed (${code}): ${diagnostics}`);
      const receipt = JSON.parse(output);
      assert.ok(receipt.schema === 'tos_public_packet_comparison_v1' && receipt.equal === true, 'native comparator receipt invalid');
      remaining(deadline);
      await unlink(actualPath); await unlink(expectedPath); // Only our create-new successful comparison spools.
    } catch (error) {
      await writeFile(join(failureDirectory, stem + '.json'), JSON.stringify({label, shape, actual_bytes: actual.length, actual_sha256: sha256(actual), expected_bytes: expected.length, expected_sha256: sha256(expected), error: String(error).slice(0,4096)}), {flag: 'wx', mode: 0o600});
      throw error; // Actual and expected raw spools stay as bounded private evidence.
    } finally {
      clearTimeout(timer);
      stop();
      await closed; // Native comparator owns no descendants; callback holds through actual pipe close.
    }
  };
  const nativeBase = loopback(request.native_base), workerBase = loopback(request.worker_base);
  assert.notEqual(nativeBase, workerBase, 'native and Worker origins must differ');
  assert.equal(request.source_bindings_scope, 'producer_supplied_observations_only', 'source pins cannot grant completed capture authority');
  const bindings = request.source_bindings;
  assert.ok(bindings && typeof bindings === 'object' && !Array.isArray(bindings) && Object.keys(bindings).length > 0, 'explicit captured input pins required');
  let totalBytes = 0;
  for (const [path, expected] of Object.entries(bindings)) {
    assert.ok(path && !isAbsolute(path) && path.split('/').every(part => part && part !== '.' && part !== '..'), 'source binding path invalid');
    assert.ok(positive(expected?.bytes), 'source binding byte bound required');
    totalBytes += expected.bytes; assert.ok(Number.isSafeInteger(totalBytes) && totalBytes <= request.maximum_source_bytes, 'source pin total exceeds bound');
    assert.equal(await realpath(join(sourceRoot, path)), join(sourceRoot, path), 'source member must be canonical');
  }
  const checkInputs = async () => {for (const [path, expected] of Object.entries(bindings)) await pin(join(sourceRoot, path), expected, request.maximum_source_bytes, deadline);};
  await checkInputs(); const marker = await completion(request, deadline);
  await compareNativePackets({nativeBase, workerBase, profile: request.profile, deadline,
    maximumResponseBytes: request.maximum_response_bytes, dataRevision: request.data_revision,
    sourceRevision: request.source_revision, compareRawPackets});
  await checkInputs(); assert.deepStrictEqual(await completion(request, deadline), marker, 'completion changed during reader verification');
  remaining(deadline);
  return {schema: 'tos_native_worker_reader_verification_receipt_v1', profile: request.profile,
    source_revision: request.source_revision, data_revision: request.data_revision,
    completion_marker_sha256: request.completion_marker_sha256, source_bindings: bindings,
    state: 'reader_transport_packets_equal', source_bindings_scope: request.source_bindings_scope, native_owner_admission: 'external_retained_callback_required'};
}
// Own the actual Core server process. The caller supplies the original native
// request bytes, monotonic cutoff and borrowed stage FD; none is reconstructed.
export async function verifyNativeWorker(request, wholeDeadline) {
  assert.equal(request.schema, 'tos_native_worker_full_verification_v1');
  assert.ok(positive(request.maximum_seconds));
  const startup = request.native_startup;
  const deadline = nodeDeadline(startup?.work_deadline_ns, request.maximum_seconds, wholeDeadline);
  assert.ok(startup && isAbsolute(startup.request_path) && isAbsolute(request.native_binary));
  assert.ok(positive(startup.maximum_request_bytes) && startup.maximum_request_bytes <= 16 * 1024 * 1024);
  assert.ok(positive(startup.maximum_startup_receipt_bytes) && startup.maximum_startup_receipt_bytes <= startup.maximum_log_bytes);
  assert.ok(positive(startup.maximum_log_bytes) && positive(startup.maximum_readiness_milliseconds)
    && positive(startup.maximum_shutdown_milliseconds) && positive(startup.maximum_inherited_fds));
  assert.ok(Number.isSafeInteger(startup.stage_ticket_fd) && startup.stage_ticket_fd >= 3
    && startup.stage_ticket_fd < startup.maximum_inherited_fds, 'actual borrowed stage FD index required');
  fstatSync(startup.stage_ticket_fd); // Presence observation only; native issuer/seals/admission remain authoritative.
  assert.ok(typeof startup.work_deadline_ns === 'string' && /^[0-9]+$/.test(startup.work_deadline_ns), 'original CLOCK_MONOTONIC cutoff string required');
  const raw = await boundedRead(startup.request_path, startup.maximum_request_bytes, deadline);
  assert.equal(sha256(raw), startup.request_sha256, 'native raw DTO differs from owner pin');
  // Observation never becomes a native request. Native strict parser receives
  // the unchanged bytes, preserving duplicates, integer lexemes and all budgets.
  const dto = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(raw), (key, value, context) =>
    key === 'work_deadline_ns' && typeof value === 'number' ? context.source : value);
  assert.equal(dto.admission?.work_deadline_ns, startup.work_deadline_ns, 'native original cutoff differs');
  assert.equal(dto.http?.max_startup_receipt_bytes, startup.maximum_startup_receipt_bytes, 'native startup receipt allowance differs');
  assert.equal(dto.admission?.stage_ticket_fd, startup.stage_ticket_fd, 'native borrowed FD differs');
  const nativeBase = loopback(request.native_base);
  assert.equal(dto.arguments?.listen, nativeBase.slice('http://'.length), 'native listen differs from explicit transport origin');
  assert.equal(dto.arguments?.max_connections, request.profile === 'representative' ? 26 : 43,
    'caller native finite connection count differs from maintained matrix plus one readiness request');
  if ('ABYSS_STAGE_TICKET_FD' in process.env) assert.equal(process.env.ABYSS_STAGE_TICKET_FD, String(startup.stage_ticket_fd));
  for (const key of ['TOS_QUERY_STORE_PATH', 'TOS_RELEASE_ROOT']) assert.ok(!(key in process.env), `ambient ${key} cannot select native oracle`);
  const evidenceDirectory = await realpath(request.failure_directory);
  const stat = await lstat(evidenceDirectory);
  assert.ok(evidenceDirectory === request.failure_directory && stat.isDirectory() && (stat.mode & 0o077) === 0);
  const log = await open(join(evidenceDirectory, 'native-server.log'), constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
  const stdio = ['pipe', 'pipe', 'pipe'];
  while (stdio.length <= startup.stage_ticket_fd) stdio.push('ignore');
  stdio[startup.stage_ticket_fd] = startup.stage_ticket_fd;
  const args = ['--root', request.source_root, 'core-snapshot', '--operation', 'tos_native_serve',
    '--work-deadline-ns', startup.work_deadline_ns];
  let child, closed, refusal, stdoutBytes = 0, stderrBytes = 0, termStarted = false, killStarted = false;
  let killTimer, deadlineTimer, readinessTimer, logWrites = Promise.resolve();
  let receiptChunks = [], receiptBytes = 0, receiptFinished = false, receiptResolve, receiptReject;
  const startupReceipt = new Promise((resolve, reject) => {receiptResolve = resolve; receiptReject = reject;});
  startupReceipt.catch(() => {}); // Terminal close may reject before the awaited readiness branch.
  const receiveReceipt = bytes => {
    if (receiptFinished) return;
    const newline = bytes.indexOf(10), part = newline < 0 ? bytes : bytes.subarray(0, newline);
    receiptBytes += part.length;
    if (receiptBytes + (newline >= 0 ? 1 : 0) > startup.maximum_startup_receipt_bytes) {
      receiptFinished = true; const error = new Error('native startup receipt exceeds admitted bound'); receiptReject(error); cancel(error); return;
    }
    receiptChunks.push(part);
    if (newline >= 0) {
      receiptFinished = true;
      try {
        const receipt = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(Buffer.concat(receiptChunks, receiptBytes)));
        assert.equal(receipt.schema, 'tos_native_core_http_startup_v1');
        assert.ok(typeof receipt.source_revision === 'string' && receipt.source_revision);
        if (request.source_revision !== undefined) assert.equal(receipt.source_revision, request.source_revision, 'selected native capture revision differs');
        assert.ok(Array.isArray(receipt.captured_inputs));
        const inputs = new Map();
        for (const input of receipt.captured_inputs) {
          assert.ok(typeof input.path === 'string' && !inputs.has(input.path), 'native retained input path absent or duplicated');
          assert.ok(hex(input.sha256) && Number.isSafeInteger(input.size_bytes) && input.size_bytes >= 0, 'native retained input observation invalid');
          inputs.set(input.path, input);
        }
        for (const [path, pin] of Object.entries(request.source_bindings ?? {})) {
          const input = inputs.get(path); assert.ok(input, 'selected input absent from native held capture');
          assert.equal(input.sha256, pin.sha256); assert.equal(input.size_bytes, pin.bytes);
        }
        receiptResolve(receipt);
      } catch (error) {receiptReject(error); cancel(error);}
      finally {receiptChunks = [];}
    }
  };
  const running = () => child?.pid && child.exitCode === null && child.signalCode === null;
  const kill = () => {if (!killStarted && running()) {killStarted = true; try {child.kill('SIGKILL');} catch (error) {refusal ??= error;}}};
  const cancel = error => {
    refusal ??= error;
    if (!termStarted && running()) {
      termStarted = true; try {child.kill('SIGTERM');} catch (error) {refusal ??= error;}
      killTimer = setTimeout(kill, Math.max(1, Math.min(startup.maximum_shutdown_milliseconds, deadline - Date.now())));
    }
  };
  const onSignal = () => cancel(new Error('native verifier cancelled'));
  const observe = (bytes, side) => {
    if (side === 'stdout') stdoutBytes += bytes.length; else stderrBytes += bytes.length;
    if (stdoutBytes + stderrBytes > startup.maximum_log_bytes) {cancel(new Error('native server log exceeds admitted bound')); return;}
    // Serial bounded log writes apply backpressure to this transport only.
    child[side].pause();
    logWrites = logWrites.then(async () => {
      try {
      let offset = 0;
      while (offset < bytes.length) {
        remaining(deadline);
        const {bytesWritten} = await log.write(bytes, offset, bytes.length - offset, null);
        assert.ok(Number.isSafeInteger(bytesWritten) && bytesWritten > 0 && bytesWritten <= bytes.length - offset, 'native log write made invalid progress');
        offset += bytesWritten;
      }
      } catch (error) {cancel(error);}
      finally {child[side].resume();} // Drain to EOF even if log writing fails.
    });
  };
  try {
    remaining(deadline);
    const readyDeadline = Math.min(deadline, Date.now() + startup.maximum_readiness_milliseconds);
    child = spawn(request.native_binary, args, {stdio});
    closed = new Promise(resolve => child.once('close', resolve));
    child.on('error', error => {refusal ??= error; receiptReject(error);});
    child.once('close', () => {if (!receiptFinished) receiptReject(new Error('native server closed before held startup receipt'));});
    child.stdin.on('error', error => cancel(error));
    child.stdout.on('data', bytes => {receiveReceipt(bytes); observe(bytes, 'stdout');});
    child.stderr.on('data', bytes => observe(bytes, 'stderr'));
    process.on('SIGTERM', onSignal); process.on('SIGINT', onSignal);
    deadlineTimer = setTimeout(() => cancel(new Error('native server exceeds original verifier deadline')), Math.max(1, deadline - Date.now()));
    readinessTimer = setTimeout(() => {
      const error = new Error('native startup exceeds original readiness deadline');
      receiptReject(error); cancel(error);
    }, Math.max(1, readyDeadline - Date.now()));
    child.stdin.end(raw); // EXACT raw request; no JSON.stringify DTO and no synthesized admission.
    const heldStartup = await startupReceipt;
    remaining(readyDeadline);
    for (;;) {
      remaining(readyDeadline);
      if (refusal) throw refusal;
      if (!running()) throw new Error('native server exited before readiness');
      try {
        const response = await fetch(nativeBase + '/health', {redirect: 'error', signal: AbortSignal.timeout(Math.max(1, readyDeadline - Date.now()))});
        if (response.status !== 200 || !response.body) throw new Error('native accepted readiness request failed');
        const reader = response.body.getReader(); let chunks = [], size = 0;
        try {for (;;) {remaining(readyDeadline); const {done,value} = await reader.read(); if (done) break; size += value.byteLength;
          assert.ok(size <= request.maximum_response_bytes, 'native readiness packet exceeds bound'); chunks.push(value);}}
        finally {await reader.cancel();}
        const health = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(Buffer.concat(chunks,size)));
        assert.equal(health.ok, true); break; // Exactly ONE accepted readiness request.
      } catch (error) {
        // Only refused TCP before the listener can retry. Accepted timeout,
        // non-200, malformed packet or any other transport error is a refusal.
        if (error.cause?.code !== 'ECONNREFUSED') throw error;
        await new Promise(resolve => setTimeout(resolve, Math.max(1, Math.min(50, readyDeadline - Date.now()))));
      }
    }
    remaining(readyDeadline);
    clearTimeout(readinessTimer);
    const receipt = await verifyHeldNativeWorker({...request, source_revision: heldStartup.source_revision, schema: 'tos_native_worker_reader_verification_v1'}, deadline);
    const code = await closed; // Native final callback/model/capture/evidence fences must naturally finish.
    if (refusal) throw refusal;
    assert.equal(code, 0, 'native held server final owner fence failed'); remaining(deadline);
    return {...receipt, native_server: 'owned_exact_DTO_normal_finite_count_exit0', native_startup_receipt: heldStartup};
  } catch (error) {
    cancel(error); throw error;
  } finally {
    clearTimeout(readinessTimer);
    clearTimeout(deadlineTimer);
    if (running()) cancel(new Error('native verifier scope closed before server completion'));
    if (closed) await closed; // Do not release lifecycle scope before actual child close.
    clearTimeout(killTimer);
    process.removeListener('SIGTERM', onSignal); process.removeListener('SIGINT', onSignal);
    await logWrites;
    await log.close();
  }
}
export async function main(args = process.argv.slice(2)) {
  assert.ok(args.length === 8 && args[0] === '--max-seconds' && /^[0-9]+$/.test(args[1]) && positive(Number(args[1]))
    && args[2] === '--request' && isAbsolute(args[3]) && args[4] === '--work-deadline-ns' && /^[0-9]+$/.test(args[5])
    && args[6] === '--worker-base-url', 'usage: verify_native_runtime.mjs --max-seconds N --request ABS_JSON --work-deadline-ns ORIGINAL_NS --worker-base-url LOOPBACK_ORIGIN');
  const maximumSeconds = Number(args[1]);
  // Preserve the original Linux CLOCK_MONOTONIC cutoff before any I/O.
  const deadline = nodeDeadline(args[5], maximumSeconds);
  for (const key of ['TOS_QUERY_STORE_PATH', 'TOS_RELEASE_ROOT']) assert.ok(!(key in process.env), `ambient ${key} cannot select native oracle`);
  const raw = await boundedRead(args[3], 64 * 1024, deadline);
  const request = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(raw)); // transport recipe only, never admission
  assert.equal(request.native_startup?.work_deadline_ns, args[5], 'wrapper original cutoff differs from native recipe');
  assert.equal(loopback(request.worker_base), loopback(args[7]), 'wrapper Worker origin differs from recipe');
  assert.ok(positive(request.maximum_seconds) && request.maximum_seconds <= maximumSeconds, 'request whole budget exceeds explicit CLI budget');
  remaining(deadline);
  const receipt = await verifyNativeWorker(request, deadline);
  console.log(JSON.stringify(receipt));
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch(error => {console.error(error.message); process.exitCode = 1;});
