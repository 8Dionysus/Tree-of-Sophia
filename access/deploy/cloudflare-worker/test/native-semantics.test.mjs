import {frozenReference} from './frozen-reference.mjs';
import assert from 'node:assert/strict';
import test from 'node:test';
import {createHash} from 'node:crypto';
import {parseNativeJson, nativeField, nativeChild, nativeNumberInfo,
  pythonStr, nativeJson, NativeContextLost, NativeBudgetExceeded, nativeLower} from '../../../shared/native-semantics.ts';
import {codePointCompare, nativeUnicodeVersion} from '../../../shared/native-unicode.ts';

function reference(referenceId, input) {
  return JSON.parse(frozenReference(import.meta.url, referenceId, input === undefined ? undefined : JSON.stringify(input)));
}

test('native sidecar preserves Python strings, order and numeric kinds', () => {
  const fixture = reference("semantics/scalars");
  for (const item of fixture) {
    const ref = parseNativeJson(item.raw);
    assert.equal(pythonStr(ref), item.str, item.raw);
  }
});

test('shortest float formatting matches Python on exact dyadic ties and exponent boundaries', () => {
  const fixture = reference("semantics/float-boundaries");
  assert.equal(fixture.length, 1471);
  for (const {raw, expected} of fixture) assert.equal(pythonStr(parseNativeJson(raw)), expected, raw);
});

test('all Unicode scalar lowercase matches pinned Python16 by digest', () => {
  const expected = reference("semantics/unicode-digests");
  const lower = createHash('sha256');
  for (let start=0;start<0x110000;start+=4096) {
    let text='';
    for(let cp=start;cp<Math.min(start+4096,0x110000);cp++) if(cp<0xd800||cp>0xdfff) {
      text+=String.fromCodePoint(cp);
    }
    lower.update(nativeLower(text));
  }
  assert.equal(lower.digest('hex'),expected.lower);
  assert.equal(nativeUnicodeVersion,'16.0.0');
  assert.throws(()=>nativeLower('A','17.0.0'),/incompatible/);
});

test('Unicode contextual final sigma and code-point ordering match Python', () => {
  const inputs=['ΟΣ','ΟΣΑ','Σ','AΣ','AΣ\u0301','AΣ\u0301B','AΣ\u0345B','AΣ\u0345',
    'İSTANBUL','СОФИЯ','\u{10400}\u{10428}','AΣ'+"'".repeat(10000),'AΣ'+"'".repeat(10000)+'B'];
  const expected=reference("semantics/contextual-lower",inputs);
  assert.deepEqual(inputs.map(s=>nativeLower(s)),expected);
  assert.equal(nativeLower('\ud800A'), '\ud800a');
  assert.equal(codePointCompare('\ue000','\u{10000}'),-1);
  assert.equal(codePointCompare('a','A'),1);
  assert.equal(codePointCompare('a','aa'),-1);
});

test('native JSON sidecar retains original bytes of numbers and dictionary key order', () => {
  const raw='{"10":1.0,"2":9007199254740993,"nested":{"2":-0.0,"1":[1e-05,true,null]}}';
  const ref=parseNativeJson(raw);
  assert.deepEqual(Object.keys(ref.value),['2','10','nested']); // source value stays ordinary JS.
  assert.equal(nativeJson(ref),raw);
  assert.equal(nativeNumberInfo(nativeChild(ref,'2')).lexeme,'9007199254740993');
  assert.equal(pythonStr(nativeField(ref,'nested.2')),'-0.0');
  assert.equal(pythonStr(nativeField(ref,'absent')),'None');
  const before=JSON.stringify(ref.value);
  assert.deepEqual(Object.keys(ref),['value']);
  assert.deepEqual(Object.keys(nativeChild(ref,'2')),['value']);
  nativeJson(ref);
  assert.equal(JSON.stringify(ref.value),before);
  assert.throws(()=>pythonStr({...ref,value:structuredClone(ref.value)}),NativeContextLost);
  assert.throws(()=>pythonStr({value:1,context:null}),NativeContextLost);
  assert.equal(pythonStr(parseNativeJson('1e0')),'1.0');
  assert.equal(pythonStr(parseNativeJson('1E00020')),'1e+20');
});

test('native JSON and callback work stays bounded and rejects malformed evidence', () => {
  assert.throws(()=>parseNativeJson('{"a":1,"a":2}'),/duplicate/);
  assert.throws(()=>parseNativeJson('1e999'),/nonfinite/);
  assert.throws(()=>parseNativeJson('[0]',{maxBytes:2}),NativeBudgetExceeded);
  assert.throws(()=>parseNativeJson('[[0]]',{maxDepth:1}),NativeBudgetExceeded);
  assert.throws(()=>parseNativeJson('[0,1]',{maxMembers:2}),NativeBudgetExceeded);
  assert.throws(()=>parseNativeJson('123',{maxIntegerDigits:2}),NativeBudgetExceeded);
  assert.throws(()=>parseNativeJson('[]',{maxBytes:0}),TypeError);
  assert.throws(()=>pythonStr(parseNativeJson('[1,2]'),3),NativeBudgetExceeded);
});
