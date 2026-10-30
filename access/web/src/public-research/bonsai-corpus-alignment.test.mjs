import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {alignCorpus,citedUnits,corpusReader,formatAlignment,textEnd} from '../../../../bonsai/lib/corpus-align.mjs';
import {corpusAlignmentIssues} from '../../../../bonsai/lib/fragments.mjs';
import * as atlasContent from '../../constructor/atlas-content.mjs';
import * as atlasRoutes from '../../constructor/atlas-routes.mjs';
import * as relationInquiry from '../../constructor/relation-inquiry.mjs';
import * as sourcesConcepts from '../../constructor/inquiry-sources-concepts.mjs';
import * as readingsEchoes from '../../constructor/inquiry-readings-echoes.mjs';
import * as foundation from '../../constructor/inquiry-foundation.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const read=corpusReader(join(root,'access','web','public','public-corpus'));
const json=path=>JSON.parse(readFileSync(join(root,path),'utf8'));
const worksOf=stand=>Object.fromEntries(Object.entries(json(`bonsai/${stand}/identities.json`).works).map(([key,value])=>[key,value.tree]));

describe('the corpus alignment of the stands',()=>{
  it('keeps the edition notes out of the text a unit is paired by',()=>{
    expect(textEnd('Text.\n〔1〕 A note.')).toBe(5);
    expect(textEnd('Text without notes.')).toBe(19);
  });
  it('is, for Zarathustra, what the mechanism gives from the stand\'s own citations',()=>{
    const content=[atlasContent,atlasRoutes,relationInquiry,sourcesConcepts,readingsEchoes,foundation].map(module=>Object.values(module));
    const works=worksOf('zarathustra'),stored=readFileSync(join(root,'bonsai','zarathustra','alignment.json'),'utf8');
    expect(formatAlignment(alignCorpus(citedUnits(content,works,read),works,read))).toBe(stored);
  },120000);
  for(const stand of ['zarathustra','antiquity'])it(`${stand}: holds every cited unit of a work the corpus translates`,()=>{
    const module=json(`bonsai/${stand}/alignment.json`),works=worksOf(stand);
    expect(corpusAlignmentIssues(module)).toEqual([]);
    const content=stand==='antiquity'?[json('bonsai/antiquity/collection.json'),json('bonsai/antiquity/first-branches.json')]:[atlasContent,atlasRoutes,relationInquiry,sourcesConcepts,readingsEchoes,foundation].map(item=>Object.values(item));
    for(const [key,{units}] of citedUnits(content,works,read)){
      const work=module.works[key];
      if(!work?.translations?.length)continue;
      for(const unit of units)expect(module.units[unit]?.w,`${stand} ${key} ${unit}`).toBe(key);
    }
  },120000);
});
