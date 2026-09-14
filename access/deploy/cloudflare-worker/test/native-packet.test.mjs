import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {parseNativeJson, nativeChild, nativeField, nativeJson, nativeNumberInfo, isNativeRef,
  nativePacketArray, nativePacketObject, nativePacketJson, nativeInteger, nativeFloat,
  NativeContextLost, NativeBudgetExceeded} from '../../../shared/native-semantics.ts';

function python(code, input) {
  return JSON.parse(execFileSync('python3', ['-B', '-c', code], {
    input, encoding: 'utf8', timeout: 30000, maxBuffer: 4 * 1024 * 1024,
  }));
}

const shapeCode = String.raw`
def shape(value):
    if isinstance(value,dict): return {'type':'dict','keys':list(value),'values':[shape(v) for v in value.values()]}
    if isinstance(value,list): return {'type':'list','values':[shape(v) for v in value]}
    return {'type':type(value).__name__,'repr':repr(value)}
`;

test('mixed packet round-trips actual Python source values, numeric kinds and ordered keys', () => {
  const fixture = python(String.raw`
import json
` + shapeCode + String.raw`
source={'10':1.0,'2':2**53+1,'__proto__':{'constructor':-0.0},
    'constructor':[False,0,None,{'2':1e-5,'1':10**400}],'unicode':'İ 😀'}
packet={'10':source,'2':source,'__proto__':source['__proto__'],
    'constructor':[source['2'],source['10'],source['constructor']],
    'derived':{'count':2,'float_count':2.0,'negative_zero':-0.0,'large':10**50}}
raw=lambda value:json.dumps(value,ensure_ascii=False,separators=(',',':'))
print(json.dumps({'source':raw(source),'packet':raw(packet),'shape':shape(packet)}))
`);
  const source = parseNativeJson(fixture.source);
  const packet = nativePacketObject([
    ['10', source], ['2', source], ['__proto__', nativeChild(source, '__proto__')],
    ['constructor', nativePacketArray([nativeChild(source, '2'), nativeChild(source, '10'), nativeChild(source, 'constructor')])],
    ['derived', nativePacketObject([
      ['count', nativeInteger(2)], ['float_count', nativeFloat(2)],
      ['negative_zero', nativeFloat(-0)], ['large', nativeInteger(10n ** 50n)],
    ])],
  ]);
  const output = nativePacketJson(packet);
  assert.equal(output, fixture.packet); // actual Python compact JSON, not a JS stringify oracle.
  const decodedShape = python('import json,sys\n' + shapeCode + '\nprint(json.dumps(shape(json.load(sys.stdin))))', output);
  assert.deepEqual(decodedShape, fixture.shape);
  assert.equal(nativeJson(source), fixture.source);
  assert.equal(nativeNumberInfo(nativeField(source, '__proto__.constructor')).lexeme, '-0.0');
});

test('source references retain noncanonical number lexemes through nested and repeated composition', () => {
  const source = parseNativeJson('{"02":1E+000,"2":-0.0,"1":9007199254740993,"__proto__":1e-005}');
  const packet = nativePacketArray([source, nativePacketObject([['nested', source], ['number', nativeChild(source, '02')]])]);
  assert.equal(nativePacketJson(packet), '[' + nativeJson(source) + ',{"nested":' + nativeJson(source) + ',"number":1E+000}]');
  assert.equal(nativePacketJson(nativeChild(source, '1')), '9007199254740993');
  assert.equal(nativePacketJson('😀'), '"😀"');
  assert.equal(nativePacketJson(null), 'null');
});

test('decoded source and exposed metadata are immutable without altering source values', () => {
  const raw = '{"10":1.0,"2":{"__proto__":"before","values":[9007199254740993,-0.0]}}';
  const source = parseNativeJson(raw);
  assert.ok(Object.isFrozen(source));
  assert.ok(Object.isFrozen(source.value));
  assert.ok(Object.isFrozen(source.value['2']));
  assert.ok(Object.isFrozen(source.value['2'].values));
  assert.throws(() => {source.value['2']['__proto__'] = 'after';}, TypeError);
  assert.throws(() => {source.value['10'] = 1;}, TypeError);
  assert.throws(() => {source.value['2'].values.push(1);}, TypeError);
  assert.throws(() => {delete source.value['2'];}, TypeError);
  assert.throws(() => {nativeNumberInfo(nativeChild(source, '10')).lexeme = '2';}, TypeError);
  assert.ok(Object.isFrozen(source.context.lookup(source.value).keys));
  assert.equal(source.context.lookup(source.value).numbers.set, undefined);
  assert.equal(nativeJson(source), raw);
  const entries = [['before', source]];
  const packet = nativePacketObject(entries);
  entries[0][0] = 'after'; entries.push(['extra', null]);
  assert.equal(nativePacketJson(packet), '{"before":' + raw + '}');
  assert.ok(Object.isFrozen(packet.entries[0]));
});

