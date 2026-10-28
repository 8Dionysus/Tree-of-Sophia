import {expect, test} from 'vitest';
import {readFileSync} from 'node:fs';
import {localizePublicJourney} from './journey-i18n.mjs';
import {actionLabel, hypothesisTitle, kindLabel, proposalDisplay, proposalStatement, researchQuestionLine} from './presentation-i18n.mjs';
const published=JSON.parse(readFileSync(new URL('../../public/public-corpus/manifest.json',import.meta.url),'utf8'));
test('all published journeys have English presentation without changing source addresses',()=>{
 for(const name of ['antiquity','zarathustra']) {
 const profile=published.profiles[name]; expect(profile.journeys.length).toBeGreaterThan(0);
 for(const source of profile.journeys) {
  const translated=localizePublicJourney(source,'en');
  for(const field of ['title','question','readingHypothesis','strongAlternative']) { expect(translated[field]).not.toEqual(source[field]); expect(translated[field]).not.toMatch(/[А-Яа-я]/); }
  const {title,question,readingHypothesis,strongAlternative,steps,...rest}=translated;
  const {title:st,question:sq,readingHypothesis:sh,strongAlternative:sa,steps:ss,...sourceRest}=source;
  expect(rest).toEqual(sourceRest); expect(steps).toHaveLength(ss.length);
  for(const [index,step] of steps.entries()) { for(const field of ['title','voice','prompt']) {expect(step[field]).not.toEqual(ss[index][field]); expect(step[field]).not.toMatch(/[А-Яа-я]/);} const {title,voice,prompt,...address}=step; const {title:a,voice:b,prompt:c,...original}=ss[index]; expect(address).toEqual(original); }
  expect(localizePublicJourney(source,'ru')).toEqual(source);
 }
 }
});
test('only generated relation prefixes are localized',()=>{
 const statement='Objection to thesis: моё чтение';
 expect(proposalDisplay({id:'objection-link-123',kind:'relation',statement},'ru')).toBe('Возражение к тезису: моё чтение');
 expect(proposalDisplay({id:'user-123',kind:'relation',statement},'ru')).toBe(statement);
 expect(proposalDisplay({id:'objection-link-123',kind:'interpretation',statement},'ru')).toBe(statement);
 expect(hypothesisTitle({id:'objection-123',title:'Objection: моё чтение'},'ru')).toBe('Возражение: моё чтение');
 expect(hypothesisTitle({id:'user-123',title:'Objection: моё чтение'},'ru')).toBe('Objection: моё чтение');
});
test('presentation translates known labels and prefixes without mutating stored text',()=>{
 const stored='Objection to thesis: моя мысль'; expect(proposalStatement(stored,'ru')).toBe('Возражение к тезису: моя мысль'); expect(proposalStatement(stored,'en')).toBe(stored);
 expect(kindLabel('relation','ru')).toBe('Связь'); expect(actionLabel('proposal.stage','en')).toBe('Added proposal');
 expect(actionLabel('hypothesis.add','ru')).toBe('Добавлена личная мысль');
 expect(actionLabel('hypothesis.add','en')).toBe('Added personal thought');
 expect(researchQuestionLine('', 'ru')).toBe('Вопрос исследования не задан.');
 expect(researchQuestionLine('   ', 'en')).toBe('No research question set.');
 expect(researchQuestionLine('What does he mean?', 'en')).toBe('Research question: What does he mean?');
});
