import {existsSync,readFileSync,readdirSync} from 'node:fs';
import {expect,test} from 'vitest';
import ts from 'typescript';
import {PUBLIC_LOCALES,LOCALE_NAMES,initialLocale,resolveLocale,shownLanguages,translationLanguage,localeText,localeValue} from './locale.mjs';
import {formatDating,formatYears} from '../../../../bonsai/lib/time.mjs';
import {localizePublicJourney} from './journey-i18n.mjs';
import {hypothesisTitle,workspaceConflictLabel} from './presentation-i18n.mjs';
import {corpusAuthorName,corpusWorkTitle} from '../observatory/human-presentation.mjs';
import {morphologyLabel} from '../corpus-reader/philology.mjs';
import {collectionPeriodKey} from './collection-view.mjs';
import {loadBonsai} from './bonsai.mjs';

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

test('every authored Bonsai display form has a translation entry',async()=>{
  const forms=new Set();
  const zarathustraModules=await loadBonsai('zarathustra');
  const walk=value=>{
    if(Array.isArray(value)){value.forEach(walk);return;}
    if(!value||typeof value!=='object')return;
    if(typeof value.ru==='string'&&typeof value.en==='string'&&value.en.trim()&&/\p{L}/u.test(value.en))forms.add(value.en);
    // A place's texts are a work's own words in its witnesses, not forms of the stand;
    // a story's pieces are its sources as they stand: the witnesses' sentences, the Tree's readings and words.
    // Language-keyed links are source addresses, not display forms.
    for(const [key,item] of Object.entries(value))if(key!=='texts'&&key!=='pieces'&&key!=='links')walk(item);
  };
  for(const stand of ['antiquity','zarathustra']){
    const home=new URL(`../../../../bonsai/${stand}/`,import.meta.url);
    // Held material is not shown by the stand. The terms module is also excluded
    // until loadBonsai returns it; this exclusion lifts when the stand loader consumes it.
    // The media names are the Tree's names of works in each language it names them in, chosen by language, not forms to translate.
    for(const file of readdirSync(home))if(file.endsWith('.json')&&file!=='held.json'&&file!=='media-names.json'&&!(stand==='zarathustra'&&file==='terms.json'&&!Object.hasOwn(zarathustraModules,'terms')))walk(read(new URL(file,home)));
  }
  for(const language of added)for(const form of forms)expect(Object.hasOwn(catalogs[language].bonsai,form),`${language}: ${form}`).toBe(true);
});

