import {expect,test} from 'vitest';
import {chapterTitle} from './chapter-title.mjs';

test('a chapter reads as its witness titles it, an English title in capitals in title case',()=>{
  const chapter={titles:{de:'Ausser Dienst.',en:'LXVI. OUT OF SERVICE.',ru:'Чародей.'}};
  expect(chapterTitle(chapter,'de')).toBe('Ausser Dienst');
  expect(chapterTitle(chapter,'en')).toBe('Out of Service');
  expect(chapterTitle(chapter,'ru')).toBe('Чародей');
  expect(chapterTitle({titles:{en:'I. THE THREE METAMORPHOSES.'}},'en')).toBe('The Three Metamorphoses');
});

test('a witness that leaves the chapter out gives no title, so the caller can turn to the original',()=>{
  expect(chapterTitle({titles:{de:'Das Eselsfest.'}},'ru')).toBe('');
  expect(chapterTitle(null,'en')).toBe('');
});
