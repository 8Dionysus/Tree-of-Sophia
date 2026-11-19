import '../observatory/human-forms-wasm-test-runtime.mjs';
import {it,expect,vi} from 'vitest';
import {Window} from 'happy-dom';
import corpusManifest from '../../public/public-corpus/manifest.json' with {type:'json'};
import aestheticsIdentities from '../../../../bonsai/aesthetics/identities.json' with {type:'json'};
const providerFixture=vi.hoisted(()=>({manifest:null}));
vi.mock('../../constructor/sky.mjs',()=>({mountConstructorSky:()=>new Proxy({}, {get:()=>()=>{}})}));
vi.mock('./bootstrap.mjs',()=>({initializePublicReaderRules:async()=>{}}));
vi.mock('../corpus-reader/notebook.mjs',()=>({createCorpusNotebook:()=>({getPreference:async()=>undefined,setPreference:async()=>{},close(){}})}));
vi.mock('../corpus-reader/retained-public-provider.mjs',()=>({createRetainedPublicCorpusProvider:()=>({describe:async()=>providerFixture.manifest,catalog:async()=>({items:[]}),document:async()=>null,structure:async()=>({items:[],nextCursor:null})})}));
vi.mock('../corpus-reader/host.mjs',()=>({mountCorpusEntry:({root,notebook})=>{const opener=document.createElement('button');opener.className='test-reader-open';root.append(opener);return {opener,notebook,reader:{setSelectionActions(){}},open:async()=>{},destroy(){}};}}));
vi.mock('./workspace-view.mjs',()=>({createPublicResearchOwners:()=>({workspace:null,shelf:null}),mountPublicWorkspaceView:()=>({connectAgentBridge(){},render(){},destroy(){},hasUnsavedDraft:()=>false})}));
vi.mock('./agent-bridge.mjs',()=>({mountPublicAgentBridge:()=>({destroy(){}})}));
vi.mock('./experience-view.mjs',()=>({mountResearchExperience:()=>({cognitiveRoot:document.createElement('div'),getState:()=>({}),destroy(){}})}));
vi.mock('./cognitive-session.mjs',()=>({mountCognitiveSession:()=>({destroy(){}})}));
vi.mock('./page-agent.mjs',()=>({connectResearchPage:()=>({destroy(){}})}));

it('starts the page, lists the three stands and enables the selected corpus reader',async()=>{
  const window=new Window({url:'http://localhost/aesthetics/'});
  providerFixture.manifest=corpusManifest;
  expect(corpusManifest.profiles.aesthetics).toBeUndefined();
  expect(corpusManifest.profiles.antiquity.documentIds).toHaveLength(51);
  for(const key of ['window','document','location','history','localStorage','navigator','getComputedStyle','CustomEvent','Event','HTMLElement','Element','Node','CSS','MutationObserver','requestAnimationFrame','cancelAnimationFrame'])vi.stubGlobal(key,key==='window'?window:window[key]);
  const errors=[];window.addEventListener('error',event=>errors.push(event.error));
  document.body.innerHTML='<div id="public-research" data-profile="aesthetics"></div>';
  try{
    await import('./app.mjs');
    await vi.waitFor(()=>{expect(document.querySelector('#tree')?.dataset.ready).toBe('true');expect(document.querySelector('.test-reader-open')?.disabled).toBe(false);},{timeout:10000});
    expect(window.tosPublicResearch.profile).toBe('aesthetics');
    expect([...document.querySelectorAll('.pr-spaces > *')].map(node=>node.textContent)).toEqual(['Antiquity','Zarathustra','Aesthetics']);
    expect(document.querySelector('.test-reader-open').disabled).toBe(false);
    const standWorks=new Set(Object.values(aestheticsIdentities.works).map(work=>work.tree));
    const expectedCount=corpusManifest.profiles.antiquity.documentIds.filter(id=>standWorks.has(id)).length;
    expect(document.querySelector('.pr-provider-state').textContent).toContain(`${expectedCount} works are listed`);
    expect(document.querySelector('.pr-error')).toBeNull();expect(errors).toEqual([]);
  }finally{window.dispatchEvent(new window.Event('pagehide'));await Promise.resolve();vi.unstubAllGlobals();window.happyDOM.abort();}
});
