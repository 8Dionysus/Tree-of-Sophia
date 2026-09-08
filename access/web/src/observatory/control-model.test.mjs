import {test,expect} from 'vitest';
import {createScrollIntent} from './control-model.mjs';
const event=(deltaY,deltaX=0,deltaMode=0)=>({deltaY,deltaX,deltaMode,ctrlKey:false,shiftKey:false});
test('automatic action stays stable within one gesture and can change for the next device',()=>{
 const intent=createScrollIntent();expect(intent(event(.1,1),'auto',0)).toBe('pan');expect(intent(event(120),'auto',80)).toBe('pan');
 expect(intent(event(120),'auto',400)).toBe('wheel');expect(intent(event(20),'auto',430)).toBe('wheel');expect(intent(event(2,3),'auto',800)).toBe('pan');
 expect(intent(event(3,0,1),'auto',1200)).toBe('wheel');
});
test('explicit actions and pinch take precedence over ambiguous wheel packets',()=>{
 const intent=createScrollIntent();expect(intent(event(5),'zoom',0)).toBe('wheel');expect(intent(event(120),'pan',5)).toBe('pan');
 expect(intent({...event(120),ctrlKey:true},'pan',10)).toBe('pinch');expect(intent({...event(120),shiftKey:true},'zoom',20)).toBe('pan');
});
