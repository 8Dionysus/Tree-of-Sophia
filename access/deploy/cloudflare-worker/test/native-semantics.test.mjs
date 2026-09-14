import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {parseNativeJson, nativeScalar, nativeField, nativeChild, nativeNumberInfo,
  pythonTruthy, pythonEquals, pythonMember, pythonStr, pythonRepr, nativeSortKey,
  nativeJson, NativeContextLost, NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {nativeLower, nativeIsPrintable, codePointCompare, nativeUnicodeVersion} from '../../../shared/native-unicode.ts';

function python(code, input) {
  return JSON.parse(execFileSync('python3', ['-B', '-c', code], {input: input === undefined ? undefined : JSON.stringify(input),
    encoding:'utf8',timeout:30000,maxBuffer:16*1024*1024}));
}

test('native sidecar preserves Python strings, repr, truthiness, order and numbers', () => {
  const fixture = python(String.raw`
import json, math, random, struct
values = [None, False, True, 0, 1, 1.0, -0.0, 1e-5, 1e-4, 1e15, 1e16, 1e20,
    2**53+1, 2**53, 10**100, 10**400, [], {}, [False,1.0,None,{'10':'ten','2':'two'}],
    {'10':'ten','2':'two','01':1.0,'nested':{'z':False,'a':1}}, '', 'foo', "don't", 'say "hi"',
    '''both ' "''', ''.join(chr(c) for c in range(256)), '\u2028\ue000\U00010000\U0001f600\u0378']
rng=random.Random(20260912)
for _ in range(5000):
    value=struct.unpack('>d',rng.getrandbits(64).to_bytes(8,'big'))[0]
    if math.isfinite(value): values.append(value)
print(json.dumps([{'raw':json.dumps(v,ensure_ascii=True,separators=(',',':')),
    'str':str(v),'repr':repr(v),'truthy':bool(v),'key':str(v or '').lower()}
    for v in values],ensure_ascii=True))
`);
  for (const item of fixture) {
    const ref = parseNativeJson(item.raw);
    assert.equal(pythonStr(ref), item.str, item.raw);
    assert.equal(pythonRepr(ref), item.repr, item.raw);
    assert.equal(pythonTruthy(ref), item.truthy, item.raw);
    assert.equal(nativeSortKey(ref), item.key, item.raw);
  }
});

test('native equality and membership retain exact integer/float and nested-value semantics', () => {
  const fixture = python(String.raw`
import json
values=[False,0,0.0,True,1,1.0,9007199254740992,9007199254740993,9007199254740992.0,
    10**100,10**400,1e100,None,'0',[],[False,1.0],{'a':False,'2':2},{'2':2.0,'a':0}]
raw=lambda x:json.dumps(x,separators=(',',':'))
pairs=[{'a':raw(a),'b':raw(b),'equal':a==b} for a in values for b in values]
members=[{'a':raw(a),'b':raw(values),'member':a in values} for a in values+[{},'missing',[True]]]
print(json.dumps({'pairs':pairs,'members':members}))
`);
  for (const {a,b,equal} of fixture.pairs) assert.equal(pythonEquals(parseNativeJson(a),parseNativeJson(b)),equal,a+' / '+b);
  for (const {a,b,member} of fixture.members) assert.equal(pythonMember(parseNativeJson(a),parseNativeJson(b)),member,a+' in '+b);
  assert.equal(pythonMember(nativeScalar('a'),parseNativeJson('{"a":1}')),true);
  assert.equal(pythonMember(nativeScalar('😀'),nativeScalar('x😀y')),true);
  assert.throws(()=>pythonMember(parseNativeJson('[]'),parseNativeJson('{}')),/unhashable/);
});

test('shortest float formatting matches Python on exact dyadic ties and exponent boundaries', () => {
  const fixture = python(String.raw`
import json, math, sys
values=[]
for numerator in range(26215,26501,2):
    for shift in [-20,-10,0,10,20]:
        value=math.ldexp(numerator/2**18,shift)
        values.extend([value,-value])
for exponent in [-323,-308,-100,-5,-4,-3,0,15,16,17,100,308]:
    value=10.0**exponent
    values.extend([math.nextafter(value,0.0),value,math.nextafter(value,math.inf)])
values.extend([sys.float_info.max,sys.float_info.min,math.ulp(0.0),-math.ulp(0.0),-0.0])
print(json.dumps([{'raw':repr(v),'expected':repr(v)} for v in values]))
`);
  assert.equal(fixture.length, 1471);
  for (const {raw, expected} of fixture) assert.equal(pythonStr(parseNativeJson(raw)), expected, raw);
});

test('all Unicode scalar lower and printable behavior matches pinned Python16 by digest', () => {
  const expected = python(String.raw`
import hashlib,json,unicodedata
assert unicodedata.unidata_version=='16.0.0'
h=hashlib.sha256(); p=hashlib.sha256()
for start in range(0,0x110000,4096):
    text=''.join(chr(c) for c in range(start,min(start+4096,0x110000)) if not 0xd800<=c<=0xdfff)
    h.update(text.lower().encode('utf-8'))
    p.update(bytes(int(c.isprintable()) for c in text))
print(json.dumps({'lower':h.hexdigest(),'printable':p.hexdigest()}))
`);
  const lower = createHash('sha256'), printable = createHash('sha256');
  for (let start=0;start<0x110000;start+=4096) {
    let text='';const bits=[];
    for(let cp=start;cp<Math.min(start+4096,0x110000);cp++) if(cp<0xd800||cp>0xdfff) {
      text+=String.fromCodePoint(cp);bits.push(Number(nativeIsPrintable(cp)));
    }
    lower.update(nativeLower(text));printable.update(Uint8Array.from(bits));
  }
  assert.equal(lower.digest('hex'),expected.lower);
  assert.equal(printable.digest('hex'),expected.printable);
  assert.equal(nativeUnicodeVersion,'16.0.0');
  assert.throws(()=>nativeLower('A','17.0.0'),/incompatible/);
});

test('Unicode contextual final sigma and code-point ordering match Python', () => {
  const inputs=['ΟΣ','ΟΣΑ','Σ','AΣ','AΣ\u0301','AΣ\u0301B','AΣ\u0345B','AΣ\u0345',
    'İSTANBUL','СОФИЯ','\u{10400}\u{10428}','AΣ'+"'".repeat(10000),'AΣ'+"'".repeat(10000)+'B'];
  const expected=python('import json,sys; print(json.dumps([s.lower() for s in json.load(sys.stdin)]))',inputs);
  assert.deepEqual(inputs.map(s=>nativeLower(s)),expected);
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
  pythonRepr(ref);nativeJson(ref);
  assert.equal(JSON.stringify(ref.value),before);
  assert.throws(()=>pythonStr({...ref,value:structuredClone(ref.value)}),NativeContextLost);
  assert.throws(()=>pythonStr({value:1,context:null}),NativeContextLost);
  assert.throws(()=>nativeScalar(1),NativeContextLost);
  assert.equal(pythonEquals(parseNativeJson('9007199254740993'),parseNativeJson('9007199254740992')),false);
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
  assert.throws(()=>pythonEquals(parseNativeJson('[1,2]'),parseNativeJson('[1,2]'),1),NativeBudgetExceeded);
  assert.throws(()=>pythonMember(parseNativeJson('3'),parseNativeJson('[1,2,3]'),1),NativeBudgetExceeded);
});