test('lost references, implicit values, malformed members and cyclic data fail closed', () => {
  const source = parseNativeJson('{"a":1.0}');
  for (const value of [source.value, {...source}, structuredClone(source), {value: 1, context: null},
    1, undefined, 1n, [], {}, () => 1]) {
    assert.equal(isNativeRef(value), false);
    assert.throws(() => nativePacketJson(value), NativeContextLost);
  }
  assert.throws(() => nativePacketArray([source.value]), NativeContextLost);
  assert.throws(() => nativePacketObject([['a', null], ['a', true]]), /duplicate/);
  assert.throws(() => nativePacketObject([[1, null]]), /malformed/);
  assert.throws(() => nativePacketObject([['a']]), /malformed/);
  assert.throws(() => nativePacketObject([['a', null, true]]), /malformed/);
  assert.throws(() => nativePacketObject({a: source}), TypeError);
  assert.throws(() => nativePacketArray([,]), NativeContextLost);
  const cycle = []; cycle.push(cycle);
  assert.throws(() => nativePacketArray(cycle), NativeContextLost);
  assert.throws(() => nativePacketJson(cycle), NativeContextLost);
  const objectCycle = {}; objectCycle.self = objectCycle;
  assert.throws(() => nativePacketObject([['cycle', objectCycle]]), NativeContextLost);
  const safe = nativePacketArray([]);
  assert.throws(() => safe.items.push(safe), TypeError);
  for (const raw of ['{"a":1,"\\u0061":2}', '{"nested":{"a":1,"a":2}}', '[1,]', '{"a":}', '01']) {
    assert.throws(() => parseNativeJson(raw), raw);
  }
});

test('derived numbers require explicit safe integer or finite float kinds', () => {
  assert.equal(nativePacketJson(nativeInteger(-2)), '-2');
  assert.equal(nativePacketJson(nativeInteger(9007199254740993n)), '9007199254740993');
  assert.equal(nativePacketJson(nativeFloat(2)), '2.0');
  assert.equal(nativePacketJson(nativeFloat(-0)), '-0.0');
  for (const value of [9007199254740992, 1.5, -0, Infinity, NaN, '1']) assert.throws(() => nativeInteger(value), TypeError);
  for (const value of [NaN, Infinity, -Infinity, 1n, '1']) assert.throws(() => nativeFloat(value), TypeError);
  assert.throws(() => nativeInteger(10n ** 4300n), NativeBudgetExceeded);
});

test('complete packet shares aggregate UTF-8, visit, depth and member budgets', () => {
  const source = parseNativeJson('["😀",1.0]');
  const packet = nativePacketArray([source, source]);
  const output = nativePacketJson(packet);
  const bytes = Buffer.byteLength(output);
  assert.equal(nativePacketJson(packet, {maxBytes: bytes, maxVisits: 7, maxDepth: 2}), output);
  assert.throws(() => nativePacketJson(packet, {maxBytes: bytes - 1}), NativeBudgetExceeded);
  assert.throws(() => nativePacketJson(packet, {maxVisits: 6}), NativeBudgetExceeded);
  assert.throws(() => nativePacketJson(packet, {maxDepth: 1}), NativeBudgetExceeded);
  assert.throws(() => nativePacketJson('😀', {maxBytes: 5}), NativeBudgetExceeded);
  assert.equal(nativePacketJson('😀', {maxBytes: 6}), '"😀"');
  assert.throws(() => nativePacketJson('\ud800', {maxBytes: 7}), NativeBudgetExceeded);
  assert.equal(nativePacketJson('\ud800', {maxBytes: 8}), '"\\ud800"');
  assert.throws(() => nativePacketJson(nativePacketObject([['😀', null]]), {maxBytes: 11}), NativeBudgetExceeded);
  assert.throws(() => nativePacketJson(packet, {maxVisits: 0}), TypeError);
  assert.throws(() => nativePacketArray([null, null], 1), NativeBudgetExceeded);
  assert.throws(() => nativePacketObject([['a', null], ['b', null]], 1), NativeBudgetExceeded);
});
