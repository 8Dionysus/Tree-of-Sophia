import {test} from 'vitest';
import assert from 'node:assert/strict';
import {displayForm} from './display-language.mjs';

const wording='Исходная формулировка';
const forms={default:wording,ru:wording,en:null,original:wording};
const selection=(overrides={})=>({requested_language:'en',selected_key:'default',actual_language:'ru',text:wording,
  reason:'fallback',available_keys:['default','original','ru'],content_available:true,
  source_form_pointer:'/display/summary/default',...overrides});

test('two-argument navigation keeps its legacy preference and unknown fallback language',()=>{
  assert.deepEqual(displayForm(forms,'ru'),{text:wording,key:'ru',lang:'ru',fallback:false});
  assert.deepEqual(displayForm(forms,'en'),{text:wording,key:'default',lang:null,fallback:true});
  assert.deepEqual(displayForm(forms,'original'),{text:wording,key:'original',lang:null,fallback:false});
  assert.deepEqual(displayForm(forms,'en',null),displayForm(forms,'en'));
});

test('exact and fallback selections preserve the delivered source language without changing wording',()=>{
  assert.deepEqual(displayForm(forms,'ru',selection({requested_language:'ru',selected_key:'ru',reason:'exact-language'})),
    {text:wording,key:'ru',lang:'ru',fallback:false});
  assert.deepEqual(displayForm(forms,'en',selection()),{text:wording,key:'default',lang:'ru',fallback:true});
  assert.deepEqual(displayForm(forms,'original',selection({requested_language:'original',selected_key:'original',reason:'original-role'})),
    {text:wording,key:'original',lang:'ru',fallback:false});
});

test('automatic selection includes the UI default preference without claiming a missing translation',()=>{
  const selected=selection({requested_language:'auto',reason:'automatic'});
  for(const preferred of ['auto','default'])assert.deepEqual(displayForm(forms,preferred,selected),
    {text:wording,key:'default',lang:'ru',fallback:false});
});

test('navigation wording remains displayable without claiming source content availability',()=>{
  const value={default:'Exact record version 2',en:'Exact record version 2'};
  const selected=selection({selected_key:'en',actual_language:'en',text:value.en,reason:'exact-language',
    available_keys:Object.keys(value),content_available:false,source_form_pointer:'/display/title/en'});
  assert.deepEqual(displayForm(value,'en',selected),{text:value.en,key:'en',lang:'en',fallback:false});
  assert.equal(selected.content_available,false);
});

test('explicit unknown language survives fallback, original and a language-keyed selection',()=>{
  for(const [preferred,key,reason]of [['en','default','fallback'],['original','original','original-role'],['ru','ru','exact-language']]){
    assert.equal(displayForm(forms,preferred,selection({requested_language:preferred,selected_key:key,reason,actual_language:null})).lang,null);
  }
  const native={default:'Author identity',original:'Author identity'};
  assert.equal(displayForm(native,'en',selection({text:native.default,actual_language:null,available_keys:Object.keys(native)})).lang,null);
});

test('supplied less-specific and private-use language tags retain owner spelling',()=>{
  const value={default:'word',EN:'word','x-owner':'word'};
  for(const [requested,key,actual,reason,fallback]of [['en-US','EN','EN','less-specific-language',true],['x-owner','x-owner','x-owner','exact-language',false]]){
    assert.deepEqual(displayForm(value,requested,selection({requested_language:requested,selected_key:key,actual_language:actual,text:'word',reason,available_keys:Object.keys(value)})),
      {text:'word',key,lang:actual,fallback});
  }
});

test('stale requests, changed dictionaries and unavailable or malformed selections fail closed',()=>{
  for(const changed of [
    {requested_language:'ru'}, {selected_key:'missing'}, {text:'Another version'},
    {available_keys:['default','ru']}, {available_keys:['default','ru','ru']},
    {actual_language:undefined}, {actual_language:'default'}, {actual_language:'original'},
    {actual_language:'auto'}, {actual_language:'ru\n'}, {reason:'invented'},
    {content_available:undefined}, {selected_key:null,text:null,actual_language:null,reason:'missing'},
  ])assert.equal(displayForm(forms,'en',selection(changed)),null,JSON.stringify(changed));
  assert.equal(displayForm({...forms,en:'New English wording'},'en',selection()),null);
  assert.equal(displayForm({...forms,default:'Changed current wording'},'en',selection()),null);
  assert.equal(displayForm(forms,'en',{}),null);
  assert.equal(displayForm(forms,'en',[]),null);
});

test('rendering a supplied selection does not mutate source data or delivery metadata',()=>{
  const value=Object.freeze({...forms}),selected=Object.freeze({...selection(),available_keys:Object.freeze(selection().available_keys)});
  const before=JSON.stringify([value,selected]);displayForm(value,'en',selected);
  assert.equal(JSON.stringify([value,selected]),before);
});
