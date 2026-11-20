// A window into the sky (G5, the probe): a phone held up turns the space as the person turns it, the way a window
// shows what is behind it. The turn comes from the device's orientation sensor; where the site's permissions policy
// closes the sensor (the published site's gyroscope=() and accelerometer=(), gate Q5), the window says so and stays
// shut. A session in immersive space (WebXR) is the next step and waits for this probe's measure.
import {localeText,resolveLocale} from './locale.mjs';

const DEGREE=Math.PI/180;
const wrap=value=>((value+540)%360)-180;

/**
 * The turn between two orientation readings, in radians: yaw from the compass heading (alpha), pitch from the tilt
 * toward and away (beta), each as the screen stands (a landscape screen trades tilt for roll). Jumps over 45 degrees
 * are a sensor's reset, not a turn.
 */
export function orientationTurn(previous,next,{angle=0,sensitivity=1}={}){
  if(!previous||!next)return {yaw:0,pitch:0};
  const tilt=reading=>angle===90?-reading.gamma:angle===270||angle===-90?reading.gamma:angle===180?-reading.beta:reading.beta;
  const yaw=wrap((next.alpha??0)-(previous.alpha??0)),pitch=(tilt(next)??0)-(tilt(previous)??0);
  if(Math.abs(yaw)>45||Math.abs(pitch)>45)return {yaw:0,pitch:0};
  return {yaw:-yaw*DEGREE*sensitivity,pitch:-pitch*DEGREE*sensitivity};
}

/** Whether the page may read the orientation sensor: true, false, or null where the browser does not say. */
export function sensorAllowed(doc){
  const policy=doc?.permissionsPolicy??doc?.featurePolicy;
  if(!policy?.allowsFeature)return null;
  try{return policy.allowsFeature('gyroscope')&&policy.allowsFeature('accelerometer');}catch{return null;}
}

const labels={
  window:['Окно в небо','A window into the sky'],
  on:['Окно в небо открыто: поворачивайте телефон','The window into the sky is open: turn the phone'],
  off:['Окно в небо закрыто','The window into the sky is closed'],
  blocked:['На этом сайте датчик положения закрыт политикой разрешений; окно в небо открывается там, где он разрешён (страницы моста)','On this site the orientation sensor is closed by the permissions policy; the window opens where it is allowed (the bridge\'s pages)'],
  unsupported:['У этого устройства нет датчика положения','This device has no orientation sensor'],
  denied:['Доступ к датчику положения не дан','Access to the orientation sensor was not given'],
  close:['Закрыть окно','Close the window'],
};

export function mountSkyWindow({shell,actionLayer,space,locale=()=> 'ru',view=globalThis}){
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  let open=false,last=null,pending={yaw:0,pitch:0},frame=0;
  const chip=doc.createElement('div');chip.className='ps-sky-window';chip.hidden=true;
  const flush=()=>{frame=0;if(pending.yaw||pending.pitch)space?.actions?.turnBy?.(pending.yaw,pending.pitch);pending={yaw:0,pitch:0};};
  const onOrientation=event=>{
    const reading={alpha:event.alpha,beta:event.beta,gamma:event.gamma};if(reading.alpha==null&&reading.beta==null)return;
    const turn=orientationTurn(last,reading,{angle:view.screen?.orientation?.angle??0});last=reading;
    pending={yaw:pending.yaw+turn.yaw,pitch:pending.pitch+turn.pitch};
    // One turn per frame; the flush clears the mark whether the frame comes now or later.
    if(!frame){frame=1;(view.requestAnimationFrame?.bind(view)??(callback=>callback()))(flush);}
  };
  function paint(){
    chip.hidden=!open;chip.replaceChildren();
    if(open){const label=doc.createElement('span');label.textContent=t('on');const close=doc.createElement('button');close.type='button';close.className='text-button';close.textContent=t('close');close.addEventListener('click',()=>void toggle(false));chip.append(label,close);}
    if(!chip.isConnected)shell.append(chip);
  }
  async function toggle(force=!open){
    if(!force){open=false;last=null;view.removeEventListener?.('deviceorientation',onOrientation);paint();actionLayer?.say?.(t('off'));return false;}
    const Orientation=view.DeviceOrientationEvent;
    if(!Orientation){actionLayer?.say?.(t('unsupported'));return false;}
    if(sensorAllowed(doc)===false){actionLayer?.say?.(t('blocked'));return false;}
    // Some browsers ask the person first, and only from a gesture.
    if(typeof Orientation.requestPermission==='function'){try{if(await Orientation.requestPermission()!=='granted'){actionLayer?.say?.(t('denied'));return false;}}catch{actionLayer?.say?.(t('denied'));return false;}}
    open=true;last=null;view.addEventListener?.('deviceorientation',onOrientation);paint();actionLayer?.say?.(t('on'));return true;
  }
  const remove=actionLayer?.addActions?.([{id:'sky-window',text:()=>t('window'),for:['any'],active:()=>open,run:()=>toggle()}]);
  return {toggle,isOpen:()=>open,destroy(){remove?.();void toggle(false);chip.remove();}};
}
