import assert from 'node:assert/strict';
import test from 'node:test';
import {publicPrototypeAsset} from '../src/public-prototypes.ts';

test('public prototype paths map to their built pages', () => {
  for (const [path, page] of [['/zarathustra', '/zarathustra/'], ['/zarathustra/', '/zarathustra/'], ['/antiquity', '/antiquity/'], ['/antiquity/', '/antiquity/']]) {
    assert.equal(publicPrototypeAsset(path), page);
  }
  assert.equal(publicPrototypeAsset('/other'), null);
});
