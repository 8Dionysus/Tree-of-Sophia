import {expect,test} from 'vitest';
import {checkTicket,checksPass,wordsOf} from './ticket-checks.mjs';

const link={type:'link',id:'affirmation:link:01:child',item:{record:{verb:{ru:'одно из имён дитяти',en:'is one of the child’s names'},grounds:[{q:'Unschuld ist das Kind'}]}}};
const levels=checks=>Object.fromEntries(checks.map(check=>[check.id,check.level]));

test('the subject\'s own words are read without its places and identifiers',()=>{
  expect(wordsOf(link.item.record)).toEqual(['одно из имён дитяти','is one of the child’s names']);
});

test('an outdated translation passes when the old word is there and the new one is the Tree\'s word',()=>{
  const checks=checkTicket({target:link,draft:{category:'translation',observation:'устарело',change:{from:'дитяти',to:'ребёнка',term:'child',to_lemma:'ребёнок'}},
    grounds:['g1','g2'],known:id=>id===link.id,termWord:'ребёнок'});
  expect(levels(checks)).toEqual({address:'ok',grounds:'ok','from-found':'ok','tree-word':'ok',repeat:'ok'});
  expect(checksPass(checks)).toBe(true);
});

test('a change without grounds, the same word twice or an unknown subject is held back; a missing word only warns',()=>{
  expect(checksPass(checkTicket({target:link,draft:{category:'translation',observation:'x',change:{from:'дитяти',to:'ребёнка'}},grounds:[],known:()=>true}))).toBe(false);
  expect(levels(checkTicket({target:link,draft:{category:'translation',observation:'x',change:{from:'Дитяти',to:'дитяти'}},grounds:['g'],known:()=>true}))['change-same']).toBe('block');
  expect(levels(checkTicket({target:link,draft:{category:'translation',observation:'x'},known:()=>false})).address).toBe('block');
  expect(levels(checkTicket({target:link,draft:{category:'translation',observation:'x',change:{from:'отрок',to:'ребёнок'}},grounds:['g'],known:()=>true}))['from-found']).toBe('warn');
  expect(checksPass(checkTicket({target:link,draft:{category:'open',observation:''},known:()=>true}))).toBe(false);
});

test('an open ticket of the same category on the same subject is reported as a repeat',()=>{
  const checks=checkTicket({target:link,draft:{category:'translation',observation:'x'},known:()=>true,similar:[{kind:'ticket',category:'translation',withdrawn:false},{kind:'ticket',category:'relation',withdrawn:false},{kind:'ticket',category:'translation',withdrawn:true}]});
  expect(checks.find(check=>check.id==='repeat')).toMatchObject({level:'warn',detail:'1'});
});
