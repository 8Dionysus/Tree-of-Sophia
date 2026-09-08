import {test,expect} from 'vitest';
import {DEFAULT_INTERFACE,INTERFACE_KEY,readInterface,validateInterface} from './interface-model.mjs';

test('existing preferences gain control defaults without losing docking, tools or panel sizes',()=>{
  const old={v:1,pinned:['reader','search'],dock:'left',text:'large',labels:'large',sizes:{reader:{width:640,height:620}}};
  const raw=JSON.stringify(old),storage={getItem:key=>key===INTERFACE_KEY?raw:null};
  expect(readInterface(storage)).toEqual({...DEFAULT_INTERFACE,...old});
  expect(storage.getItem(INTERFACE_KEY)).toBe(raw);
});
test('persisted control choices roundtrip and reject unsupported values before application',()=>{
  const preferences={...DEFAULT_INTERFACE,scrollAction:'zoom',dragAction:'pan',sensitivity:'fast',motion:'paused',uiLanguage:'es',theme:'light',positions:{settings:{x:.3,y:.4}},sizes:{settings:{width:430,height:560}}};
  expect(validateInterface(JSON.parse(JSON.stringify(preferences)))).toEqual(preferences);
  for(const delta of [{motion:'sometimes'},{inputMode:'tablet'},{scrollAction:'device'},{uiLanguage:'unknown'},{positions:{settings:{x:1.5,y:0}}},{v:2},{pinned:['settings']},{sizes:{settings:{width:99999,height:560}}}])
    expect(()=>validateInterface({...preferences,...delta})).toThrow();
});

test('legacy device preferences migrate to actions without changing existing research settings',()=>{
 const old={...DEFAULT_INTERFACE};delete old.scrollAction;expect(validateInterface({...old,inputMode:'mouse'}).scrollAction).toBe('zoom');expect(validateInterface({...old,inputMode:'trackpad'}).scrollAction).toBe('auto');
});
