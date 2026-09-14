import {test,expect} from 'vitest';
import {findInParagraphs} from './search.mjs';

test('search treats metacharacters as literal text and returns every nonoverlapping occurrence',()=>{
  expect(findInParagraphs(['a.b aXb a.b','[x] \\ * + ? (a) ^ $ {1} |'], 'a.b')).toEqual({matches:[{paragraph:0,index:0,length:3},{paragraph:0,index:8,length:3}],truncated:false});
  for(const query of ['[x]','\\','*','+','?','(a)','^','$','{1}','|'])expect(findInParagraphs(['[x] \\ * + ? (a) ^ $ {1} |'],query).matches).toHaveLength(1);
});

test('Cyrillic and Unicode case folding preserve UTF-16 display offsets and original text',()=>{
  const paragraphs=['😀 Мысль, МЫСЛЬ; мысль','𐐀 𐐨'];
  expect(findInParagraphs(paragraphs,'мысль')).toEqual({matches:[{paragraph:0,index:3,length:5},{paragraph:0,index:10,length:5},{paragraph:0,index:17,length:5}],truncated:false});
  expect(findInParagraphs(paragraphs,'𐐨').matches).toEqual([{paragraph:1,index:0,length:2},{paragraph:1,index:3,length:2}]);
  expect(paragraphs[0]).toBe('😀 Мысль, МЫСЛЬ; мысль');
});

test('limit reports truncation only after observing one additional result, also across paragraphs',()=>{
  expect(findInParagraphs(['a','a'],'a',{limit:2})).toEqual({matches:[{paragraph:0,index:0,length:1},{paragraph:1,index:0,length:1}],truncated:false});
  expect(findInParagraphs(['a a','a'],'a',{limit:2})).toEqual({matches:[{paragraph:0,index:0,length:1},{paragraph:0,index:2,length:1}],truncated:true});
  expect(findInParagraphs(['a'.repeat(501)],'a').matches).toHaveLength(500);
  expect(findInParagraphs(['a'.repeat(501)],'a').truncated).toBe(true);
});

test('empty queries return no matches; nonempty query spacing and decomposed text are not normalized',()=>{
  for(const query of ['','  ','\n\t'])expect(findInParagraphs(['anything'],query)).toEqual({matches:[],truncated:false});
  expect(findInParagraphs(['a aa a '],' a ').matches).toEqual([{paragraph:0,index:4,length:3}]);
  expect(findInParagraphs(['e\u0301'],'é').matches).toEqual([]);
  expect(findInParagraphs(['first','second'],'first\nsecond').matches).toEqual([]);
});

test('invalid and oversized queries are rejected before searching',()=>{
  expect(()=>findInParagraphs(['text'],'x'.repeat(161))).toThrow('limit');
  for(const limit of [0,-1,NaN,1.5,501])expect(()=>findInParagraphs(['text'],'t',{limit})).toThrow('invalid-input');
  expect(()=>findInParagraphs([123],'t')).toThrow('invalid-input');
  expect(()=>findInParagraphs(['text'],null)).toThrow('invalid-input');
});
