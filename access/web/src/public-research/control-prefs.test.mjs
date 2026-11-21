import {expect,test} from 'vitest';
import {createControlPrefs,normalizePrefs,keyLabel,DEFAULT_KEYS,CONTROL_PREFS_KEY} from './control-prefs.mjs';

const memory=()=>{const values=new Map();return {getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key),values};};

test('stored preferences are bounded and a key clash restores both defaults',()=>{
  const prefs=normalizePrefs({turnSpeed:9,zoomSpeed:-1,textScale:3,contrast:'loud',keys:{layer:'KeyB',bench:'KeyB',search:'F5'}});
  expect(prefs).toMatchObject({turnSpeed:3,zoomSpeed:.2,textScale:1.5,contrast:'normal'});
  expect(prefs.keys.layer).toBe(DEFAULT_KEYS.layer);
  expect(prefs.keys.bench).toBe(DEFAULT_KEYS.bench);
  expect(prefs.keys.search).toBe('Slash');
});

test('a preset sets its speeds; changing a speed makes the set custom; the page reads size and contrast',()=>{
  const storage=memory(),root={style:{values:{},setProperty(name,value){this.values[name]=value;}},dataset:{}};
  const controls=createControlPrefs({storage,root,view:null});
  controls.set({preset:'trackpad'});
  expect(controls.get()).toMatchObject({preset:'trackpad',turnSpeed:.7,zoomSpeed:.45,inertia:true});
  controls.set({turnSpeed:1.5});
  expect(controls.get().preset).toBe('custom');
  controls.set({textScale:1.25,contrast:'high',keys:{...controls.get().keys,layer:'KeyY'}});
  expect(root.style.values['--ui-text-scale']).toBe('1.25');
  expect(root.dataset.uiContrast).toBe('high');
  expect(JSON.parse(storage.values.get(CONTROL_PREFS_KEY)).keys.layer).toBe('KeyY');
  expect(createControlPrefs({storage,root,view:null}).get().keys.layer).toBe('KeyY');
  controls.reset();
  expect(controls.get().keys.layer).toBe('KeyL');
});

test('a key shows the sign it stands for',()=>{
  expect(keyLabel('KeyL')).toBe('L');expect(keyLabel('Slash')).toBe('/');expect(keyLabel('Digit3')).toBe('3');
});
