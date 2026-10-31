import assert from 'node:assert/strict';
import {describe,it} from 'vitest';
import {createLatestFragmentRequest} from './fragment-request.mjs';

describe('pending chapter reads',()=>{
  it('only allows the newest navigation to paint',()=>{
    const guard=createLatestFragmentRequest(),old=guard.begin(),latest=guard.begin();
    assert.equal(old(),false);assert.equal(latest(),true);
  });
  it('does not let a late response reopen a closed reader',()=>{
    const guard=createLatestFragmentRequest(),pending=guard.begin();
    guard.cancel();assert.equal(pending(),false);
    assert.equal(guard.begin()(),true);
  });
  it('keeps the newer paint when responses complete in reverse order',async()=>{
    const guard=createLatestFragmentRequest();let releaseOld,releaseNew,visible=null;
    const oldWait=new Promise(resolve=>{releaseOld=resolve;}),newWait=new Promise(resolve=>{releaseNew=resolve;});
    const paint=async(label,wait)=>{const current=guard.begin();await wait;if(current())visible=label;};
    const old=paint('old',oldWait),latest=paint('new',newWait);
    releaseNew();await latest;releaseOld();await old;assert.equal(visible,'new');
  });
  it('invalidates all pending requests on disposal',()=>{
    const guard=createLatestFragmentRequest(),first=guard.begin(),second=guard.begin();
    guard.cancel();guard.cancel();assert.equal(first(),false);assert.equal(second(),false);
  });
});
