import {expect,test} from 'vitest';
import {readChapter,displayText} from '../../../../../bonsai/lib/witnesses.mjs';
import chapterFile from '../../../../../bonsai/zarathustra/witnesses/I.23.json';
import {citeOfSelection,findInChapter,fingerprintOf,isWord,labelOf,nearness,rangeOf,rawOffset,snapToSentence,snapToWords,whatTreeKnows,witnessWords,wordsOf} from './place.mjs';

const chapter=readChapter(chapterFile);

test('a selection widens to whole words and, by a gesture, to its sentence',()=>{
  const text='Eine schenkende Tugend ist die höchste Tugend. Ungemein ist sie.';
  const snapped=snapToWords(text,7,18);
  expect(text.slice(snapped.start,snapped.end)).toBe('schenkende Tugend');
  const sentence=snapToSentence(text,snapped.start,snapped.end);
  expect(text.slice(sentence.start,sentence.end)).toBe('Eine schenkende Tugend ist die höchste Tugend.');
  expect(isWord('Tugend')).toBe(true);expect(isWord('die Tugend')).toBe(false);
});

test('a shown offset maps back to the raw character it begins at',()=>{
  const unit=chapter.units.de.find(item=>item.raw.includes('¬'))??chapter.units.de[0];
  const view=displayText(unit.raw,unit.kind);
  for(const shown of [0,5,Math.floor(view.text.length/2),view.text.length]){
    const raw=rawOffset(unit,shown);expect(view.at(raw)).toBeGreaterThanOrEqual(shown);
  }
});

test('a selection in the original is its own citation; one in a translation finds the original through the sentence pairs',()=>{
  const de=chapter.units.de,ru=chapter.units.ru;
  const own=citeOfSelection(chapter,'de',{index:2,offset:0},{index:2,offset:20});
  expect(own.cite).toMatchObject({c:'I.23',u:[de[2].id,de[2].id],s:0,e:20});
  const row=chapter.rows.find(item=>item.sentences?.ru?.length&&item.ru?.length);
  const [, target]=row.sentences.ru[0],[ti,ts,te]=target[0];
  const translated=citeOfSelection(chapter,'ru',{index:ti,offset:ts},{index:ti,offset:Math.min(te,ts+10)});
  expect(translated.witness).toMatchObject({language:'ru',u:[ru[ti].id,ru[ti].id]});
  const place=rangeOf(translated.cite,chapter);
  expect(row.de).toContain(place.a);
  expect(wordsOf(translated.cite,chapter,'de').length).toBeGreaterThan(0);
  expect(witnessWords({cite:translated.cite,witness:translated.witness},chapter).length).toBeGreaterThan(0);
});

test('places meet in the same words, the same paragraph or only the chapter',()=>{
  expect(nearness({a:2,s:0,b:2,e:20},{a:2,s:10,b:3,e:5})).toBe('exact');
  expect(nearness({a:2,s:0,b:2,e:20},{a:2,s:30,b:2,e:40})).toBe('paragraph');
  expect(nearness({a:2,s:0,b:2,e:20},{a:5,s:0,b:5,e:9})).toBe('chapter');
});

test('what the Tree keeps at a place is found by the meeting of places',()=>{
  const de=chapter.units.de;
  const cite={w:'also-sprach-zarathustra',c:'I.23',u:[de[2].id,de[2].id],s:0,e:de[2].raw.length};
  const passage={id:'p1',kind:'passage',record:{c:'I.23',u:[de[2].id,de[2].id]}};
  const far={id:'p2',kind:'passage',record:{c:'I.23',u:[de[9].id,de[9].id]}};
  const concept={id:'bestowing-virtue',kind:'conception'},link={id:'l1',kind:'relation'};
  const items=new Map([[passage.id,passage],[far.id,far],[concept.id,concept],[link.id,link]]);
  const index={items,get:id=>items.get(id),references:id=>id==='p1'?[concept]:id==='p2'?[link]:[]};
  const core={voices:{'I.23':[{u:[de[2].id,de[2].id],speaker:'zarathustra',mode:'speech',layer:'primary',attribution:{addressee:'disciples'}}]},semanticUses:{terms:[]}};
  const known=whatTreeKnows({fragment:{cite},chapter,index,core});
  expect(known.nodes).toEqual([expect.objectContaining({item:concept,near:'exact'})]);
  expect(known.links).toEqual([expect.objectContaining({item:link,near:'chapter'})]);
  expect(known.voices).toEqual([expect.objectContaining({speaker:'zarathustra',addressee:'disciples'})]);
});

test('a phrase is found wherever it stands in a chapter, as whole words',()=>{
  const hits=findInChapter(chapter,'de','Tugend');
  expect(hits.length).toBeGreaterThan(3);
  expect(hits.every(hit=>hit.hit.toLowerCase()==='tugend')).toBe(true);
  expect(labelOf('one two three four five six seven eight nine')).toBe('one two three four five six seven eight…');
  expect(fingerprintOf('a')).not.toBe(fingerprintOf('b'));
});
