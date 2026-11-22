import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {createMediaView} from './media-view.mjs';
import {readMedia} from '../../../../bonsai/lib/media.mjs';
import zarathustraMedia from '../../../../bonsai/zarathustra/media.json' with {type:'json'};

function setup(){
  const window=new Window({url:'http://localhost/zarathustra/'});
  vi.stubGlobal('document',window.document);
  vi.stubGlobal('HTMLStyleElement',window.HTMLStyleElement);
  const card=window.document.createElement('div');window.document.body.append(card);
  const view=createMediaView({t:key=>key,locale:()=> 'en'});
  const subject=zarathustraMedia.uses[0].subject,items=readMedia(zarathustraMedia).forSubject(subject);
  return {window,card,view,items};
}

test('closing the viewer gives focus back to the image that opened it',()=>{
  const {window,card,view,items}=setup();
  view.render(card,items.slice(0,1));
  const open=card.querySelector('.ps-media-open');
  open.click();
  expect(window.document.querySelector('dialog.ps-media-viewer')).not.toBeNull();
  view.close();
  expect(window.document.activeElement).toBe(open);
  vi.unstubAllGlobals();
});

test('when the opening picture fails while the viewer is open, focus returns to its place on the card',()=>{
  const {window,card,view,items}=setup();
  view.render(card,items.slice(0,1));
  const open=card.querySelector('.ps-media-open'),figure=open.parentElement;
  open.click();
  open.querySelector('img').dispatchEvent(new window.Event('error'));
  expect(open.isConnected).toBe(false);
  expect(figure.querySelector('.ps-media-missing')).not.toBeNull();
  view.close();
  expect(window.document.activeElement).toBe(figure);
  expect(window.document.querySelector('dialog.ps-media-viewer')).toBeNull();
  vi.unstubAllGlobals();
});

test('a small copy in a link strip that fails while open returns focus to its row',()=>{
  const {window,card,view,items}=setup();
  view.renderStrip(card,[{label:null,items:items.slice(0,1)}]);
  const open=card.querySelector('.ps-media-thumb'),row=open.parentElement;
  open.click();
  open.querySelector('img').dispatchEvent(new window.Event('error'));
  view.close();
  expect(window.document.activeElement).toBe(row);
  vi.unstubAllGlobals();
});
