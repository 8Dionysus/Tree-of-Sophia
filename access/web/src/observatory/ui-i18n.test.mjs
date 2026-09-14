import {test,expect,afterEach} from 'vitest';
import fs from 'node:fs';
import {UI_CATALOG} from './ui-catalog.mjs';
import {t,ui,uiComputed,setUiLanguage,uiLanguage} from './ui-i18n.mjs';

afterEach(()=>setUiLanguage('ru'));
test('switching UI language updates marked messages while preserving raw source and user wording',()=>{
  const source='Настройки',userNote='Поиск',message=ui('Для: {0}',[source]),action=ui('Настройки');
  expect(String(action)).toBe('Настройки');
  setUiLanguage('en');expect(String(action)).toBe('Settings');expect(String(message)).toBe('For: Настройки');
  setUiLanguage('es');expect(String(action)).toBe('Ajustes');expect(String(message)).toBe('Para: Настройки');
  expect(source).toBe('Настройки');expect(userNote).toBe('Поиск');
  expect(t('Для: {0}',['<img src=x onerror=alert(1)>'])).toBe('Para: <img src=x onerror=alert(1)>');
  expect(()=>setUiLanguage('xx')).toThrow();expect(uiLanguage()).toBe('es');
});
test('derived labels and plural forms use the current interface language',()=>{
  const label=ui('Источники').toLowerCase(),count=uiComputed(()=>new Intl.PluralRules(uiLanguage()).select(21));
  setUiLanguage('en');expect(String(label)).toBe('sources');expect(String(count)).toBe('other');
  setUiLanguage('es');expect(String(label)).toBe('fuentes');expect(String(count)).toBe('other');
  setUiLanguage('ru');expect(String(count)).toBe('one');
});
test('both language catalogs retain every interpolation and cover authored message calls',()=>{
  const placeholders=text=>[...text.matchAll(/\{\d+\}/g)].map(match=>match[0]).sort();
  for(const [source,translations]of Object.entries(UI_CATALOG))for(const language of ['en','es']){
    expect(translations[language]?.trim(),source+' / '+language).toBeTruthy();
    expect(placeholders(translations[language]),source+' / '+language).toEqual(placeholders(source));
  }
  const dir=new URL('.',import.meta.url);
  for(const file of fs.readdirSync(dir).filter(file=>/\.(mjs|js)$/.test(file)&&!file.includes('.test.')&&!file.startsWith('ui-'))){
    const source=fs.readFileSync(new URL(file,dir),'utf8');
    for(const match of source.matchAll(/\b(?:ui|t)\(("(?:[^"\\]|\\.)*")/g))expect(UI_CATALOG[JSON.parse(match[1])],file+': '+match[1]).toBeTruthy();
  }
});
