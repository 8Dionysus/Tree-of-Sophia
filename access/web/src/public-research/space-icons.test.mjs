import {describe,expect,it} from 'vitest';
import {ICON_NAMES,hasIcon,iconMarkup} from './space-icons.mjs';

describe('the Tree\'s icons',()=>{
  it('draws every lens and every research tool in one grammar of lines and stars',()=>{
    for(const name of ['tree','meaning','sources','tensions','images','time','space','proximity','routes','lensInquiry','grow','research','new','connect','compare','measureAction'])expect(hasIcon(name),name).toBe(true);
    for(const name of ICON_NAMES){
      const markup=iconMarkup(name);
      expect(markup,name).toMatch(/viewBox="0 0 24 24"/);
      expect(markup,name).toMatch(/stroke="currentColor"/);
      // Every coordinate stays on the grid.
      for(const n of markup.matchAll(/c[xy]="([\d.]+)"/g))expect(Number(n[1])).toBeLessThanOrEqual(24);
    }
  });
  it('gives every lens and research icon a star, and leaves the plain tools of the map and the record without one',()=>{
    for(const name of ['tree','meaning','sources','tensions','images','time','space','proximity','routes','lensInquiry','grow','research','connect','compare','measureAction','fit'])expect(iconMarkup(name),name).toMatch(/ps-icon-star/);
    for(const name of ['zoomIn','zoomOut','graphUndo','graphRedo','export','import'])expect(iconMarkup(name),name).not.toMatch(/ps-icon-star/);
  });
  it('knows no name it was not given',()=>{expect(iconMarkup('nothing')).toBe('');});
});
