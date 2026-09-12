import {test,expect} from 'vitest';
import {readerVersionCodes,readerComparisonVersion,readerDisplayVersions} from './version-options.mjs';
const document={originalLanguage:'de',versions:{de:{},en:{},ru:{}}};

test('a declared original follows RU/EN without being inferred from the interface',()=>{
 expect(readerVersionCodes(document)).toEqual(['ru','en','de']);
 expect(readerVersionCodes({versions:{en:{},ru:{}}})).toEqual(['ru','en']);
 expect(readerVersionCodes({status:'link-only'})).toEqual([]);
});

test('parallel reading chooses an original and one translation, with an explicit alternative pair',()=>{
 expect(readerDisplayVersions(document,'ru','single')).toEqual(['ru']);
 expect(readerDisplayVersions(document,'ru','parallel')).toEqual(['ru','de']);
 expect(readerDisplayVersions(document,'de','parallel','en')).toEqual(['en','de']);
 expect(readerDisplayVersions(document,'ru','parallel','en')).toEqual(['ru','en']);
 expect(readerComparisonVersion(document,'de','de')).toBe('ru');
});

test('a different work never reuses an absent original or creates duplicate columns',()=>{
 const greek={originalLanguage:'grc',versions:{ru:{},en:{},grc:{}}},legacy={versions:{ru:{},en:{}}};
 expect(readerDisplayVersions(greek,'en','parallel','de')).toEqual(['en','grc']);
 expect(readerDisplayVersions(legacy,'en','parallel','grc')).toEqual(['ru','en']);
 expect(readerDisplayVersions({originalLanguage:'en',versions:{en:{}}},'en','parallel')).toEqual(['en']);
});
