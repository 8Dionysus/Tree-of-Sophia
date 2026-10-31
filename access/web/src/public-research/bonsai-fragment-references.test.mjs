import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {describe,it} from 'vitest';
import * as atlas from '../../constructor/atlas-content.mjs';
import * as routes from '../../constructor/atlas-routes.mjs';
import * as guides from '../../constructor/close-reading-guides.mjs';
import * as foundation from '../../constructor/inquiry-foundation.mjs';
import * as readings from '../../constructor/inquiry-readings-echoes.mjs';
import * as sources from '../../constructor/inquiry-sources-concepts.mjs';
import * as relations from '../../constructor/relation-inquiry.mjs';

const read=path=>JSON.parse(readFileSync(new URL('../../../../'+path,import.meta.url),'utf8'));
function check(value,stand){
  const places=read(`bonsai/${stand}/fragments.json`).places,chapters=new Map();
  const visit=(v,path)=>{
    if(!v||typeof v!=='object')return;
    if(v.w&&(v.c||v.loc||Array.isArray(v.u))){
      if(v.loc)assert.ok(places[v.w]?.[v.loc],`${path}: missing ${v.w} ${v.loc}`);
      else if(v.c){
        assert.equal(v.w,'also-sprach-zarathustra',`${path}: chapter of another work`);
        if(!chapters.has(v.c))chapters.set(v.c,read(`bonsai/zarathustra/witnesses/${v.c}.json`));
        const units=chapters.get(v.c).units.de,a=units.findIndex(u=>u[0]===v.u?.[0]),b=units.findIndex(u=>u[0]===v.u?.[1]);
        assert.ok(a>=0&&b>=a,`${path}: invalid endpoints`);
        const s=v.s??0,e=v.e??units[b][2].length;
        assert.ok(Number.isInteger(s)&&Number.isInteger(e)&&s>=0&&s<=units[a][2].length&&e>=0&&e<=units[b][2].length&&(a!==b||e>s),`${path}: invalid offsets`);
      }else assert.ok(v.u.length===2&&v.u.every(id=>typeof id==='string'&&id),`${path}: invalid corpus endpoints`);
    }
    for(const [k,item] of Object.entries(v))visit(item,path+'.'+k);
  };
  visit(value,stand);
}

describe('every authored fragment reference',()=>{
  it('includes close-reading guides as well as the six atlas modules',()=>{
    check([atlas,routes,guides,foundation,readings,sources,relations],'zarathustra');
  });
  for(const stand of ['zarathustra','antiquity'])it(`${stand}: checks objects and their cited grounds`,()=>{
    check(read(`bonsai/${stand}/objects.json`),stand);
  });
  it('checks the Antiquity collection and its first branches',()=>{
    check([read('bonsai/antiquity/collection.json'),read('bonsai/antiquity/first-branches.json')],'antiquity');
  });
  it('keeps the missing Spinoza scholium in explicitly identified digital witnesses',()=>{
    const data=read('bonsai/zarathustra/fragments.json'),place=data.places.ethica['III.P9.S'];
    assert.equal(place.witness.la,'la-wikisource');assert.equal(place.witness.en,'en-gutenberg');
    for(const language of ['la','en'])assert.ok(place.texts[language]&&data.works.ethica.witnesses[place.witness[language]].url);
  });
});
