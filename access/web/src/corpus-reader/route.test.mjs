import {test} from 'vitest';
import assert from 'node:assert/strict';
import {encodeReadingRoute,decodeReadingRoute} from './route.mjs';
import {explorationScope,scopeText} from './scope.mjs';
test('exact Unicode locators round trip without copying notes or source text to URL',()=>{
  const address={documentId:'tos:work:источник',versionId:'de:1893',unitId:'unit:§10',revision:'a'.repeat(64)};
  const hash=encodeReadingRoute({...address,quote:'private quotation',note:'personal note',anchor:{text:'payload'}});
  assert.deepEqual(decodeReadingRoute(hash),address);
  assert.ok(!hash.includes('private'));assert.ok(!hash.includes('personal'));assert.ok(!hash.includes('payload'));
});
test('malformed addresses do not quietly become a different text version',()=>{
  assert.equal(decodeReadingRoute('#other'),null);
  for(const hash of ['#reading=%bad','#reading=%7B%7D','#reading='+ 'a'.repeat(14001)])assert.throws(()=>decodeReadingRoute(hash));
  assert.throws(()=>encodeReadingRoute({documentId:'x',versionId:'v',unitId:'u',revision:''}));
});
test('scope keeps source rows, folded display and continuation state distinct',()=>{
  const state={view:{nodes:[{},{},{}],relations:[{},{}],continuation:{next_cursor:'next'}},model:{vertices:[{}],edges:[{}]},mode:'compact'};
  const scope=explorationScope(state);assert.equal(scope.loadedNodes,3);assert.equal(scope.shownNodes,1);assert.equal(scope.hasMore,true);
  assert.equal(scope.corpusTotal,undefined);assert.match(scopeText(scope,'en'),/More material is available/);
  assert.match(scopeText(scope,'ru'),/Загружено/);assert.doesNotMatch(scopeText(scope,'ru'),/общий размер|недоступна/);
  state.view.continuation.next_cursor=null;assert.equal(explorationScope(state).hasMore,false);assert.doesNotMatch(scopeText(explorationScope(state),'en'),/No next page|corpus total/);
});