test('locale aliases preserve script choice and authored forms keep source and personal text literal',()=>{
  expect(localeText('Interface language','ru')).toBe('Язык интерфейса');
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

test('the public corpus names its authors in the reader\'s language, and an author it does not know as the manifest spells it',()=>{
  expect(corpusAuthorName('Plato','ru')).toBe('Платон');expect(corpusAuthorName('Plato','en')).toBe('Plato');
  expect(corpusAuthorName('Xenophon','ru')).toBe('Ксенофонт');expect(corpusAuthorName('Xenophon','es')).toBe('Jenofonte');
  expect(corpusWorkTitle({id:'tos.work.xenophon.memorabilia',title:'Memorabilia'},'ru')).toBe('Воспоминания о Сократе');
  expect(corpusAuthorName('Hippocratic corpus','ru')).toBe('Гиппократов корпус');
  expect(corpusAuthorName('Friedrich Nietzsche','ru')).toBe('Фридрих Ницше');
  expect(corpusAuthorName('Plato','de')).toBe('Platon');expect(corpusAuthorName('Plato','es')).toBe('Platón');expect(corpusAuthorName('Plato','zh-Hans')).toBe('柏拉图');
  expect(corpusAuthorName({en:'Anonymous',ru:'Аноним'},'ru')).toBe('Аноним');expect(corpusAuthorName({en:'Anonymous',ru:'Аноним'},'de')).toBe('Anonymous');
  expect(corpusAuthorName('Someone Else','ru')).toBe('Someone Else');
  // Every author of the public corpus has a Russian name, where the checkout holds the corpus.
  const manifest=new URL('../../public/public-corpus/manifest.json',import.meta.url);
  if(existsSync(manifest))for(const document of read(manifest).documents)expect(corpusAuthorName(document.author,'ru'),document.author).toMatch(/[А-Яа-яЁё]/);
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


test('every statically authored public UI message has a translation entry',()=>{
  const forms=new Set(),seen=new Set(),pending=[new URL('./app.mjs',import.meta.url)];
  const literal=node=>node&&(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node));
  const presentationPair=(first,second)=>literal(first)&&literal(second)&&/[А-Яа-яЁё]/u.test(first.text);
  const strings=node=>{
    if(!node)return;
    if(literal(node)){if(node.text.trim()&&/\p{L}/u.test(node.text))forms.add(node.text);return;}
    if(ts.isObjectLiteralExpression(node))for(const property of node.properties)if(ts.isPropertyAssignment(property))strings(property.initializer);
  };
  while(pending.length){
    const file=pending.pop();if(seen.has(file.href)||!file.pathname.endsWith('.mjs')||file.pathname.includes('/generated/'))continue;
    seen.add(file.href);
    const source=ts.createSourceFile(file.pathname,readFileSync(file,'utf8'),ts.ScriptTarget.Latest,true);
    const visit=node=>{
      if((ts.isImportDeclaration(node)||ts.isExportDeclaration(node))&&literal(node.moduleSpecifier)&&node.moduleSpecifier.text.startsWith('.'))pending.push(new URL(node.moduleSpecifier.text,file));
      if(ts.isObjectLiteralExpression(node)){
        const properties=new Map(node.properties.filter(ts.isPropertyAssignment).map(property=>[property.name?.text,property.initializer]));
        if(properties.has('ru')&&properties.has('en'))strings(properties.get('en'));
      }
      if(ts.isArrayLiteralExpression(node)&&node.elements.length===2&&presentationPair(...node.elements))strings(node.elements[1]);
      if(ts.isBinaryExpression(node)&&node.operatorToken.kind===ts.SyntaxKind.EqualsToken&&/\.en(?:\.|\[)/.test(node.left.getText(source)))strings(node.right);
      if(ts.isCallExpression(node)){
        const name=node.expression.getText(source);
        if(name==='Object.assign'&&/\.en$/.test(node.arguments[0]?.getText(source)))strings(node.arguments[1]);
        if(['localeText','bonsaiText'].includes(name)&&literal(node.arguments[0]))strings(node.arguments[0]);
        if(name==='t'&&presentationPair(...node.arguments))strings(node.arguments[1]);
      }
      ts.forEachChild(node,visit);
    };
    visit(source);
  }
  const operations=read(new URL('../../../../bonsai/zarathustra/research-operations.json',import.meta.url));
  for(const axis of operations.axes)forms.add(axis.label);
  for(const claim of operations.claims)forms.add(claim.statement);
  for(const language of added){
    const missing=[...forms].filter(form=>!Object.hasOwn(catalogs[language].public,form)&&!Object.hasOwn(catalogs[language].bonsai,form));
    expect(missing,`${language}: missing authored public messages`).toEqual([]);
  }
});

test('the site opens in English unless the address, the saved choice or the system names a language it has',()=>{
  expect(PUBLIC_LOCALES).toEqual(['en','ru','es','de','zh-Hans']);
  expect(Object.keys(LOCALE_NAMES)).toEqual([...PUBLIC_LOCALES]);
  expect(initialLocale()).toBe('en');
  expect(initialLocale({system:['fr-FR','ru-RU']})).toBe('ru');
  expect(initialLocale({system:['fr-FR','it-IT']})).toBe('en');
  expect(initialLocale({saved:'de',system:['ru-RU']})).toBe('de');
  expect(initialLocale({requested:'zh-CN',saved:'de',system:['ru']})).toBe('zh-Hans');
  expect(initialLocale({requested:'xx',saved:'nonsense',system:['pt-BR']})).toBe('en');
  expect(resolveLocale('xx')).toBe('en');expect(resolveLocale(undefined)).toBe('en');
});

test('a reader sees the original and one translation in a language the interface speaks to them in',()=>{
  const zarathustra=['de','en','ru'];
  expect(shownLanguages('en','de',zarathustra)).toEqual(['de','en']);
  expect(shownLanguages('ru','de',zarathustra)).toEqual(['de','ru']);
  expect(shownLanguages('es','de',zarathustra)).toEqual(['de','en']);
  expect(shownLanguages('zh-Hans','de',zarathustra)).toEqual(['de','en']);
  expect(shownLanguages('de','de',zarathustra)).toEqual(['de']);
  // A Russian reader is never given the English, nor an English reader the Russian, in its place.
  expect(shownLanguages('ru','grc',['grc','en'])).toEqual(['grc']);
  expect(shownLanguages('en','grc',['grc','ru'])).toEqual(['grc']);
  expect(translationLanguage('es',['ru','es','en'])).toBe('es');
  for(const locale of PUBLIC_LOCALES)expect(shownLanguages(locale,'de',zarathustra).length).toBeLessThanOrEqual(2);
});
