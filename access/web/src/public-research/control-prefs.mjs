// How this device steers the stand: turning and zoom speed, coasting, inversion, the keys of the stand's actions,
// the size of reading text and the contrast of signs. Kept in this browser for this device; nothing else reads it.

export const CONTROL_PREFS_KEY='tos.public-research.controls.v1';

// Keys are physical keys (KeyboardEvent.code), so a key keeps its place in any keyboard layout.
export const DEFAULT_KEYS=Object.freeze({palette:'KeyK',layer:'KeyL',search:'Slash',frame:'KeyO',bench:'KeyB',expand:'KeyF'});
export const PRESETS=Object.freeze({
  mouse:Object.freeze({turnSpeed:1,zoomSpeed:1,inertia:true}),
  trackpad:Object.freeze({turnSpeed:.7,zoomSpeed:.45,inertia:true}),
  touch:Object.freeze({turnSpeed:1.2,zoomSpeed:1,inertia:true}),
  precise:Object.freeze({turnSpeed:.5,zoomSpeed:.6,inertia:false}),
});
export const DEFAULT_PREFS=Object.freeze({preset:'mouse',...PRESETS.mouse,invertTurn:false,invertZoom:false,keys:DEFAULT_KEYS,textScale:1,contrast:'normal'});

const clamp=(value,low,high,fallback)=>Number.isFinite(Number(value))?Math.min(high,Math.max(low,Number(value))):fallback;
const KEY=/^(Key[A-Z]|Digit[0-9]|Slash|Backslash|Period|Comma|Semicolon|Quote|BracketLeft|BracketRight|Minus|Equal|Backquote)$/;

/** A complete, valid set of preferences from whatever was stored. */
export function normalizePrefs(value){
  const input=value&&typeof value==='object'?value:{};
  const keys={...DEFAULT_KEYS};
  for(const [name,code] of Object.entries(input.keys??{}))if(Object.hasOwn(DEFAULT_KEYS,name)&&typeof code==='string'&&KEY.test(code))keys[name]=code;
  // Two actions on one key would make one of them unreachable: a clash restores the defaults of both.
  const seen=new Map();
  for(const [name,code] of Object.entries(keys)){if(seen.has(code)){keys[name]=DEFAULT_KEYS[name];keys[seen.get(code)]=DEFAULT_KEYS[seen.get(code)];}else seen.set(code,name);}
  return {
    preset:Object.hasOwn(PRESETS,input.preset)||input.preset==='custom'?input.preset:'mouse',
    turnSpeed:clamp(input.turnSpeed,.2,3,1),zoomSpeed:clamp(input.zoomSpeed,.2,3,1),inertia:input.inertia!==false,
    invertTurn:input.invertTurn===true,invertZoom:input.invertZoom===true,keys,
    textScale:clamp(input.textScale,.85,1.5,1),contrast:input.contrast==='high'?'high':'normal',
  };
}

/** The label a key shows: the letter or sign it stands for. */
export function keyLabel(code){
  if(/^Key[A-Z]$/.test(code))return code.slice(3);
  if(/^Digit[0-9]$/.test(code))return code.slice(5);
  return {Slash:'/',Backslash:'\\',Period:'.',Comma:',',Semicolon:';',Quote:'\'',BracketLeft:'[',BracketRight:']',Minus:'-',Equal:'=',Backquote:'`'}[code]??code;
}

export function createControlPrefs({storage=globalThis.localStorage,root=globalThis.document?.documentElement,view=globalThis}={}){
  const listeners=new Set();
  const read=()=>{try{return normalizePrefs(JSON.parse(storage?.getItem(CONTROL_PREFS_KEY)??'null'));}catch{return normalizePrefs(null);}};
  let prefs=read();
  // Text size and contrast are page state the stylesheets read.
  const apply=()=>{if(!root)return;root.style.setProperty('--ui-text-scale',String(prefs.textScale));root.dataset.uiContrast=prefs.contrast;};
  const notify=()=>{apply();for(const listener of listeners)listener(prefs);};
  const onStorage=event=>{if(event.key===CONTROL_PREFS_KEY){prefs=read();notify();}};
  view?.addEventListener?.('storage',onStorage);
  apply();
  return {
    get:()=>prefs,
    /** Change some preferences; choosing a preset sets its speeds and coasting. */
    set(patch){
      let next={...prefs,...patch};
      if(patch.preset&&Object.hasOwn(PRESETS,patch.preset))next={...next,...PRESETS[patch.preset]};
      else if(['turnSpeed','zoomSpeed','inertia'].some(key=>key in patch))next.preset='custom';
      prefs=normalizePrefs(next);
      try{storage?.setItem(CONTROL_PREFS_KEY,JSON.stringify(prefs));}catch{}
      notify();return prefs;
    },
    reset(){prefs=normalizePrefs(null);try{storage?.removeItem(CONTROL_PREFS_KEY);}catch{}notify();return prefs;},
    subscribe(listener){listeners.add(listener);return ()=>listeners.delete(listener);},
    dispose(){view?.removeEventListener?.('storage',onStorage);listeners.clear();},
  };
}
