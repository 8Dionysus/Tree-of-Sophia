import {describe,expect,it} from 'vitest';
import {hintWords,placeHint} from './space-hints.mjs';

describe('the words of a hint',()=>{
  it('says its text and the keys that do the same',()=>{
    expect(hintWords({hint:'Приблизить',keys:'+'})).toEqual({text:'Приблизить',keys:['+'],note:''});
    expect(hintWords({hint:'Повернуть пространство',keys:['←','↑'].join('\u0001'),note:'нажмите — исходный поворот'})).toEqual({text:'Повернуть пространство',keys:['←','↑'],note:'нажмите — исходный поворот'});
  });
  it('says nothing when it only repeats what the thing already shows',()=>{
    expect(hintWords({hint:'Маршруты',shown:'⌁ Маршруты'})).toBeNull();
    expect(hintWords({hint:'',shown:'Маршруты'})).toBeNull();
  });
  it('still speaks when the thing shows only an icon, or when it names a key',()=>{
    expect(hintWords({hint:'Маршруты',shown:'⌁'})?.text).toBe('Маршруты');
    expect(hintWords({hint:'Приблизить',keys:'+',shown:'Приблизить'})?.keys).toEqual(['+']);
  });
});

describe('where a hint stands',()=>{
  const viewport={width:1000,height:700},size={width:200,height:40};
  it('stands on the preferred side when it fits there',()=>{
    expect(placeHint({left:400,top:300,right:440,bottom:330},size,viewport,'top')).toEqual({x:320,y:252,side:'top'});
    expect(placeHint({left:400,top:300,right:440,bottom:330},size,viewport,'right')).toEqual({x:448,y:295,side:'right'});
  });
  it('turns to the first side that fits near an edge of the view',()=>{
    expect(placeHint({left:400,top:10,right:440,bottom:40},size,viewport,'top').side).toBe('bottom');
    expect(placeHint({left:900,top:300,right:940,bottom:330},size,viewport,'right').side).toBe('left');
  });
  it('never leaves the view',()=>{
    const placed=placeHint({left:-50,top:-50,right:-10,bottom:-10},size,viewport,'top');
    expect(placed.x).toBeGreaterThanOrEqual(8);expect(placed.y).toBeGreaterThanOrEqual(8);
    const huge=placeHint({left:400,top:300,right:440,bottom:330},{width:2000,height:900},viewport,'top');
    expect(huge.x).toBe(8);expect(huge.y).toBe(8);
  });
});
