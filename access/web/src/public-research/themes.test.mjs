import {readFileSync} from 'node:fs';
import postcss from 'postcss';
import {expect,it} from 'vitest';
import {THEMES,THEME_KEY,parseColor,themeColor,themePostcss,themeSelector,themeValue} from './themes.mjs';
import {STANDS} from './stands.mjs';
import {growBonsai} from '../home/home-bonsai.mjs';

const luminance=color=>{const [r,g,b]=parseColor(color).map(channel=>channel<=0.04045?channel/12.92:((channel+0.055)/1.055)**2.4);return 0.2126*r+0.7152*g+0.0722*b;};
const contrast=(a,b)=>{const [x,y]=[luminance(a),luminance(b)].sort((p,q)=>q-p);return (x+0.05)/(y+0.05);};
const light=THEMES.filter(theme=>!theme.dark);

it('takes the night’s ground, ink and gold to each light theme’s page, ink and accent',()=>{
  expect(THEMES.map(theme=>theme.id)).toEqual(['midnight','noon','rubric','dawn']);
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
  expect(themeSelector('html','dawn')).toBe('html[data-site-theme="dawn"]');
  expect(themeSelector(' body','dawn')).toBe(':root[data-site-theme="dawn"] body');
  const css=':root{--gold:#dcc690;--space:4px}@media (max-width:600px){#tree .x{color:#e4e6ea;border:0;margin:0}}.y{margin:0}.z{color:inherit}@keyframes k{from{color:#fff}}';
  const result=(await postcss([themePostcss()]).process(css,{from:undefined})).root;
  const twins=[];result.walkRules(rule=>{if(rule.selector.includes('data-site-theme'))twins.push(rule);});
  expect(twins.map(rule=>rule.selector)).toEqual([
    ':root[data-site-theme="noon"]',':root[data-site-theme="rubric"]',':root[data-site-theme="dawn"]',
    ':root[data-site-theme="noon"] #tree .x',':root[data-site-theme="rubric"] #tree .x',':root[data-site-theme="dawn"] #tree .x',
  ]);
  expect(twins[0].toString()).toBe(':root[data-site-theme="noon"]{--gold:#83601f;--space:4px}');
  expect(twins[3].parent.type).toBe('atrule');expect(twins[3].toString()).toBe(':root[data-site-theme="noon"] #tree .x{color:#1f2633;border:0}');
});

it('sets the saved theme before the first paint, from the same names',()=>{
  const boot=readFileSync(new URL('../../public/theme.js',import.meta.url),'utf8');
  expect(boot).toContain(THEME_KEY);
  expect(/\^\(([^)]*)\)\$/.exec(boot)[1].split('|').sort()).toEqual(THEMES.map(theme=>theme.id).sort());
  for(const page of ['zarathustra','antiquity','home','not-found'])expect(readFileSync(new URL(`../../${page}/index.html`,import.meta.url),'utf8')).toContain('<script src="/theme.js"></script>');
});

it('grows each bonsai the same way from its seed, with its own star at the crown',()=>{
  const first=growBonsai({seed:1883}),again=growBonsai({seed:1883}),other=growBonsai({seed:347});
  expect(again).toEqual(first);expect(other).not.toEqual(first);
  expect(first.stars.filter(star=>star.crown)).toHaveLength(1);
  expect(Math.min(...first.stars.map(star=>star.y))).toBe(first.stars.find(star=>star.crown).y);
  // Every stand of the site has its page.
  for(const stand of STANDS)expect(readFileSync(new URL(`../../${stand.id}/index.html`,import.meta.url),'utf8')).toContain(`data-profile="${stand.id}"`);
});
