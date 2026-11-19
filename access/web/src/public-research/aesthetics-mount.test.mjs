import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,it,vi} from 'vitest';
import {Window} from 'happy-dom';
const sky=vi.hoisted(()=>({scenes:[]}));
vi.mock('../../constructor/sky.mjs',()=>({mountConstructorSky:()=>new Proxy({}, {get:(_,key)=>key==='update'?(scene=>sky.scenes.push(scene)):(()=>{})})}));
import {createPublicSpace} from './space-library.mjs';
import {mountPublicResearchSpace} from './public-space.mjs';
import {bonsaiFor} from './bonsai.mjs';
import {aestheticsTimeline} from './space-timeline.mjs';

it('mounts Aesthetics and opens its four lenses',async()=>{
  const window=new Window({url:'http://localhost/aesthetics/'});
  for(const key of ['window','document','location','history','localStorage','navigator','getComputedStyle','CustomEvent','Event','HTMLElement','Element','Node','CSS','requestAnimationFrame','cancelAnimationFrame','innerWidth'])vi.stubGlobal(key,key==='window'?window:window[key]);
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
      if(name==='Древо'){const names=sky.scenes.at(-1).clusters.filter(item=>item.id.startsWith('group-'));expect(names.map(item=>item.title)).toEqual(['Группа A','Группа B','Группа C','Группа D']);expect(names.every(item=>!item.yields)).toBe(true);}
      if(name==='Время'){
        const scene=sky.scenes.at(-1),bonsai=bonsaiFor('aesthetics'),time=aestheticsTimeline({field:bonsai.field,library:prepared.library,chronology:bonsai.chronology});
        const shown=new Map(scene.nodes.map(node=>[node.materialId,node.position]));
        for(const [id,position] of time.positions)expect(shown.get(id),id).toEqual(position);
        expect(scene.clusters.filter(item=>item.variant==='lane').map(item=>item.title)).toEqual(['Группа A','Группа B','Группа C','Группа D']);
      }
    }
  }finally{view.destroy();vi.unstubAllGlobals();window.happyDOM.abort();}
});

it('offers the three stands in their current order with the Aesthetics tree',async()=>{
  const window=new Window({url:'http://localhost/home/'});
  for(const key of ['window','document','location','history','localStorage','navigator','getComputedStyle','CustomEvent','Event','HTMLElement','Element','Node','CSS'])vi.stubGlobal(key,key==='window'?window:window[key]);
  vi.stubGlobal('fetch',vi.fn(async()=>({ok:false,status:404})));
  document.body.innerHTML='<div class="home-bar"></div><div class="home-head"></div><div class="home-choice"></div>';
  try{
    await import('../home/home.mjs');
    const links=[...document.querySelectorAll('.home-stand a')];expect(links.map(link=>new URL(link.href).pathname)).toEqual(['/antiquity/','/zarathustra/','/aesthetics/']);
    expect(links[2].querySelector('.home-tree')).not.toBeNull();expect(links[2].textContent).toContain('The philosophy of beauty, art and sensory experience');
  }finally{vi.unstubAllGlobals();window.happyDOM.abort();}
});
