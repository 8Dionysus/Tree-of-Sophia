import {beforeAll,expect,it} from 'vitest';
import {readFileSync} from 'node:fs';
import * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {configureResearchIndexSearch,createResearchIndex} from './research-index.mjs';

beforeAll(()=>{rules.initSync({module:new WebAssembly.Module(readFileSync(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url)))});configureResearchIndexSearch(rules);});

const index=()=>createResearchIndex({nodes:[],space:{researchRevision:'r1',core:{
  concepts:[{id:'overman',title:{ru:'Сверхчеловек',en:'Overman'}},{id:'child',title:{ru:'Дитя',en:'Child'}}],
  figures:[{id:'tightrope-walker',existence:'work-world',title:{ru:'Канатный плясун',en:'The tightrope walker'},note:'over the market'}],
}}});

it('asks the native query owner, which reads every word ignoring case in index order',()=>{
  const found=query=>index().search(query).map(item=>item.id);
  expect(found('OVER')).toEqual(['overman','tightrope-walker']);
  expect(found('сверхЧЕЛОВЕК')).toEqual(['overman']);
  expect(found('tightrope market')).toEqual(['tightrope-walker']);
  expect(found('tightrope child')).toEqual([]);
});

it('selects by kind and bounds the answer',()=>{
  const research=index();
  expect(research.search('over',{kind:'figure'}).map(item=>item.id)).toEqual(['tightrope-walker']);
  expect(research.search('over',{limit:1}).map(item=>item.id)).toEqual(['overman']);
});
