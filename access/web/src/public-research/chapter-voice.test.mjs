import {expect,test,vi} from 'vitest';
import {createChapterVoice,readingPlan,sentenceText,silentDuration} from './chapter-voice.mjs';
import {readChapter} from '../../../../bonsai/lib/witnesses.mjs';
import chapterFile from '../../../../bonsai/zarathustra/witnesses/I.02.json' with {type:'json'};

const data={
  units:{de:[{raw:'Erster Satz. Zwei¬\nter Satz.',kind:'p'},{raw:'Ohne Paar.',kind:'p'}],en:[{raw:'First sentence. Second sentence.',kind:'p'},{raw:'Unpaired.',kind:'p'}]},
  rows:[
    {de:[0],en:[0],sentences:{en:[[[[0,13,28]],[[0,16,32]]],[[[0,0,12]],[[0,0,15]]]]}},
    {de:[1],en:[1]},
  ],
};

test('the plan reads each sentence with its partner, in the order of the original, and a unit without pairs whole',()=>{
  const plan=readingPlan(data,{target:'en'});
  expect(plan.map(sentence=>sentenceText(data,sentence))).toEqual(['Erster Satz.','Zweiter Satz.','Ohne Paar.']);
  expect(plan[1].target).toEqual([{unit:0,start:16,end:32}]);
  expect(plan[2]).toEqual({row:1,source:[{unit:1,start:0,end:10}],target:[]});
  // Without the translation's pairs the original is still cut into its sentences.
  expect(readingPlan(data,{target:'ru'}).map(sentence=>sentence.target)).toEqual([[],[],[]]);
});

test('on the stand\'s own chapter the plan reads every German unit and keeps the pairs of the shown translation',()=>{
  const chapter=readChapter(chapterFile),plan=readingPlan(chapter,{target:'ru'});
  const read=new Set(plan.flatMap(sentence=>sentence.source.map(part=>part.unit)));
  expect(read.size).toBe(chapter.units.de.length);
  expect(plan.filter(sentence=>sentence.target.length).length).toBeGreaterThan(plan.length/2);
  expect(sentenceText(chapter,plan[0])).toMatch(/^Drei Verwandlungen nenne ich euch des Geistes:/);
});

test('without a German voice the light moves at the pace of reading, sentence by sentence, and stops at the end',()=>{
  vi.useFakeTimers();
  try{
    const lit=[],states=[];
    const voice=createChapterVoice({speech:null,Utterance:null,light:sentence=>lit.push(sentence?sentenceText(data,sentence):null),onState:value=>states.push(value)});
    voice.load(data,{target:'en'});voice.play();
    expect(lit.at(-1)).toBe('Erster Satz.');expect(states.at(-1)).toMatchObject({state:'playing',index:0,total:3,voiced:false});
    vi.advanceTimersByTime(silentDuration('Erster Satz.'));expect(lit.at(-1)).toBe('Zweiter Satz.');
    voice.pause();vi.advanceTimersByTime(60000);expect(lit.at(-1)).toBe('Zweiter Satz.');expect(voice.state().state).toBe('paused');
    voice.play();vi.advanceTimersByTime(silentDuration('Zweiter Satz.'));expect(lit.at(-1)).toBe('Ohne Paar.');
    vi.advanceTimersByTime(silentDuration('Ohne Paar.'));expect(lit.at(-1)).toBeNull();expect(voice.state()).toMatchObject({state:'idle',index:0});
  }finally{vi.useRealTimers();}
});

test('with a German voice each sentence is spoken in German at the chosen pace, and a stop cancels speech',()=>{
  const spoken=[];
  class Utterance{constructor(text){this.text=text;}}
  const speech={getVoices:()=>[{lang:'en-US'},{lang:'de-DE',name:'Anna'}],speak:utterance=>spoken.push(utterance),cancel:vi.fn()};
  const voice=createChapterVoice({speech,Utterance,light:()=>{}});
  voice.load(data,{target:'en',start:1});voice.setRate(1.25);voice.play();
  expect(spoken[0]).toMatchObject({text:'Ohne Paar.',lang:'de-DE',rate:1.25,voice:{name:'Anna'}});
  voice.load(data,{target:'en'});voice.play();spoken.at(-1).onend();
  expect(spoken.map(utterance=>utterance.text)).toEqual(['Ohne Paar.','Erster Satz.','Zweiter Satz.']);
  const calls=speech.cancel.mock.calls.length;voice.stop();
  expect(speech.cancel.mock.calls.length).toBeGreaterThan(calls);expect(voice.state().state).toBe('idle');
});
