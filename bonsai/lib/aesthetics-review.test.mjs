import {test} from 'node:test';
import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {aestheticsReviewIssues,readAestheticsReview} from './aesthetics-review.mjs';

const root=resolve(import.meta.dirname,'../..'),original=readAestheticsReview(root);
const mutations={
  'missing register row':x=>x.layer.readings.pop(),
  'duplicate register row':x=>x.layer.readings.push(x.layer.readings[0]),
  'wrong register key':x=>x.layer.concepts[0].keys[0]='missing',
  'area count':x=>x.layer.areas.pop(),
  'work agent missing':x=>x.layer.works[0].agents=[],
  'work agent unknown':x=>x.layer.works[0].agents=['missing'],
  'work first agent':x=>x.layer.works[0].id='different.work',
  'reading area missing':x=>x.layer.readings[0].areas=[],
  'reading area unknown':x=>x.layer.readings[0].areas=['missing'],
  'reading concept missing':x=>x.layer.readings[0].concepts=[],
  'reading concept unknown':x=>x.layer.readings[0].concepts=['missing'],
  'reading work':x=>x.layer.readings[0].work='missing',
  'reading agent missing':x=>x.layer.readings[0].agents=[],
  'reading agent unknown':x=>x.layer.readings[0].agents=['missing'],
  'reading source':x=>x.layer.readings[0].sources=['missing'],
  'reading locus':x=>x.layer.readings[0].locus='',
  'reading URL':x=>x.layer.readings[0].url='',
  'reading thesis':x=>x.layer.readings[0].thesis={en:''},
  'unresolved concepts':x=>x.layer.readings[0].concepts_unresolved=[],
  'unresolved authors':x=>x.layer.readings[0].authors_unresolved=[],
  'link from':x=>x.layer.links[0].from={concept:'missing'},
  'link to':x=>x.layer.links[0].to={reading:'missing'},
  'link grounds missing':x=>x.layer.links[0].grounds=[],
  'link grounds unknown':x=>x.layer.links[0].grounds=['missing'],
  'unresolved grounds':x=>x.layer.links[0].grounds_unresolved=[],
  'concept area missing':x=>x.layer.concepts[0].areas=[],
  'concept area unknown':x=>x.layer.concepts[0].areas=['missing'],
  'key target':x=>x.layer.keys[Object.keys(x.layer.keys)[0]].id='missing',
  'agent ID syntax':x=>x.layer.agents[0].id='BAD!',
  'work ID syntax':x=>x.layer.works[0].id='BAD!',
  'corrected name':x=>x.layer.agents.find(a=>a.id==='valentin-wagner').register_names[0].original='wrong',
  'retired identity':x=>x.layer.agents[0].id='vera-wagner',
};

test('review layer rejects each damaged reference and register invariant',()=>{
  assert.deepEqual(aestheticsReviewIssues(original),[]);
  for(const [name,mutate] of Object.entries(mutations)){
    const input=structuredClone(original);mutate(input);
    assert.notEqual(aestheticsReviewIssues(input).length,0,name);
  }
});
