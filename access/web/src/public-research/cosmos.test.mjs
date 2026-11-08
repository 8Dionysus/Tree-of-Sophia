import {expect,test} from 'vitest';
import {readFileSync} from 'node:fs';

const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');

test('every stand page paints the sky between pages before its own scripts, and the home page keeps it ready',()=>{
  for(const page of ['zarathustra','antiquity']){
    const html=read(`../../${page}/index.html`);
    expect(html).toContain('<link rel="stylesheet" href="/cosmos.css" />');
    expect(html).toMatch(new RegExp(`<script src="/cosmos.js" data-stand="${page}"></script>\\s*<div id="public-research"`));
  }
  const home=read('../../home/index.html');
  expect(home).toContain('<link rel="stylesheet" href="/cosmos.css" />');
  expect(home).toContain('<script src="/cosmos.js" data-cosmos="off"></script>');
});

test('the sky keeps to the site policy: no inline style, its worker beside it, its mark by a relative address',()=>{
  const script=read('../../public/cosmos.js'),style=read('../../public/cosmos.css'),worker=read('../../public/cosmos-worker.js');
  expect(script).not.toMatch(/createElement\(['"]style/);
  expect(script).toContain('cosmos-worker.js');
  expect(worker).toContain('self.onmessage');
  expect(style).toContain('url(tree-mark.svg)');
  expect(style).not.toMatch(/url\(\//);
});
