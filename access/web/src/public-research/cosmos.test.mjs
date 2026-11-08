import {expect,test} from 'vitest';
import {existsSync,readFileSync} from 'node:fs';

const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');

test('every stand page has the sky between pages in its first frame, and the home page keeps it ready',()=>{
  for(const page of ['zarathustra','antiquity']){
    const html=read(`../../${page}/index.html`);
    // In the head, after the theme and the sky's style: the page that arrives is never painted without its sky.
    expect(html).toMatch(new RegExp(`<script src="/theme.js"></script>[\\s\\S]*<link rel="stylesheet" href="/cosmos.css" />\\s*<script src="/cosmos.js" data-stand="${page}"></script>\\s*</head>`));
  }
  const home=read('../../home/index.html');
  expect(home).toContain('<link rel="stylesheet" href="/cosmos.css" />');
  expect(home).toContain('<script src="/cosmos.js" data-cosmos="off"></script>');
});

test('the sky keeps to the site policy: no inline style, its mark by a relative address, its galaxies beside it',()=>{
  const script=read('../../public/cosmos.js'),style=read('../../public/cosmos.css');
  expect(script).not.toMatch(/createElement\(['"]style/);
  expect(style).toContain('url(tree-mark.svg)');
  expect(style).not.toMatch(/url\(\//);
  // Each near galaxy the sky names, and the atlas of the crowd, is drawn once and served beside the script.
  const plates=[...script.matchAll(/\{name:'([a-z]+)'/g)].map(match=>match[1]).concat('atlas');
  expect(plates).toEqual(['whirlpool','barred','sunflower','sombrero','hoag','atlas']);
  for(const name of plates)expect(existsSync(new URL(`../../public/cosmos/${name}.webp`,import.meta.url))).toBe(true);
});
