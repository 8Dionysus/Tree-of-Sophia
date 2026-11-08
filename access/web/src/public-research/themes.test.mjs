import {readFileSync} from 'node:fs';
import postcss from 'postcss';
import {expect,it} from 'vitest';
import {THEMES,THEME_KEY,parseColor,themeColor,themePostcss,themeSelector,themeValue} from './themes.mjs';
import {STANDS} from './stands.mjs';

const luminance=color=>{const [r,g,b]=parseColor(color).map(channel=>channel<=0.04045?channel/12.92:((channel+0.055)/1.055)**2.4);return 0.2126*r+0.7152*g+0.0722*b;};
const contrast=(a,b)=>{const [x,y]=[luminance(a),luminance(b)].sort((p,q)=>q-p);return (x+0.05)/(y+0.05);};
const light=THEMES.filter(theme=>!theme.dark);

it('takes the night’s ground, ink and gold to each light theme’s page, ink and accent',()=>{
  expect(THEMES.map(theme=>theme.id)).toEqual(['midnight','noon']);
  for(const theme of light){
    expect(themeColor(theme.id,'#070F1B').toUpperCase()).toBe(theme.palette.ground.toUpperCase());
    expect(themeColor(theme.id,'#E4E6EA').toUpperCase()).toBe(theme.palette.ink.toUpperCase());
    expect(themeColor(theme.id,'#DCC690').toUpperCase()).toBe(theme.palette.accent.toUpperCase());
  }
  expect(themeColor('midnight','#dcc690')).toBe('#dcc690');
});

it('keeps the night’s words readable on every light page',()=>{
  // The night's text, its quiet text and its gold, on the page and on a panel of each light theme.
  for(const theme of light)for(const surface of ['#070F1B','#0b1626'])for(const word of ['#E4E6EA','#B9C6D5','#93A5B9','#DCC690','#c3cfdb']){
    expect(contrast(themeColor(theme.id,word),themeColor(theme.id,surface)),`${theme.id} ${word} on ${surface}`).toBeGreaterThanOrEqual(4.5);
  }
});

