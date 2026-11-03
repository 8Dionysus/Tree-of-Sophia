import {describe,expect,it} from 'vitest';
import {readFileSync} from 'node:fs';
import {termForms,termMatcher,treeRendering} from './term-view.mjs';

const module={terms:[
  {id:'ubermensch',original:{lemma:'Übermensch',forms:['Übermensch','Übermenschen']},renderings:{ru:{tos:{term:'сверхчеловек',forms:['сверхчеловек','сверхчеловека']},witness:{term:'сверхчеловек',forms:['сверхчеловеке']}},en:{tos:{term:'overman',forms:['overman']},witness:{term:'Superman',forms:['Superman']}}}},
  {id:'kobolde',original:{lemma:'Kobold',forms:['Kobolde']},renderings:{ru:{tos:{term:'горные духи',forms:['горные духи','горных духов']},witness:{term:'горные духи',forms:['горных духов']}},en:{tos:{term:'goblins',forms:['goblins']}}}},
]};

describe('active terms',()=>{
  it('shows the distinct Tree rendering even when its Russian word matches Antonovsky',()=>{
    expect(treeRendering(module.terms[0],'ru')).toEqual({lang:'en',term:'overman'});
    expect(treeRendering(module.terms[0],'en')).toEqual({lang:'en',term:'overman'});
    expect(treeRendering(module.terms[1],'ru')).toBeNull();
    const registry=JSON.parse(readFileSync(new URL('../../../../bonsai/zarathustra/terms.json',import.meta.url),'utf8'));
    const overman=registry.terms.find(term=>term.id==='overman');
    expect(overman.original.lemma).toBe('Übermensch');
    expect(overman.renderings.ru.witness.term).toBe('сверхчеловек');
    expect(overman.renderings.en.witness.term).toBe('Superman');
    expect(treeRendering(overman,'ru')).toEqual({lang:'en',term:'overman'});
    expect(overman.gloss.ru).toBeTruthy();
  });
  it('read the Tree’s words and the shown translation’s words in a language, and the original’s in German',()=>{
    expect([...termForms(module,'ru').keys()]).toEqual(['сверхчеловек','сверхчеловека','сверхчеловеке','горные духи','горных духов']);
    expect([...termForms(module,'en').keys()]).toEqual(['overman','Superman','goblins']);
    expect([...termForms(module,'de').keys()]).toEqual(['Übermensch','Übermenschen','Kobolde']);
  });
  it('find a form as a whole word, the longest first; a lower-case form in any case',()=>{
    const found=termMatcher(termForms(module,'ru')).find('Сверхчеловека ждут горных духов и сверхчеловечество.').map(item=>item.term.id);
    expect(found).toEqual(['ubermensch','kobolde']);
    expect(termMatcher(new Map())).toBeNull();
  });
  it('read a capitalised form with its capital',()=>{
    const terms={terms:[{id:'becoming',original:{lemma:'Werden',forms:['Werden']},renderings:{ru:{tos:{term:'Само',forms:['Само','Самого']},witness:{term:'Само',forms:['Само']}}}}]};
    expect(termMatcher(termForms(terms,'de')).find('Wir werden sehen; das Werden bleibt.').map(item=>item.text)).toEqual(['Werden']);
    expect(termMatcher(termForms(terms,'ru')).find('превзойти его самого; служит своему Самому, Самого.').map(item=>item.text)).toEqual(['Самого']);
    expect(termMatcher(new Map())).toBeNull();
  });
});
