import {describe,it,expect} from 'vitest';
import {fragmentMayShow,externalPlacesIssues,wholeExternalSentence} from '../../../../bonsai/lib/fragments.mjs';
import {aestheticsReadingLanguages} from './fragments.mjs';

const entry=(basis)=>({basis,law:'The applicable statutory provision and its term.'});
const witness=(russia='public-domain',usa='public-domain',eu='public-domain')=>({edition:'Book',attribution:'Author',url:'https://example.org/page',rights:{russia:entry(russia),usa:entry(usa),eu:entry(eu)}});

describe('territorial fragment rights',()=>{
  it('shows the original and the exact reader language without English fallback',()=>{
    expect(aestheticsReadingLanguages('ru','de',['de','en','ru'])).toEqual(['de','ru']);
    expect(aestheticsReadingLanguages('ru','de',['de','en'])).toEqual(['de']);
    expect(aestheticsReadingLanguages('zh-Hans','de',['de','zh-Hans'])).toEqual(['de','zh-Hans']);
  });
  it('handles uncased scripts and full-width sentence endings',()=>{
    expect(wholeExternalSentence('这是一句完整的话。')).toBe(true);
    expect(wholeExternalSentence('「这是一个句子。」')).toBe(true);
    expect(wholeExternalSentence('هذه جملة كاملة؟')).toBe(true);
    expect(wholeExternalSentence('lowercase fragment.')).toBe(false);
    expect(wholeExternalSentence('没有终点')).toBe(false);
  });
  it('requires a complete permission and withholds text if any jurisdiction has only a locator',()=>{
    expect(fragmentMayShow(witness())).toBe(true);
    expect(fragmentMayShow(witness('public-domain','short-quotation','public-domain'))).toBe(true);
    expect(fragmentMayShow(witness('public-domain','locator-only','public-domain'))).toBe(false);
    const licensed=witness('licensed');
    expect(fragmentMayShow(licensed)).toBe(false);
    licensed.licence_evidence={edition:'Book',licence_url:'https://creativecommons.org/licenses/by/4.0/',source:{file:'edition.html',url:'https://example.org/edition',sha256:'a'.repeat(64)},explanation:'The publisher licenses this exact edition under CC BY 4.0.'};
    expect(fragmentMayShow(licensed)).toBe(true);
    const incomplete=witness();delete incomplete.rights.eu;
    expect(fragmentMayShow(incomplete)).toBe(false);
    incomplete.rights.basis='public-domain';
    expect(fragmentMayShow(incomplete)).toBe(false);
    expect(fragmentMayShow({rights:{basis:'public-domain'}})).toBe(true);
  });
  it('checks every jurisdiction and limits a quotation',()=>{
    const module={schema:'tos.bonsai.fragments.v1',works:{w:{title:{ru:'Книга',en:'Book'},original:'en',witnesses:{en:witness('short-quotation')}}},places:{w:{p:{label:{ru:'с. 1',en:'p. 1'},texts:{en:'A'.repeat(901)}}}}};
    expect(externalPlacesIssues(module)).toContain('w p en quotation is longer than a short quotation');
    module.places.w.p.texts.en='一句。二句。三句。四句。';
    expect(externalPlacesIssues(module)).toContain('w p en quotation is longer than a short quotation');
    module.places.w.p.quotation_justification={en:'The complete sentence requires this length.'};
    module.places.w.p.address={kind:'section',value:'III.2',url:'https://example.org/section'};
    module.places.w.p.links={en:'https://example.org/section'};
    expect(externalPlacesIssues(module)).toEqual([]);
    module.places.w.p.address.url='https://example.org/other';
    expect(externalPlacesIssues(module)).toContain('w p has an invalid original source address');
    module.places.w.p.address.url='https://example.org/section';
    module.works.w.witnesses.en.rights.usa=entry('locator-only');
    expect(externalPlacesIssues(module)).toContain('w p shows en text of a work cited by locator only');
    delete module.works.w.witnesses.en.rights.eu;
    expect(externalPlacesIssues(module)).toContain('work w en witness has no rights basis');
  });
});
