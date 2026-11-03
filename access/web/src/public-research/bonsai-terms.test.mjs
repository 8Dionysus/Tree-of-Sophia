import {readFileSync,readdirSync} from 'node:fs';
import {describe,expect,it} from 'vitest';
import {formPattern,holdsForm,termsIssues} from '../../../../bonsai/lib/terms.mjs';
import {termPlaceReader} from '../../../../bonsai/lib/graft.mjs';
import {fileURLToPath} from 'node:url';

// The philological core checks each term against the witnesses: the original says
// the word at its first place, and each translation says its own word there.
const dir=new URL('../../../../bonsai/zarathustra/witnesses/',import.meta.url);
const chapters=Object.fromEntries(readdirSync(dir).filter(name=>name.endsWith('.json')).map(name=>[name.slice(0,-5),JSON.parse(readFileSync(new URL(name,dir),'utf8'))]));
const editions={'antonovsky-1913':{lang:'ru',label:{ru:'пер. Ю. М. Антоновского, 1913',en:'tr. Yu. M. Antonovsky, 1913'}},'common-1909':{lang:'en',label:{ru:'пер. Т. Коммона, 1909',en:'tr. Thomas Common, 1909'}}};
const unit='tos.text-unit.sid-6ff057cbfa5323c0c554f447ed37cf36';
const kobolde=()=>({
  id:'kobolde',original:{lang:'de',lemma:'Kobold',forms:['Kobold','Kobolde']},
  first:{w:'also-sprach-zarathustra',c:'I.08',u:[unit,unit],s:0,e:52,q:'Ich will Kobolde um mich haben'},
  renderings:{
    ru:{tos:{term:'горные духи',forms:['горные духи','горных духов']},witness:{edition:'antonovsky-1913',term:'горные духи',forms:['горные духи','горных духов']},avoid:['домовые']},
    en:{tos:{term:'goblins',forms:['goblins']},witness:{edition:'common-1909',term:'goblins',forms:['goblins']}},
  },
  gloss:{ru:'Заратустра хочет горных духов вокруг себя, потому что он мужествен.',en:'Zarathustra wants goblins about him, for he is courageous.'},
});

it('checks an Antiquity corpus place, its exact word boundaries and normalized Greek forms',()=>{
  const root=fileURLToPath(new URL('../../../../',import.meta.url));
  const alignment=JSON.parse(readFileSync(new URL('../../../../bonsai/antiquity/alignment.json',import.meta.url),'utf8'));
  const collection=JSON.parse(readFileSync(new URL('../../../../bonsai/antiquity/collection.json',import.meta.url),'utf8'));
  const source=collection.terms.find(term=>term.id==='psyche');
  const cite=source.uses[0].body.cite[0];
  const term={id:'psyche',original:{lang:'grc',lemma:'ψυχή',forms:['ψυχή','ψυχῆς']},first:cite,renderings:{ru:{tos:{term:'душа',forms:['душа']}},en:{tos:{term:'soul',forms:['soul']}}},gloss:{ru:'Душа живёт.',en:'The soul lives.'}};
  const options={pivot:['grc','la'],place:termPlaceReader(root,alignment,null)};
  expect(termsIssues({schema:'tos.bonsai.terms.v1',terms:[term]}, {},options)).toEqual([]);
  term.first={...cite,u:['missing','missing']};
  expect(termsIssues({schema:'tos.bonsai.terms.v1',terms:[term]}, {},options).join(' ')).toContain('unknown or unordered units');
  term.first=cite;term.original.forms=['ψυχή','ψυχ'];
  expect(termsIssues({schema:'tos.bonsai.terms.v1',terms:[term]}, {},options).join(' ')).toContain('original does not say');
});
const module=terms=>({schema:'tos.bonsai.terms.v1',work:'za',editions,terms});

describe('the terms of a bonsai',()=>{
  it('finds the original word at the first place and each translation’s word in its counterpart',()=>{
    expect(termsIssues(module([kobolde()]),chapters)).toEqual([]);
  });
  it('names a translation that does not say the word it is given',()=>{
    const term=kobolde();term.renderings.ru.witness={edition:'antonovsky-1913',term:'домовые',forms:['домовые','домовых']};
    expect(termsIssues(module([term]),chapters)).toEqual(['term kobolde: antonovsky-1913 does not say домовые at I.08']);
  });
  it('names a first place where the original does not say the word',()=>{
    const term=kobolde();term.original={lang:'de',lemma:'Übermensch',forms:['Übermensch']};
    expect(termsIssues(module([term]),chapters)).toEqual(['term kobolde: the original does not say Übermensch at I.08']);
  });
  it('does not turn a substring into a word by cutting the citation short',()=>{
    const term=kobolde(),raw=chapters['I.08'].units.de.find(entry=>entry[0]===unit)[2];
    const start=raw.indexOf('Kobolde');
    term.first.s=start;term.first.e=start+'Kobold'.length;term.first.q='Kobold';
    term.original={lang:'de',lemma:'Kobold',forms:['Kobold']};
    expect(termsIssues(module([term]),chapters)).toContain('term kobolde: the original does not say Kobold at I.08');
  });
  it('checks the quoted words against the cited place',()=>{
    const term=kobolde();term.first.q='a different quotation';
    expect(termsIssues(module([term]),chapters)).toContain('term kobolde: first place quote is not in the cited original');
  });
  it('keeps a form to one term and apart from what the term avoids',()=>{
    const term=kobolde(),other={...kobolde(),id:'goblins'};term.renderings.ru.avoid=['горные духи'];
    expect(termsIssues(module([term,other]),chapters)).toEqual(expect.arrayContaining(['term kobolde: ru form "горные духи" is also avoided','term goblins: ru form "горные духи" belongs to kobolde']));
  });
  it('reads forms as whole words, ignoring case',()=>{
    expect(holdsForm('Сверхчеловек — смысл земли',['сверхчеловек'])).toBe(true);
    expect(holdsForm('сверхчеловека',['сверхчеловек'])).toBe(false);
    expect(formPattern(['eternal return']).test('the Eternal Return of the same')).toBe(true);
  });
});