it('keeps a shadow dark and a veil a veil, and the night’s other hues their own hue',()=>{
  expect(themeValue('noon','0 16px 44px #000c','box-shadow')).toMatch(/^0 16px 44px #1f2633[0-9a-f]{2}$/);
  expect(themeValue('noon','#0008','background')).toMatch(/^#1f2633[0-9a-f]{2}$/);
  expect(themeValue('noon','dark','color-scheme')).toBe('light');
  expect(themeValue('noon','url(#glow) #fff','fill')).toBe('url(#glow) #1d2431');
  const [r,,b]=parseColor(themeColor('noon','#ba9be5'));expect(b).toBeGreaterThan(r);
});

it('adds one twin per light theme to each rule that paints, scoped to the theme on the root',async()=>{
  expect(themeSelector(':root:lang(zh-Hans) #bar','noon')).toBe(':root[data-site-theme="noon"]:lang(zh-Hans) #bar');
  expect(themeSelector('html','noon')).toBe('html[data-site-theme="noon"]');
  expect(themeSelector(' body','noon')).toBe(':root[data-site-theme="noon"] body');
  const css=':root{--gold:#dcc690;--space:4px}@media (max-width:600px){#tree .x{color:#e4e6ea;border:0;margin:0}}.y{margin:0}.z{color:inherit}@keyframes k{from{color:#fff}}';
  const result=(await postcss([themePostcss()]).process(css,{from:undefined})).root;
  const twins=[];result.walkRules(rule=>{if(rule.selector.includes('data-site-theme'))twins.push(rule);});
  expect(twins.map(rule=>rule.selector)).toEqual([
    ':root[data-site-theme="noon"]',':root[data-site-theme="noon"] #tree .x',
    // `inherit` keeps its colour, and is twinned all the same: its rule still wins over the colour before it.
    ':root[data-site-theme="noon"] .z',
  ]);
  expect(twins[0].toString()).toBe(':root[data-site-theme="noon"]{--gold:#83601f}');
  // A rule written for one theme already stays that theme's own.
  const own=(await postcss([themePostcss()]).process(':root[data-site-theme="noon"] .w{background:#f4efe4e0}',{from:undefined})).root;
  expect(own.nodes).toHaveLength(1);
  expect(twins[1].parent.type).toBe('atrule');expect(twins[1].toString()).toBe(':root[data-site-theme="noon"] #tree .x{color:#1f2633}');
});

it('keeps the night’s cascade for every family it turns, and leaves the others to the night',async()=>{
  // A width held in a custom property is the night's own, narrowed by a later rule: no twin may restore it.
  const css='#t{--panel:calc(20px + 8px);--ink:#e4e6ea}#t[data-open=false]{--panel:12px}.a{background:#070f1b}.a.b{background:none}';
  const result=(await postcss([themePostcss()]).process(css,{from:undefined})).root;
  const twins=[];result.walkRules(rule=>{if(rule.selector.includes('data-site-theme'))twins.push(rule.toString());});
  expect(twins).toEqual([':root[data-site-theme="noon"] #t{--ink:#1f2633}',':root[data-site-theme="noon"] .a{background:#f4efe4}',':root[data-site-theme="noon"] .a.b{background:none}']);
});

it('follows the system’s light or dark until the reader chooses a theme',()=>{
  const boot=readFileSync(new URL('../../public/theme.js',import.meta.url),'utf8');
  const run=(saved,dark)=>{const attributes={},root={setAttribute:(key,value)=>{attributes[key]=value;},style:{}};
    new Function('document','localStorage','matchMedia',boot)({documentElement:root},{getItem:()=>saved},query=>({matches:/dark/.test(query)?dark:!dark}));
    return [attributes['data-site-theme'],root.style.colorScheme];};
  expect(run(null,true)).toEqual(['midnight','dark']);expect(run(null,false)).toEqual(['noon','light']);
  expect(run('noon',true)).toEqual(['noon','light']);expect(run('midnight',false)).toEqual(['midnight','dark']);
  // A theme the site no longer has gives way to the system's.
  expect(run('dawn',false)).toEqual(['noon','light']);
});

it('sets the saved theme before the first paint, from the same names',()=>{
  const boot=readFileSync(new URL('../../public/theme.js',import.meta.url),'utf8');
  expect(boot).toContain(THEME_KEY);
  expect(/\^\(([^)]*)\)\$/.exec(boot)[1].split('|').sort()).toEqual(THEMES.map(theme=>theme.id).sort());
  for(const page of ['zarathustra','antiquity','home','not-found'])expect(readFileSync(new URL(`../../${page}/index.html`,import.meta.url),'utf8')).toContain('<script src="/theme.js"></script>');
});

it('gives each bonsai its own tree with one crown star, the tree its card and the sky both show',()=>{
  const sky=readFileSync(new URL('../../public/cosmos.css',import.meta.url),'utf8');
  const named=/var TREES=\[([^\]]*)\]/.exec(readFileSync(new URL('../../public/cosmos.js',import.meta.url),'utf8'))[1];
  expect(named.split(',').map(id=>id.trim().slice(1,-1)).sort()).toEqual(STANDS.map(stand=>stand.id).sort());
  for(const stand of STANDS){
    const tree=readFileSync(new URL(`../../public/trees/${stand.id}.svg`,import.meta.url),'utf8');
    expect(tree).toContain('class="tree-wood"');
    expect(tree.match(/tree-crown/g)).toHaveLength(1);
    expect(sky).toContain(`[data-tree=${stand.id}] .cosmos-mark{background-image:url(trees/${stand.id}.svg)}`);
    // Every stand of the site has its page.
    expect(readFileSync(new URL(`../../${stand.id}/index.html`,import.meta.url),'utf8')).toContain(`data-profile="${stand.id}"`);
  }
});
