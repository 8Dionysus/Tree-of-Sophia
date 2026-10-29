import {readFileSync,readdirSync} from 'node:fs';
import {expect,test} from 'vitest';
import {PUBLIC_LOCALES,resolveLocale,localeText,localeValue} from './locale.mjs';
import {formatDating,formatYears} from '../../../../bonsai/lib/time.mjs';
import {localizePublicJourney} from './journey-i18n.mjs';
import {hypothesisTitle,workspaceConflictLabel} from './presentation-i18n.mjs';
import {corpusWorkTitle} from '../observatory/human-presentation.mjs';
import {morphologyLabel} from '../corpus-reader/philology.mjs';
import {collectionPeriodKey} from './collection-view.mjs';

const added=['es','de','zh-Hans'];
const read=url=>JSON.parse(readFileSync(url,'utf8'));
const catalogs=Object.fromEntries(added.map(language=>[language,{
  bonsai:read(new URL(`../../../../bonsai/locales/${language}.json`,import.meta.url)),
  public:read(new URL(`./locales/${language}.json`,import.meta.url)),
}]));
const markers=value=>(value.match(/\{[^{}]+\}/gu)??[]).sort();

test('catalogs cover the same messages with intact placeholders and boundary spaces',()=>{
  for(const section of ['bonsai','public']){
    const keys=Object.keys(catalogs.es[section]).sort();
    for(const language of added){
      const catalog=catalogs[language][section];
      expect(Object.keys(catalog).sort(),`${language}/${section} coverage`).toEqual(keys);
      for(const [key,value]of Object.entries(catalog)){
        expect(typeof value,`${language}: ${key}`).toBe('string');
        expect(value.trim().length,`${language}: ${key}`).toBeGreaterThan(0);
        expect(markers(value),`${language}: ${key}`).toEqual(markers(key));
        if(language!=='zh-Hans')expect(value.match(/^\s*/u)?.[0],`${language}: leading space ${key}`).toBe(key.match(/^\s*/u)?.[0]);
        if(language!=='zh-Hans')expect(value.match(/\s*$/u)?.[0],`${language}: trailing space ${key}`).toBe(key.match(/\s*$/u)?.[0]);
      }
    }
  }
});

test('every authored Bonsai display form has a translation entry',()=>{
  const forms=new Set();
  const walk=value=>{
    if(Array.isArray(value)){value.forEach(walk);return;}
    if(!value||typeof value!=='object')return;
    if(typeof value.ru==='string'&&typeof value.en==='string'&&value.en.trim()&&/\p{L}/u.test(value.en))forms.add(value.en);
    Object.values(value).forEach(walk);
  };
  for(const stand of ['antiquity','zarathustra']){
    const home=new URL(`../../../../bonsai/${stand}/`,import.meta.url);
    for(const file of readdirSync(home))if(file.endsWith('.json'))walk(read(new URL(file,home)));
  }
  for(const language of added)for(const form of forms)expect(Object.hasOwn(catalogs[language].bonsai,form),`${language}: ${form}`).toBe(true);
});

test('locale aliases preserve script choice and authored forms keep source and personal text literal',()=>{
  expect(resolveLocale('es-MX')).toBe('es');expect(resolveLocale('de-DE')).toBe('de');
  expect(resolveLocale('zh-CN')).toBe('zh-Hans');expect(resolveLocale('zh-Hans-SG')).toBe('zh-Hans');
  expect(resolveLocale('zh-Hant',null)).toBe(null);
  for(const language of PUBLIC_LOCALES){
    expect(localeValue('Research question',language)).toBe('Research question');
    expect(localeText('constructor',language)).toBe('constructor');
    expect(localeText('__proto__',language)).toBe('__proto__');
  }
  const form=Object.freeze({ru:'Место',en:'Place'}),before=JSON.stringify(form);
  for(const language of added)expect(localeValue(form,language)).toBe(catalogs[language].public.Place??catalogs[language].bonsai.Place);
  for(const language of added)expect(localeText('Reading',language,[],'corpus-reader')).toBe(catalogs[language].public['corpus-reader:Reading']);
  expect(JSON.stringify(form)).toBe(before);
  expect(localeValue({ru:'Апология · 21d',en:'Apology of Socrates · 21d'},'es')).toContain('21d');
  expect(localeValue({ru:'Апология · 21d',en:'Apology of Socrates · 21d'},'es')).not.toContain('Apology of Socrates');
});

