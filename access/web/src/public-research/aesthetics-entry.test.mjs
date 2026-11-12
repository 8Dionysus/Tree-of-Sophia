import '../observatory/human-forms-wasm-test-runtime.mjs';
import {it,expect,vi} from 'vitest';
import {Window} from 'happy-dom';
vi.mock('../../constructor/sky.mjs',()=>({mountConstructorSky:()=>new Proxy({}, {get:()=>()=>{}})}));
vi.mock('./bootstrap.mjs',()=>({initializePublicReaderRules:async()=>{}}));
vi.mock('../corpus-reader/notebook.mjs',()=>({createCorpusNotebook:()=>({getPreference:async()=>undefined,setPreference:async()=>{},close(){}})}));
vi.mock('../corpus-reader/retained-public-provider.mjs',()=>({createRetainedPublicCorpusProvider:()=>({describe:async()=>({revision:'fixture',profiles:{antiquity:{documentIds:['work']}},documents:[]}),catalog:async()=>({items:[]}),document:async()=>null,structure:async()=>({items:[],nextCursor:null})})}));
vi.mock('../corpus-reader/host.mjs',()=>({mountCorpusEntry:({root,notebook})=>{const opener=document.createElement('button');opener.className='test-reader-open';root.append(opener);return {opener,notebook,reader:{setSelectionActions(){}},open:async()=>{},destroy(){}};}}));
vi.mock('./workspace-view.mjs',()=>({createPublicResearchOwners:()=>({workspace:null,shelf:null}),mountPublicWorkspaceView:()=>({connectAgentBridge(){},render(){},destroy(){},hasUnsavedDraft:()=>false})}));
vi.mock('./agent-bridge.mjs',()=>({mountPublicAgentBridge:()=>({destroy(){}})}));
vi.mock('./experience-view.mjs',()=>({mountResearchExperience:()=>({cognitiveRoot:document.createElement('div'),getState:()=>({}),destroy(){}})}));
vi.mock('./cognitive-session.mjs',()=>({mountCognitiveSession:()=>({destroy(){}})}));
vi.mock('./page-agent.mjs',()=>({connectResearchPage:()=>({destroy(){}})}));

it('starts the page, lists the three stands and enables the selected corpus reader',async()=>{
  const window=new Window({url:'http://localhost/aesthetics/'});
  for(const key of ['window','document','location','history','localStorage','navigator','getComputedStyle','CustomEvent','Event','HTMLElement','Element','Node','CSS','MutationObserver','requestAnimationFrame','cancelAnimationFrame'])vi.stubGlobal(key,key==='window'?window:window[key]);
  const errors=[];window.addEventListener('error',event=>errors.push(event.error));
  document.body.innerHTML='<div id="public-research" data-profile="aesthetics"></div>';
  try{
    await import('./app.mjs');
    await vi.waitFor(()=>{expect(document.querySelector('#tree')?.dataset.ready).toBe('true');expect(document.querySelector('.test-reader-open')?.disabled).toBe(false);},{timeout:10000});
    expect(window.tosPublicResearch.profile).toBe('aesthetics');
    expect([...document.querySelectorAll('.pr-spaces > *')].map(node=>node.textContent)).toEqual(['Zarathustra','Antiquity','Aesthetics']);
    expect(document.querySelector('.test-reader-open').disabled).toBe(false);
    expect(document.querySelector('.pr-error')).toBeNull();expect(errors).toEqual([]);
  }finally{window.dispatchEvent(new window.Event('pagehide'));await Promise.resolve();vi.unstubAllGlobals();window.happyDOM.abort();}
});
