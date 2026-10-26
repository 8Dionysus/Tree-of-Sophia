import assert from 'node:assert/strict';
import test from 'node:test';
import {publishedNodeFixtureWorker} from './native-lens-fixture.ts';

test('public prototype routes serve their built MPA HTML through ASSETS', async () => {
  const worker = await publishedNodeFixtureWorker();
  for (const [path, assetPath] of [
    ['/zarathustra', '/zarathustra/'],
    ['/zarathustra/', '/zarathustra/'],
    ['/antiquity', '/antiquity/'],
    ['/antiquity/', '/antiquity/'],
  ]) {
    let fetchedPath;
    const response = await worker.fetch(new Request(`https://tos.test${path}`), {
      ASSETS: {async fetch(request) {
        fetchedPath = new URL(request.url).pathname;
        return new Response(`<main>${assetPath}</main>`, {
          headers: {'Content-Type': 'text/html; charset=utf-8'},
        });
      }},
    }, {});
    assert.equal(fetchedPath, assetPath);
    assert.equal(response.status, 200);
    assert.match(response.headers.get('Content-Security-Policy'), /default-src 'self'/);
    assert.equal(await response.text(), `<main>${assetPath}</main>`);
  }
});