test('translated reading paths keep all edition and passage addresses',()=>{
  const published=read(new URL('../../public/public-corpus/manifest.json',import.meta.url));
  const addresses=({title,question,readingHypothesis,strongAlternative,steps,...rest})=>({...rest,steps:steps?.map(({title,voice,prompt,...address})=>address)});
  for(const profile of Object.values(published.profiles))for(const source of profile.journeys){
    const original=JSON.stringify(source);
    for(const language of added){
      const translated=localizePublicJourney(source,language);
      expect(addresses(translated)).toEqual(addresses(source));
      expect(translated.question).not.toEqual(source.question);
    }
    expect(JSON.stringify(source)).toBe(original);
  }
});

test('localized dates retain astronomical era boundaries, precision and uncertainty',()=>{
  for(const language of added){
    const years=formatYears({first:0,last:1,certainty:'uncertain'},language);
    expect(years).toContain('1');expect(years).toContain('(?)');
    expect(years).toContain(catalogs[language].bonsai.BCE);expect(years).toContain(catalogs[language].bonsai.CE);
    const month=formatDating({kind:'date-assertion',year_numbering:'astronomical',value:'1883-02',certainty:'approximate'},language);
    expect(month).toContain('1883');expect(month).not.toContain('February');
    expect(month).toContain(catalogs[language].bonsai['c.']);
    expect(formatDating({kind:'unknown-date'},language)).toBe('');
  }
});

test('provider title fallbacks remain literal even when they match a UI message',()=>{
  const title=Object.freeze({ru:'Место',en:'Place'}),document=Object.freeze({id:'external-work',title});
  for(const language of added)expect(corpusWorkTitle(document,language)).toBe('Place');
  expect(corpusWorkTitle({id:'external-work',title:{...title,es:'Lugar del proveedor'}},'es')).toBe('Lugar del proveedor');
  expect(corpusWorkTitle({id:'external-work',title:'Place'},'es')).toBe('Place');
});

test('known morphology labels localize while unknown analyzer tags stay literal',()=>{
  const english='noun · nominative · singular';
  expect(morphologyLabel('<NN><Nom><Sg>','en')).toBe(english);
  for(const language of added){
    const label=morphologyLabel('<NN><Nom><Sg>',language);
    expect(label).not.toBe(english);expect(label.split(' · ')).toHaveLength(3);
    expect(morphologyLabel('<unrecognized-source-tag>',language)).toBe('<unrecognized-source-tag>');
  }
});

test('generated legacy titles follow the UI but keep the personal suffix unchanged',()=>{
  const suffix='原文 · моя мысль {0}';
  for(const savedLanguage of added){
    const title=`${localeText('Objection',savedLanguage)}: ${suffix}`;
    const stored=Object.freeze({id:'objection-fixture',title,body:suffix});
    for(const language of PUBLIC_LOCALES){
      const prefix=language==='ru'?'Возражение: ':localeText('Objection: ',language);
      expect(hypothesisTitle(stored,language)).toBe(prefix+suffix);
      expect(hypothesisTitle({...stored,id:'manual-fixture'},language)).toBe(title);
    }
    expect(stored.title).toBe(title);expect(stored.body).toBe(suffix);
  }
});

test('import conflict labels retain exact ids and unknown diagnostics',()=>{
  const id='source:fixture cannot be imported : ü{0}γ';
  const conflict=`workspace source address ${id} cannot be imported because its shelf owner packet is unavailable or conflicted.`;
  for(const language of PUBLIC_LOCALES){
    expect(workspaceConflictLabel(conflict,language)).toContain(id);
    expect(workspaceConflictLabel('Unrecognized owner condition: literal detail',language)).toBe('Unrecognized owner condition: literal detail');
  }
});

test('an era filter uses the same key in every display language',()=>{
  const period=Object.freeze({ru:'Классическая Греция',en:'Classical Greece'}),key=collectionPeriodKey(period);
  expect(key).toBe('Classical Greece');
  for(const language of PUBLIC_LOCALES){expect(localeValue(period,language)).toBeTruthy();expect(collectionPeriodKey(period)).toBe(key);}
});
