import {it,expect} from 'vitest';
import manifest from '../../public/public-corpus/manifest.json' with {type:'json'};
import aestheticsIdentities from '../../../../bonsai/aesthetics/identities.json' with {type:'json'};
import {publicCorpusWorkCount} from './corpus-work-count.mjs';

it('counts the Aesthetics subset through the Antiquity reader fallback',()=>{
  expect(manifest.profiles.aesthetics).toBeUndefined();
  expect(manifest.profiles.antiquity.documentIds).toHaveLength(51);
  const fixture={profiles:{antiquity:{documentIds:['work-a','work-b','work-c']}}};
  expect(publicCorpusWorkCount('aesthetics',fixture,{first:{tree:'work-a'},second:{tree:'work-c'},outside:{tree:'work-d'}})).toBe(2);
});

it('prefers the stand profile when a matching public snapshot is available',()=>{
  const current={profiles:{
    aesthetics:{documentIds:['work-a','work-b']},
    antiquity:{documentIds:['work-a','work-b','work-c']},
  }};
  expect(publicCorpusWorkCount('aesthetics',current,aestheticsIdentities.works)).toBe(2);
});

it('leaves full-profile reader counts unchanged',()=>{
  expect(publicCorpusWorkCount('antiquity',manifest)).toBe(51);
});
