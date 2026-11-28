// @vitest-environment happy-dom
import {describe,expect,it,vi} from 'vitest';
vi.mock('../../../../bonsai/aesthetics/fragments.json',()=>({default:{
  works:{work:{original:'de',author:{en:'Author'},title:{en:'Work'},witnesses:{
    de:{rights:{basis:'public-domain'},url:'https://example.org/de'},
    'en-alt':{rights:{basis:'short-quotation'},url:'https://example.org/en',attribution:{en:'A translator'}}
  }}},
  places:{work:{p:{label:{en:'Page'},texts:{de:'Ein Satz.',en:'Something uncanny in real experience is conditioned thus.'},witness:{en:'en-alt'},links:{en:'https://example.org/page'},emphasis:{en:['real experience']}}}}
}}));
import {createFragments} from './fragments.mjs';

describe('Aesthetics printed emphasis',()=>{
  it('shows and copies the reviewed words with their emphasis and actual witness',async()=>{
    const writeText=vi.fn().mockResolvedValue();Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText}});
    const figure=createFragments({profile:'aesthetics',locale:()=> 'en',t:key=>key}).fragment({w:'work',loc:'p'});
    document.body.append(figure);
    for(let i=0;i<100&&figure.hasAttribute('aria-busy');i++)await new Promise(resolve=>setTimeout(resolve,5));
    expect(figure.querySelector('blockquote[lang="en"] em')?.textContent).toBe('real experience');
    expect(figure.querySelectorAll('.ps-fragment-source')[1].textContent).toContain('A translator');
    figure.querySelector('.ps-fragment-copy').click();
    expect(writeText.mock.calls[0][0]).toContain('*real experience*');
    expect(writeText.mock.calls[0][0]).toContain('A translator');figure.remove();
  });
});
