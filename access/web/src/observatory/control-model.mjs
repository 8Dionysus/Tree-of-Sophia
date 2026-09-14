// WheelEvent reports deltas, not device identity. Auto chooses an action for
// each gesture; the user can override that action for a smooth mouse wheel.
// https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event
export function createScrollIntent(){
  let last=-Infinity,action=null;
  const resolve=(event,preference='auto',now=performance.now())=>{
    if(event.ctrlKey)return 'pinch';
    if(event.shiftKey)return 'pan';
    if(preference!=='auto'){action=preference==='zoom'?'wheel':'pan';last=now;return action;}
    if(now-last>220||!action){
      const notch=event.deltaMode!==0||(Math.abs(event.deltaX)<1&&Number.isInteger(event.deltaY)&&Math.abs(event.deltaY)>=50);
      action=notch?'wheel':'pan';
    }
    last=now;return action;
  };
  resolve.reset=()=>{last=-Infinity;action=null;};return resolve;
}
export const controlGain=speed=>({gentle:.65,normal:1,fast:1.5})[speed]??1;
