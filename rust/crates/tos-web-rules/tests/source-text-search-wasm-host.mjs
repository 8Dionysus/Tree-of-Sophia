#!/usr/bin/env node
// Exercise the shared QRY source-text kernel through the generated WASM
// binding with exact selected-text values from the maintained Nietzsche pair.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';

const [bindingPath, wasmPath, nativeHostPath] = process.argv.slice(2);
if (!bindingPath || !wasmPath || !nativeHostPath) {
  throw new Error('usage: node source-text-search-wasm-host.mjs BINDING.mjs MODULE_bg.wasm NATIVE_HOST');
}
const binding = await import(pathToFileURL(bindingPath).href);
await binding.default({module_or_path: await readFile(wasmPath)});
const encoder = new TextEncoder();
const decoder = new TextDecoder();
let parityCases = 0;
function executePair(request, selection, candidates) {
  // Serialize once and feed the exact same three byte strings to both hosts.
  const requestBytes = Buffer.from(encoder.encode(JSON.stringify(request)));
  const selectionBytes = Buffer.from(encoder.encode(JSON.stringify(selection)));
  const candidateBytes = Buffer.from(encoder.encode(JSON.stringify(candidates)));
  let wasmOutcome;
  try {
    wasmOutcome = {
      kind: 'ok',
      bytes: Buffer.from(binding.source_text_search_wasm_v1(requestBytes, selectionBytes, candidateBytes)),
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const code = message.match(/\b(invalid_request|budget_exceeded|stale_cursor|corrupt_candidate|cancelled|deadline_exceeded)\b/)?.[1];
    assert.ok(code, `WASM returned an unclassified search error: ${message}`);
    wasmOutcome = {kind: 'error', code, message};
  }

  const nativeInput = Buffer.concat([
    requestBytes, Buffer.from('\n'), selectionBytes, Buffer.from('\n'), candidateBytes, Buffer.from('\n'),
  ]);
  const native = spawnSync(nativeHostPath, [], {input: nativeInput, maxBuffer: 9 * 1024 * 1024});
  assert.equal(native.error, undefined, `native parity host failed to start: ${native.error?.message}`);
  assert.equal(native.status, 0, `native parity host failed: ${decoder.decode(native.stderr ?? Buffer.alloc(0))}`);
  const nativeBytes = Buffer.from(native.stdout ?? Buffer.alloc(0));
  const nativeText = decoder.decode(nativeBytes);
  const nativeError = nativeText.match(/^error:([a-z_]+):/)?.[1];
  const nativeOutcome = nativeError
    ? {kind: 'error', code: nativeError}
    : {kind: 'ok', bytes: nativeBytes};
  assert.equal(nativeOutcome.kind, wasmOutcome.kind, 'native and WASM outcome kinds diverged');
  if (wasmOutcome.kind === 'error') {
    assert.equal(nativeOutcome.code, wasmOutcome.code, 'native and WASM error codes diverged');
  } else {
    assert.deepEqual(nativeOutcome.bytes, wasmOutcome.bytes, 'native and WASM canonical JSON bytes diverged');
  }
  parityCases += 1;
  if (wasmOutcome.kind === 'error') {
    const error = new Error(wasmOutcome.message);
    error.code = wasmOutcome.code;
    throw error;
  }
  return JSON.parse(decoder.decode(wasmOutcome.bytes));
}

const agent = JSON.parse(await readFile(new URL('../../../../ToS/source-witnesses/agents/friedrich-nietzsche/agent.json', import.meta.url)));
const forms = JSON.parse(await readFile(new URL('../../../../ToS/source-witnesses/agents/friedrich-nietzsche/agent.human-forms.json', import.meta.url)));
const sourcePath = 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.human-forms.json';
const agentPath = 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json';
const expectedText = new Map([
  ['/preferred_label', agent.preferred_label],
  ['/notes', agent.notes],
  ['/variant_labels/0/value', agent.variant_labels[0].value],
]);
assert.equal(agent.schema_version, 'tos_corpus_record_v1');
assert.equal(agent.record_id, 'tos.agent.friedrich-nietzsche');
assert.equal(forms.schema_version, 'tos_human_form_set_v1');
assert.equal(forms.forms.length, 3);
const documents = forms.forms.map((form, index) => {
  const wording = form.bindings.wording;
  const text = expectedText.get(wording.pointer);
  assert.equal(form.content.kind, 'source-copy');
  assert.equal(wording.record.id, agent.record_id);
  assert.equal(wording.record.version, agent.record_version);
  assert.equal(wording.record.digest, 'sha256:8d6034511d8fb7618c7a97ad4cc07f84c2fecc2c5792786168d00a0796495a48');
  assert.equal(typeof text, 'string');
  assert.equal(form.subject.digest, wording.record.digest);
  assert.equal(form.subject.version, agent.record_version);
  return {
    document_type: `human_form/${form.role}`,
    source_path: sourcePath,
    custody_revision: `fixture-source-${index + 1}`,
    document_id: form.form_id,
    selection_seq: 7,
    identity: form.form_id,
    title: form.role === 'name' ? text : '',
    aliases: [],
    body: form.role === 'name' ? '' : text,
  };
});
assert.deepEqual(documents.map(document => document.document_id), forms.forms.map(form => form.form_id));
assert.equal(documents[0].title, agent.preferred_label);
assert.equal(documents[1].body, agent.notes);
assert.equal(documents[2].title, agent.variant_labels[0].value);
assert.ok(forms.forms.every(form => form.bindings.wording.record.id === agent.record_id));
assert.ok(forms.forms.every(form => form.bindings.wording.record.digest === forms.forms[0].subject.digest));

const selection = {
  consumer_id: 'portable-wasm-fixture',
  domain: 'tos-source-text-parity',
  consumer_cut: 'fixture-cut-7',
  profile_id: 'tos_source_selected_text_v2',
  profile_sha256: 'a'.repeat(64),
  selection_seq: 7,
};
const candidates = {complete: true, documents};
const russian = executePair({query: ' ФРИДРИХ ', limit: 5, document_types: ['human_form/name'], source_paths: [sourcePath]}, selection, candidates);
assert.equal(russian.operation_profile, 'tos_source_text_search_v1');
assert.equal(russian.query, 'фридрих');
assert.deepEqual(russian.selection, selection);
assert.deepEqual(russian.hits.map(hit => [hit.document_id, hit.rank, hit.excerpt_field, hit.excerpt]), [
  [forms.forms[2].form_id, 2, 'title', agent.variant_labels[0].value],
]);
assert.equal(russian.has_more, false);
assert.equal(russian.next_cursor, null);

const first = executePair({query: 'i', limit: 1}, selection, candidates);
assert.equal(first.hits[0].document_id, forms.forms[0].form_id);
assert.equal(first.has_more, true);
assert.equal(typeof first.next_cursor, 'string');
const next = executePair({query: 'i', limit: 1, cursor: first.next_cursor}, selection, candidates);
assert.equal(next.hits[0].document_id, forms.forms[1].form_id);
assert.equal(next.hits[0].rank, 5);
assert.deepEqual(next.selection, selection);
assert.equal(next.has_more, false);
assert.equal(next.next_cursor, null);

assert.throws(() => executePair({query: 'nietzsche', limit: 5}, selection, {...candidates, complete: false}), /budget exceeded/);
const changedSelection = {...selection, profile_sha256: 'b'.repeat(64)};
assert.throws(() => executePair({query: 'i', limit: 1, cursor: first.next_cursor}, changedSelection, candidates), /continuation/);

console.log(JSON.stringify({status: 'pass', native_wasm_same_input_cases: parityCases, source_documents: documents.length,
  corpus_source_paths: [agentPath, sourcePath], live_owner_currentness: false}));
