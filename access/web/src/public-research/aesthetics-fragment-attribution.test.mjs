// @vitest-environment happy-dom
import {describe,expect,it,vi} from 'vitest';
import {createFragments} from './fragments.mjs';

describe('Aesthetics witness attribution',()=>{
  it('copies the actual translation attribution alongside its quotation',async()=>{
    const writeText=vi.fn().mockResolvedValue();
    Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText}});
    const fragments=createFragments({profile:'aesthetics',locale:()=> 'en',t:key=>key});
    const figure=fragments.fragment({w:'friedrich-schiller.ueber-das-erhabene',loc:'Essay p286a'});
    document.body.append(figure);
    for(let i=0;i<100&&figure.hasAttribute('aria-busy');i++)await new Promise(resolve=>setTimeout(resolve,5));
    const captions=[...figure.querySelectorAll('.ps-fragment-source')];
    expect(captions).toHaveLength(2);
    expect(captions[1].textContent).toContain('translated by John Weiss');
    expect(captions[1].textContent).toContain('p. 246');
    expect(captions[1].textContent).not.toContain('p. 286');
    expect([...figure.querySelectorAll('.ps-fragment-cite .ps-fragment-whole')].some(link=>link.textContent.includes('fragmentQuoted'))).toBe(true);
    figure.querySelector('.ps-fragment-copy').click();
    expect(writeText).toHaveBeenCalledOnce();
    expect(writeText.mock.calls[0][0]).toContain('translated by John Weiss');
    expect(writeText.mock.calls[0][0]).toContain('p. 246');
    figure.remove();
  });
});
