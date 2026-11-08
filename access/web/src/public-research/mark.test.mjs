import {expect,test} from 'vitest';
import {existsSync,readFileSync} from 'node:fs';

const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');

test('the Tree\'s mark has a drawing of its own for the noon, wherever the mark stands',()=>{
  for(const file of ['tree-mark.svg','tree-mark-2x.svg','tree-mark-noon.svg','favicon.svg','favicon.ico','apple-touch-icon.png'])
    expect(existsSync(new URL(`../../public/${file}`,import.meta.url))).toBe(true);
  // The noon's drawing is the night's tree in other inks: the same wood and stars, only their colours differ.
  const shape=svg=>svg.replace(/#[0-9a-f]{6}/gi,'');
  expect(shape(read('../../public/tree-mark-noon.svg'))).toBe(shape(read('../../public/tree-mark.svg')));
  const noon=[
    [read('./public-space.css'),':root[data-site-theme="noon"] #research-bar .pr-wordmark::before{background-image:url(/tree-mark-noon.svg)}'],
    [read('../home/home.css'),':root[data-site-theme="noon"] .home-mark::before{background-image:url(/tree-mark-noon.svg)}'],
    [read('../not-found.css'),':root[data-site-theme="noon"] .nf-mark::before{background-image:url(/tree-mark-noon.svg)}'],
    [read('../../public/cosmos.css'),'.tos-cosmos[data-sky=noon]:not([data-tree]) .cosmos-mark{filter:none;background-image:url(tree-mark-noon.svg)}'],
  ];
  for(const [style,rule] of noon)expect(style).toContain(rule);
  expect(read('./site-theme.css')).not.toContain('pr-wordmark');
});
