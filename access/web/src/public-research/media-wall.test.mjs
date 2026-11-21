import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {densityScale,mediaAddress,mountMediaWall,parseMediaAddress,wallItems,yearsOf} from './media-wall.mjs';
import {readMedia} from '../../../../bonsai/lib/media.mjs';
import {checkTicket} from './ticket-checks.mjs';
import zarathustraMedia from '../../../../bonsai/zarathustra/media.json' with {type:'json'};

test('an image, a region of it and a segment of a recording have addresses that read back',()=>{
  expect(mediaAddress({stand:'zarathustra',image:'met:67.55.170/whole'})).toBe('media:zarathustra:met:67.55.170/whole');
  const region=mediaAddress({stand:'zarathustra',image:'met:67.55.170/whole',region:[10.4,20.6,300,400.2]});
  expect(region).toBe('media:zarathustra:met:67.55.170/whole#xywh=pixel:10,21,300,400');
  expect(parseMediaAddress(region)).toEqual({stand:'zarathustra',image:'met:67.55.170/whole',region:[10,21,300,400]});
  expect(parseMediaAddress(mediaAddress({stand:'antiquity',image:'a/b',segment:[1.234,5]}))).toEqual({stand:'antiquity',image:'a/b',segment:[1.23,5]});
  expect(parseMediaAddress('media:zarathustra:x#xywh=1,2')).toBeNull();
  // A ticket accepts a region by its address.
  expect(checkTicket({target:{type:'region',id:region},draft:{observation:'the wheel'}})[0]).toEqual({id:'address',level:'ok'});
  expect(checkTicket({target:{type:'region',id:'media:zarathustra:x'},draft:{observation:'the wheel'}})[0].level).toBe('block');
});

test('the wall holds every image once, with the subjects it serves, in the order of its years',()=>{
  const items=wallItems(readMedia(zarathustraMedia));
  expect(items.length).toBe(Object.keys(zarathustraMedia.images).length);
  expect(new Set(items.map(item=>item.id)).size).toBe(items.length);
  const dated=items.filter(item=>item.years);
  for(let i=1;i<dated.length;i++)expect(dated[i].years.from).toBeGreaterThanOrEqual(dated[i-1].years.from);
  expect(items.every(item=>item.uses.length>0)).toBe(true);
  expect(yearsOf({kind:'interval-assertion',interval:{start:'1700',end:'1799'}})).toEqual({from:1700,to:1799});
  expect(yearsOf({kind:'date-assertion',value:'1900-05'})).toEqual({from:1900,to:1900});
  expect(yearsOf({kind:'interval-assertion',interval:{start:'1868'}})).toEqual({from:1868,to:1868,open:'after'});
  expect(yearsOf({kind:'unknown-date'})).toBeNull();
});

function setup(){
  const window=new Window({url:'http://localhost/zarathustra/'});
  const document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const shell=document.querySelector('#tree');
  const added=[],actionLayer={addActions:list=>{added.push(...list);return ()=>{};},say:vi.fn(),copy:vi.fn(async()=>true)};
  const actDesk={openTicket:vi.fn(async()=>{}),openTransfer:vi.fn(async()=>{})};
  const wall=mountMediaWall({shell,actionLayer,actDesk,locale:()=> 'en',profile:'zarathustra',load:async()=>readMedia(zarathustraMedia)});
  return {document,wall,added,actionLayer,actDesk};
}

test('the wall chooses two images to set side by side and filters by subject',async()=>{
  const {document,wall,added}=setup();
  expect(added.map(action=>action.id)).toEqual(['media-wall']);
  await wall.open({subject:'wheel'});
  const thumbs=[...document.querySelectorAll('.mw-thumb')];
  expect(thumbs.length).toBe(zarathustraMedia.uses.filter(use=>use.subject==='wheel').length);
  const beside=[...document.querySelectorAll('.mw-bar button')].find(node=>node.textContent==='Side by side');
  expect(beside.disabled).toBe(true);
  document.querySelectorAll('.mw-thumb')[0].click();document.querySelectorAll('.mw-thumb')[1].click();
  expect([...document.querySelectorAll('.mw-thumb')].filter(node=>node.getAttribute('aria-pressed')==='true').length).toBe(2);
  expect([...document.querySelectorAll('.mw-bar button')].find(node=>node.textContent==='Side by side').disabled).toBe(false);
  [...document.querySelectorAll('.mw-bar .ad-tab')].find(node=>node.dataset.order==='scale').click();
  expect(document.querySelector('.mw-scale .mw-ruler .mw-tick')).not.toBeNull();
});

test('a region of an image is quoted with its work, holder, rights and address, and goes to a ticket as a media place',async()=>{
  const {wall}=setup();
  const items=wallItems(readMedia(zarathustraMedia)),item=items.find(value=>value.id==='met:67.55.170/whole');
  const target=wall.regionTarget(item,[100,200,300,400]);
  expect(target).toMatchObject({type:'region',id:'media:zarathustra:met:67.55.170/whole#xywh=pixel:100,200,300,400'});
  const quote=wall.quoteOf(target);
  expect(quote).toContain('The Wheel of Fortune (1900)');
  expect(quote).toContain('Region: 100, 200, 300, 400 px');
  expect(quote).toContain('Public domain: https://www.metmuseum.org/art/collection/search/11391');
  expect(checkTicket({target,draft:{observation:'the wheel'}})[0].level).toBe('ok');
});

test('each year on the scale is as wide as its images need, so they stand in a few rows; the years still run in order',()=>{
  const items=wallItems(readMedia(zarathustraMedia)),scale=densityScale(items);
  const places=items.filter(item=>item.years).map(item=>({...scale.place(item),id:item.id}));
  expect(Math.max(...places.map(place=>place.lane))).toBeLessThan(3);
  // No two images of a row overlap.
  for(const lane of [0,1,2]){const xs=places.filter(place=>place.lane===lane).map(place=>place.x).sort((a,b)=>a-b);for(let i=1;i<xs.length;i++)expect(xs[i]-xs[i-1]).toBeGreaterThanOrEqual(100);}
  for(let year=1480;year<1914;year++)expect(scale.x(year+1)).toBeGreaterThanOrEqual(scale.x(year));
  expect(scale.x(1890)-scale.x(1880)).toBeGreaterThan(scale.x(1610)-scale.x(1600));
  expect(scale.ticks.filter(tick=>tick.major).map(tick=>tick.year)).toEqual([1500,1550,1600,1650,1700,1750,1800,1850,1900]);
});
