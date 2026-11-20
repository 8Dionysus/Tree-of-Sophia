import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {heldAt,heldOf,mountHeld} from './held-view.mjs';
import held from '../../../../bonsai/zarathustra/held.json' with {type:'json'};

test('the held record keeps every node, link, route and source with the reason review gave',()=>{
  const record=heldOf(held);
  expect(record.nodes).toHaveLength(held.nodes.length);
  expect(record.edges).toHaveLength(held.edges.length);
  expect(record.routes.map(route=>route.id)).toEqual(held.routes.map(route=>route.id));
  expect(record.nodes.every(node=>node.reason)).toBe(true);
  const at=heldAt(record,'material:demo:dossier');
  expect(at.edges.length).toBeGreaterThan(0);
  expect(at.edges.every(edge=>edge.from==='dossier'||edge.to==='dossier')).toBe(true);
});

function setup(layer){
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const added=[],actionLayer={addActions:list=>{added.push(...list);return ()=>{};},layer:()=>layer,say:vi.fn()};
  const view=mountHeld({shell:document.querySelector('#tree'),actionLayer,locale:()=> 'en',load:async()=>held});
  return {document,added,actionLayer,view};
}

test('what review withheld shows only in the research layer, on request, each with its reason and its apparatus behind a door',async()=>{
  const reading=setup('reading');
  expect(reading.added[0].when()).toBe(false);
  expect(await reading.view.open()).toBeNull();
  expect(reading.actionLayer.say).toHaveBeenCalledWith('What review withheld shows only in the research layer');
  const research=setup('research');
  expect(research.added[0].when()).toBe(true);
  await research.view.open();
  const groups=[...research.document.querySelectorAll('.hv-section')].map(section=>section.dataset.group);
  expect(groups).toEqual(['nodes','edges','routes','sources']);
  const first=research.document.querySelector('.hv-item');
  expect(first.querySelector('.hv-reason').textContent).toMatch(/^Reason: \S/);
  expect(first.querySelector('details.tech-section')).not.toBeNull();
  await research.view.open({type:'star',id:'material:demo:dossier',label:'Dossier'});
  const sheets=research.document.querySelectorAll('.hv-sheet');
  expect(sheets[sheets.length-1].querySelector('.hv-section').dataset.group).toBe('edges');
});
