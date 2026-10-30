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
  it('invalidates all pending requests on disposal',()=>{
    const guard=createLatestFragmentRequest(),first=guard.begin(),second=guard.begin();
    guard.cancel();guard.cancel();assert.equal(first(),false);assert.equal(second(),false);
  });
});
