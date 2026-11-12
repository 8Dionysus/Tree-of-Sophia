import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,it,vi} from 'vitest';
import {Window} from 'happy-dom';
const sky=vi.hoisted(()=>({scenes:[]}));
vi.mock('../../constructor/sky.mjs',()=>({mountConstructorSky:()=>new Proxy({}, {get:(_,key)=>key==='update'?(scene=>sky.scenes.push(scene)):(()=>{})})}));
import {createPublicSpace} from './space-library.mjs';
import {mountPublicResearchSpace} from './public-space.mjs';

it('mounts Aesthetics and opens its four lenses',async()=>{
  const window=new Window({url:'http://localhost/aesthetics/'});
  for(const key of ['window','document','location','history','localStorage','navigator','getComputedStyle','CustomEvent','Event','HTMLElement','Element','Node','CSS','requestAnimationFrame','cancelAnimationFrame'])vi.stubGlobal(key,key==='window'?window:window[key]);
  const errors=[];window.addEventListener('error',event=>errors.push(event.error));
  const provider={describe:async()=>({revision:'fixture',profiles:{antiquity:{documentIds:[]}},documents:[]}),catalog:async()=>({items:[]}),document:async()=>null,structure:async()=>({items:[],nextCursor:null})};
  const prepared=await createPublicSpace({profile:'aesthetics',provider});
  const host=document.createElement('main');document.body.append(host);
  const view=mountPublicResearchSpace({host,profile:'aesthetics',locale:()=> 'ru',...prepared,api:{provider,reader:{setSelectionActions(){}}},saveState:()=>{}});
  try{
    expect(view.root.dataset.ready).toBe('true');
    const rail=view.root.querySelector('.ps-lens-rail');
    for(const name of ['Древо','Источники','Время','Близость']){
      const button=[...rail.querySelectorAll('button')].find(node=>node.textContent.includes(name));expect(button,name).toBeTruthy();button.click();expect([...rail.querySelectorAll('button')].find(node=>node.textContent.includes(name))?.getAttribute('aria-pressed'),name).toBe('true');expect(sky.scenes.at(-1)?.nodes.length,name).toBeGreaterThan(0);expect(errors,name).toEqual([]);
    }
  }finally{view.destroy();vi.unstubAllGlobals();window.happyDOM.abort();}
});

it('offers all three stands on the home page without a placeholder for Aesthetics',async()=>{
  const window=new Window({url:'http://localhost/home/'});
  for(const key of ['window','document','location','history','localStorage','navigator','getComputedStyle','CustomEvent','Event','HTMLElement','Element','Node','CSS'])vi.stubGlobal(key,key==='window'?window:window[key]);
  vi.stubGlobal('fetch',vi.fn(async()=>({ok:false,status:404})));
  document.body.innerHTML='<div class="home-bar"></div><div class="home-head"></div><div class="home-choice"></div>';
  try{
    await import('../home/home.mjs');
    const links=[...document.querySelectorAll('.home-stand a')];expect(links.map(link=>new URL(link.href).pathname)).toEqual(['/zarathustra/','/antiquity/','/aesthetics/']);
    expect(links[2].querySelector('.home-tree')).toBeNull();expect(links[2].textContent).toContain('The philosophy of beauty, art and sensory experience');
  }finally{vi.unstubAllGlobals();window.happyDOM.abort();}
});
