import assert from 'node:assert/strict';
import {describe,it} from 'vitest';
import {readCorpusFragmentRange} from './corpus-fragment-range.mjs';

const work={documentId:'work'},version={versionId:'version',revision:'pinned'};
const unit=id=>({id,text:id});
const page=(ids,nextCursor=null,revision='pinned')=>({documentId:'work',versionId:'version',revision,units:ids.map(unit),nextCursor});
function provider(pages){
  const calls=[];
  return {calls,window:async args=>{calls.push(args);return pages[calls.length-1];}};
}

describe('exact, bounded corpus fragment ranges',()=>{
  it('pins every page and returns the complete requested endpoints',async()=>{
    const p=provider([page(['a','b'],'next'),page(['c','d','after'])]);
    assert.deepEqual((await readCorpusFragmentRange(p,work,version,'a','d')).map(u=>u.id),['a','b','c','d']);
    assert.equal(p.calls.length,2);
    assert.ok(p.calls.every(call=>call.revision==='pinned'&&call.limit===8));
    assert.equal(p.calls[0].unitId,'a');assert.equal(p.calls[1].cursor,'next');
  });
  it('rejects changed text even when the unit IDs still match',async()=>{
    await assert.rejects(readCorpusFragmentRange(provider([page(['a'],null,'new')]),work,version,'a'),/text layer changed/);
  });
  it('rejects a revision change between windows',async()=>{
    await assert.rejects(readCorpusFragmentRange(provider([page(['a'],'next'),page(['b'],null,'new')]),work,version,'a','b'),/text layer changed/);
  });
  it('does not turn an incomplete prefix into a complete quotation',async()=>{
    await assert.rejects(readCorpusFragmentRange(provider([page(['a'])]),work,version,'a','missing'),/Incomplete/);
  });
  it('retains the four-window budget and refuses a longer range',async()=>{
    const p=provider(Array.from({length:4},(_,i)=>page(Array.from({length:8},(_,j)=>String(i*8+j)),String(i+1))));
    await assert.rejects(readCorpusFragmentRange(p,work,version,'0','32'),/budget/);
    assert.equal(p.calls.length,4);
  });
  it('rejects the wrong start and repeated cursors or units',async()=>{
    await assert.rejects(readCorpusFragmentRange(provider([page(['wrong'])]),work,version,'a'),/start/);
    await assert.rejects(readCorpusFragmentRange(provider([page(['a'],'same'),page(['b'],'same')]),work,version,'a','c'),/Incomplete/);
    await assert.rejects(readCorpusFragmentRange(provider([page(['a'],'next'),page(['a','b'])]),work,version,'a','b'),/Repeated/);
  });
  it('keeps empty section endpoints rather than shifting citation offsets',async()=>{
    const first=page(['a','b']);first.units[0]={id:'a',kind:'section',text:''};
    assert.equal((await readCorpusFragmentRange(provider([first]),work,version,'a','b'))[0].id,'a');
  });
  it('requires a known revision before reading and refuses another version',async()=>{
    const p=provider([]);
    await assert.rejects(readCorpusFragmentRange(p,work,{versionId:'version'},'a'),/exact version/);
    assert.equal(p.calls.length,0);
    await assert.rejects(readCorpusFragmentRange(provider([{...page(['a']),versionId:'other'}]),work,version,'a'),/text layer changed/);
  });
});
