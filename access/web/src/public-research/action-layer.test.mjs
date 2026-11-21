import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {mountActionLayer} from './action-layer.mjs';

function setup(search='?treeLayer=research'){
  const window=new Window({url:`http://localhost/zarathustra/${search}`});
  const document=window.document;
  document.body.innerHTML='<header id="research-bar"><nav class="main-tools"><button class="corpus-open">Library</button></nav><div class="rb-menu"></div></header><div id="tree"></div>';
  const header=document.querySelector('#research-bar'),shell=document.querySelector('#tree'),columnTools=document.createElement('div');shell.append(columnTools);
  const items=new Map([['recurrence',{id:'recurrence',kind:'conception',title:{ru:'Вечное возвращение',en:'Eternal recurrence'}}]]);
  let chosen={id:'recurrence',kind:'node'};
  const space={index:{revision:'r1',get:id=>items.get(id)},selection:()=>chosen,compare:vi.fn(),
    actions:{lenses:()=>[{id:'tree',title:{ru:'Древо',en:'Tree'}}],lens:()=>'tree',setLens:vi.fn(()=>true),routeState:()=>({id:'gate',source:'atlas',step:2}),
      toggleRoutes:vi.fn(),focusSearch:vi.fn(),layOnBench:vi.fn(),canLayOnBench:()=>true,columnTools,setPassageActions:vi.fn()},bench:()=>({isOpen:()=>false})};
  const registry={invoke:vi.fn(async()=>({id:'recurrence'})),notifyStateChange:vi.fn()};
  const experience={getState:()=>({mode:'original'}),open:vi.fn(),setMode:vi.fn()};
  const layer=mountActionLayer({shell,header,space,experience,registry,api:{reader:null},locale:()=> 'en'});
  return {window,document,shell,header,space,registry,layer,choose:value=>{chosen=value;}};
}
const labels=document=>[...document.querySelectorAll('.al-palette-item .al-palette-label')].map(node=>node.textContent);

test('the presence layer comes from the address, lives on the page and walks with L',()=>{
  const {window,document,shell,layer,registry}=setup();
  expect(layer.layer()).toBe('research');
  expect(shell.dataset.presenceLayer).toBe('research');
  const heard=vi.fn();shell.addEventListener('public-research-layer',event=>heard(event.detail.layer));
  document.dispatchEvent(new window.KeyboardEvent('keydown',{code:'KeyL',key:'l',bubbles:true}));
  expect(layer.layer()).toBe('agents');
  expect(new URL(window.location.href).searchParams.get('treeLayer')).toBe('agents');
  expect(heard).toHaveBeenCalledWith('agents');
  expect(registry.notifyStateChange).toHaveBeenCalled();
  layer.setLayer('reading');
  expect(new URL(window.location.href).searchParams.has('treeLayer')).toBe(false);
  expect(document.querySelectorAll('.rb-menu .al-layer')).toHaveLength(3);
});

test('one menu lists what applies to the chosen star and runs it through the page command',async()=>{
  const {document,layer,registry,choose}=setup();
  layer.openPalette();
  expect(labels(document)).toEqual(expect.arrayContaining(['Open the card','Copy link','Compare','Lens: Tree']));
  const input=document.querySelector('.al-palette-input');
  input.value='open the card';input.dispatchEvent(new document.defaultView.Event('input'));
  input.dispatchEvent(new document.defaultView.KeyboardEvent('keydown',{key:'Enter'}));
  await vi.waitFor(()=>expect(registry.invoke).toHaveBeenCalledWith('tos.page.select',{item_id:'recurrence'}));
  choose(null);layer.openPalette();
  expect(labels(document)).not.toContain('Open the card');
  expect(labels(document)).toContain('Copy a link to this view');
  // The card's tools hold the menu button.
  expect(document.querySelector('.al-card-menu')).not.toBeNull();
});

test('a link names the subject, the experience mode and the layer; the view state names the route step',()=>{
  const {layer}=setup('?treeLens=meaning');
  const link=new URL(layer.linkTo({type:'star',id:'recurrence'}));
  expect(link.searchParams.get('item')).toBe('recurrence');
  expect(link.searchParams.get('revision')).toBe('r1');
  expect(link.searchParams.get('experience')).toBe('original');
  expect(link.searchParams.get('treeLens')).toBe('meaning');
  expect(link.searchParams.get('treeLayer')).toBeNull();
  expect(layer.context()).toMatchObject({presence_layer:'reading',lens:'tree',route:{id:'gate',step:2},bench_open:false,reader_selection:null});
});

test('the table of what agents can do joins interface actions, page commands, act commands and native tools',()=>{
  const {document,layer}=setup();
  layer.openPalette();
  const input=document.querySelector('.al-palette-input');
  input.value='what agents';input.dispatchEvent(new document.defaultView.Event('input'));
  input.dispatchEvent(new document.defaultView.KeyboardEvent('keydown',{key:'Enter'}));
  const cells=[...document.querySelectorAll('.al-capabilities-table td')].map(cell=>cell.textContent);
  expect(cells).toEqual(expect.arrayContaining(['Open the card','tos.page.select','tos.act.append','tos_knowledge_catalog','tos knowledge catalog']));
});
