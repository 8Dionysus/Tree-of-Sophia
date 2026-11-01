import {describe,expect,it} from 'vitest';
import {isPreReform,modernRussian,modernWord,respell} from './orthography.mjs';

describe('Russian printed before 1918 in today\'s spelling',()=>{
  it('tells the old spelling from the new',()=>{
    expect(isPreReform('Что такъ рано пришелъ ты, Критонъ?')).toBe(true);
    expect(isPreReform('впечатлѣніе')).toBe(true);
    expect(isPreReform('Что так рано пришел ты, Критон?')).toBe(false);
    expect(isPreReform('объём и подъезд')).toBe(false);
    expect(isPreReform('СЪ НИМЪ')).toBe(true);
  });
  it('changes the letters and drops a closing hard sign, never an inner one',()=>{
    expect(modernRussian('Не знаю, Аѳиняне, какое впечатлѣніе сдѣлали на васъ мои обвинители')).toBe('Не знаю, Афиняне, какое впечатление сделали на вас мои обвинители');
    expect(modernWord('подъѣздъ')).toBe('подъезд');
    expect(modernWord('Еѵангеліе')).toBe('Евангелие');
    expect(modernRussian('какъ-то')).toBe('как-то');
  });
  it('respells the endings of adjectives and keeps nouns in -ия',()=>{
    expect(modernWord('добраго')).toBe('доброго');
    expect(modernWord('синяго')).toBe('синего');
    expect(modernWord('хорошаго')).toBe('хорошего');
    expect(modernWord('большаго')).toBe('большого');
    expect(modernWord('которыя')).toBe('которые');
    expect(modernWord('высшія')).toBe('высшие');
    expect(modernWord('другія')).toBe('другие');
    expect(modernWord('древнія')).toBe('древние');
    expect(modernWord('знанія')).toBe('знания');
    expect(modernWord('различія')).toBe('различия');
    expect(modernWord('прямодушія')).toBe('прямодушия');
    expect(modernWord('олигархія')).toBe('олигархия');
    expect(modernWord('благо')).toBe('благо');
  });
  it('keeps -с in prefixes before a voiceless consonant and leaves roots alone',()=>{
    expect(modernWord('разсказъ')).toBe('рассказ');
    expect(modernWord('безсмертіе')).toBe('бессмертие');
    expect(modernWord('изслѣдованіе')).toBe('исследование');
    expect(modernWord('низкій')).toBe('низкий');
    expect(modernWord('безъ')).toBe('без');
  });
  it('respells the reform\'s own words and keeps capitals',()=>{
    expect(modernRussian('Онѣ пришли, ея слова, безъ нея')).toBe('Они пришли, её слова, без неё');
    expect(modernWord('СЪ')).toBe('С');
    expect(modernRussian('СЪ НИМЪ')).toBe('С НИМ');
  });
  it('maps a place in the shown text back to the source',()=>{
    const source='Сокр. Что такъ рано пришелъ ты, Критонъ?';
    const shown=respell(source);
    expect(shown.text).toBe('Сокр. Что так рано пришел ты, Критон?');
    expect(shown.toSource).toHaveLength(shown.text.length+1);
    const at=shown.text.indexOf('рано');
    expect(source.slice(shown.toSource[at],shown.toSource[at+4])).toBe('рано');
    expect(shown.toSource.at(-1)).toBe(source.length);
    expect(respell('без перемен').text).toBe('без перемен');
  });
  it('keeps whole-word spelling and source offsets when highlights split words',()=>{
    const source='😀 подъѣздъ. Что такъ?',shown=respell(source);
    expect(shown.text).toBe('😀 подъезд. Что так?');
    // The boundary after the inner hard sign must not turn it into a word-final sign.
    const boundaries=[0,source.indexOf('ѣ'),source.indexOf('так')+3,source.length];
    const parts=boundaries.slice(1).map((end,index)=>shown.text.slice(shown.toDisplay[boundaries[index]],shown.toDisplay[end]));
    expect(parts.join('')).toBe(shown.text);
    expect(parts[0]).toBe('😀 подъ');
    const word=shown.text.indexOf('так');
    expect(source.slice(shown.toSource[word],shown.toSource[word+3])).toBe('такъ');
    expect(shown.toDisplay[source.indexOf('ъ',source.indexOf('так'))]).toBe(word+3);
  });
});
