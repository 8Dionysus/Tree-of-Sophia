// The site's themes. Midnight is the stands' own night sky; Noon is its day, the night turned over.
// A light theme takes every colour of the night into its own palette by one rule: grounds become its
// page and leaves, light words become its ink, the night's gold becomes its accent, and every other
// hue keeps itself at the lightness the page needs. The stylesheets are written once, for the night;
// the build adds each light theme's twin of every rule that paints (themePostcss), so a rule written
// later wears every theme by itself.
//   Noon: warm paper, ink and the gold of the great noon.
// Until the reader chooses, the page follows the system's own light or dark.

export const THEME_ATTRIBUTE='data-site-theme';
export const THEME_KEY='tos.public-research.theme.v1';
export const DEFAULT_THEME='midnight';
export const SYSTEM_THEME='system';
export const SYSTEM_NAME=Object.freeze({ru:'Как в системе',en:'System'});
/** The theme of the system's light or dark. */
export const systemTheme=dark=>dark?'midnight':'noon';
export const THEMES=Object.freeze([
  Object.freeze({id:'midnight',dark:true,name:Object.freeze({ru:'Полночь',en:'Midnight'}),palette:Object.freeze({ground:'#070F1B',raised:'#0B1626',ink:'#E4E6EA',accent:'#DCC690'})}),
  Object.freeze({id:'noon',name:Object.freeze({ru:'Полдень',en:'Noon'}),palette:Object.freeze({ground:'#F4EFE4',raised:'#FBF8F1',ink:'#1F2633',accent:'#83601F'})}),
]);
export const themeById=id=>THEMES.find(theme=>theme.id===id)??null;

const clamp=(value,low=0,high=1)=>Math.min(high,Math.max(low,value));

/** A CSS colour as [r,g,b,a] in 0..1: #rgb, #rgba, #rrggbb, #rrggbbaa, rgb()/rgba(), white and black. */
export function parseColor(text){
  const value=String(text).trim().toLowerCase();
  let match=/^#([0-9a-f]{3,8})$/.exec(value);
  if(match){
    const digits=match[1];if(![3,4,6,8].includes(digits.length))return null;
    const full=digits.length<5?[...digits].map(digit=>digit+digit).join(''):digits;
    return [0,2,4,6].map(at=>full.length>at?parseInt(full.slice(at,at+2),16)/255:1);
  }
  match=/^rgba?\(([^)]*)\)$/.exec(value);
  if(match){
    const parts=match[1].split(/[\s,/]+/).filter(Boolean);if(parts.length<3)return null;
    const channel=part=>part.endsWith('%')?parseFloat(part)/100:parseFloat(part)/255;
    const rgba=[...parts.slice(0,3).map(channel),parts[3]===undefined?1:parts[3].endsWith('%')?parseFloat(parts[3])/100:parseFloat(parts[3])];
    return rgba.some(Number.isNaN)?null:rgba;
  }
  if(value==='white')return [1,1,1,1];
  if(value==='black')return [0,0,0,1];
  return null;
}
const pair=value=>Math.round(clamp(value)*255).toString(16).padStart(2,'0');
export const formatColor=([r,g,b,a=1])=>`#${pair(r)}${pair(g)}${pair(b)}${a<0.998?pair(a):''}`;

// OKLab and OKLCh (Björn Ottosson): lightness that the eye reads as even, so one rule serves every hue.
const linear=channel=>channel<=0.04045?channel/12.92:((channel+0.055)/1.055)**2.4;
const gamma=channel=>channel<=0.0031308?12.92*channel:1.055*channel**(1/2.4)-0.055;
function toLab([r,g,b]){
  const [R,G,B]=[r,g,b].map(linear);
  const l=Math.cbrt(0.4122214708*R+0.5363325363*G+0.0514459929*B),m=Math.cbrt(0.2119034982*R+0.6806995451*G+0.1073969566*B),s=Math.cbrt(0.0883024619*R+0.2817188376*G+0.6299787005*B);
  return [0.2104542553*l+0.7936177850*m-0.0040720468*s,1.9779984951*l-2.4285922050*m+0.4505937099*s,0.0259040371*l+0.7827717662*m-0.8086757660*s];
}
const lchOf=([L,A,B])=>[L,Math.hypot(A,B),(Math.atan2(B,A)*180/Math.PI+360)%360];
const toLch=rgb=>lchOf(toLab(rgb));
function fromLch([L,C,h]){
  const angle=h*Math.PI/180;
  // The chroma gives way first, so a colour outside the screen's gamut keeps its lightness and hue.
  for(let chroma=Math.max(0,C);;chroma=Math.max(0,chroma-0.004)){
    const A=chroma*Math.cos(angle),B=chroma*Math.sin(angle);
    const l=(L+0.3963377774*A+0.2158037573*B)**3,m=(L-0.1055613458*A-0.0638541728*B)**3,s=(L-0.0894841775*A-1.2914855480*B)**3;
    const rgb=[4.0767416621*l-3.3077115913*m+0.2309699292*s,-1.2684380046*l+2.6097574011*m-0.3413193965*s,-0.0041960863*l-0.7034186147*m+1.7076147010*s];
    if(chroma===0||rgb.every(channel=>channel>=-0.0005&&channel<=1.0005))return rgb.map(channel=>gamma(clamp(channel)));
  }
}

const NIGHT={ground:toLch(parseColor('#070F1B')),ink:toLch(parseColor('#E4E6EA')),gold:toLch(parseColor('#DCC690'))};
// The night's surfaces rise from its ground by this much lightness: panels, cards and wells of the night.
const RISE=0.10;
const prepared=new Map();
function paletteOf(id){
  if(!prepared.has(id)){const theme=themeById(id);prepared.set(id,theme&&!theme.dark?Object.fromEntries(Object.entries(theme.palette).map(([key,value])=>{const lab=toLab(parseColor(value));return [key,{lab,lch:lchOf(lab)}];})):null);}
  return prepared.get(id);
}
// A lightness of the night on the theme's page, as a place between two of the palette's colours: its
// ground is the page, its surfaces rise to the leaf, and its light words fall to the ink; what lies
// deeper than the night's ground sits just under the page. The neutral there is the blend of those two.
const blend=(from,to,t)=>from.map((value,index)=>value+(to[index]-value)*t);
function place(L,palette){
  const ground=NIGHT.ground[0],surface=ground+RISE,ink=NIGHT.ink[0],{ground:page,raised:leaf,ink:dark}=palette;
  if(L<=ground){const lab=[...page.lab];lab[0]=clamp(page.lab[0]-(ground-L)*0.5);return lab;}
  if(L<=surface)return blend(page.lab,leaf.lab,(L-ground)/RISE);
  const t=(L-surface)/(ink-surface),lab=blend(leaf.lab,dark.lab,clamp(t));
  if(t>1)lab[0]=clamp(dark.lab[0]-(t-1)*0.06,dark.lab[0]-0.06,1);
  return lab;
}
const fromLab=([L,A,B])=>fromLch(lchOf([L,A,B]));
const golden=([,C,h])=>C>=0.035&&h>=55&&h<=115;
const neutral=([,C,h])=>C<0.035||(C<0.075&&h>=200&&h<=290);

/**
 * A colour of the night in the theme `id`. `role` "shadow" keeps a dark shadow dark and light under a
 * light page; "paint" is every other use. Midnight and anything that is not a colour come back as given.
 */
export function themeColor(id,color,{role='paint'}={}){
  const palette=paletteOf(id),rgba=palette&&parseColor(color);if(!rgba)return color;
  const lch=toLch(rgba),[L,C,h]=lch,alpha=rgba[3];
  const ink=fromLab(palette.ink.lab);
  if(role==='shadow'&&L<0.4)return formatColor([...ink,clamp(alpha*0.4,0.04,0.3)]);
  // A dark veil over the page (a dialog's backdrop) stays a veil, in the theme's ink.
  if(role==='paint'&&alpha<=0.85&&L<NIGHT.ground[0]+0.015)return formatColor([...ink,clamp(alpha*0.45,0.03,0.5)]);
  const spot=place(L,palette),rest=role==='shadow'?alpha*0.6:alpha;
  if(neutral(lch))return formatColor([...fromLab(spot),rest]);
  if(golden(lch)){
    const target=clamp(spot[0]+palette.accent.lch[0]-place(NIGHT.gold[0],palette)[0],palette.ink.lch[0],palette.raised.lch[0]);
    return formatColor([...fromLch([target,palette.accent.lch[1]*clamp(C/NIGHT.gold[1],0.35,1.5),palette.accent.lch[2]]),rest]);
  }
  return formatColor([...fromLch([spot[0],Math.min(C*1.05,0.16),h]),rest]);
}

const COLOR=/#[0-9a-fA-F]{3,8}\b|rgba?\([^)]*\)|\b(?:white|black)\b/g;
// Quoted strings and url() hold no colours of the theme (an SVG's #id, an icon's own fill).
const OPAQUE=/("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|url\([^)]*\))/;
const PAINTING=/^(?:color|background(?:-color|-image)?|border(?:-(?:top|right|bottom|left|block|inline)(?:-(?:start|end))?)?(?:-color)?|outline(?:-color)?|fill|stroke|box-shadow|text-shadow|caret-color|accent-color|text-decoration(?:-color)?|column-rule(?:-color)?|filter|color-scheme|--[\w-]+)$/i;

/** A declaration's value in the theme `id`. */
export function themeValue(id,value,property=''){
  if(/^color-scheme$/i.test(property))return value.replace(/\bdark\b/i,'light');
  const role=/shadow/i.test(property)||/drop-shadow/i.test(value)?'shadow':'paint';
  return value.split(OPAQUE).map((part,index)=>index%2?part:part.replace(COLOR,match=>themeColor(id,match,{role}))).join('');
}
const paints=(property,value)=>PAINTING.test(property);

/** `selector` under the theme `id` on the document's root. */
export function themeSelector(selector,id){
  const scope=`[${THEME_ATTRIBUTE}="${id}"]`,trimmed=selector.trim();
  if(/^:root(?![\w-])/.test(trimmed))return trimmed.replace(/^:root/,`:root${scope}`);
  if(/^html(?![\w-])/.test(trimmed))return trimmed.replace(/^html/,`html${scope}`);
  return `:root${scope} ${trimmed}`;
}

// A painting property and its longhands are one family for the cascade: `border` and `border-left-color`, `background`
// and `background-image`. A custom property is a family of its own.
const familyOf=property=>{const name=property.toLowerCase();return name.startsWith('--')?name:name.replace(/-(?:top|right|bottom|left|block|inline|start|end|color|image)/g,'');};

/**
 * The build's PostCSS plugin: after each rule that paints, one twin per light theme holding that rule's
 * painting declarations in the theme's colours. A family of properties is the themes' to turn when any of its
 * values in the sheet holds a colour; then every value of it is twinned, those that keep their colour too. Every
 * twin is more specific than every rule of the night by the same measure, so the cascade among the twins is the
 * night's own cascade, and a family no theme turns, such as a custom property holding a width, keeps the night's.
 */
export function themePostcss(){
  const light=THEMES.filter(theme=>!theme.dark);
  return {postcssPlugin:'tos-site-themes',OnceExit(root){
    const rules=[];
    root.walkRules(rule=>{
      if(rule.parent?.type==='atrule'&&/keyframes$/i.test(rule.parent.name))return;
      // A rule written for a theme already is that theme's own word; it has no night to be turned.
      if(rule.selector.includes(`[${THEME_ATTRIBUTE}`))return;
      rules.push(rule);
    });
    const declarations=rule=>rule.nodes.filter(node=>node.type==='decl'&&paints(node.prop,node.value));
    const turned=new Set();
    for(const rule of rules)for(const node of declarations(rule))if(light.some(theme=>themeValue(theme.id,node.value,node.prop)!==node.value))turned.add(familyOf(node.prop));
    for(const rule of rules){
      const painting=declarations(rule).filter(node=>turned.has(familyOf(node.prop)));
      if(!painting.length)continue;
      let at=rule;
      for(const theme of light){
        const twin=rule.clone({selectors:rule.selectors.map(selector=>themeSelector(selector,theme.id))});twin.removeAll();
        for(const node of painting)twin.append(node.clone({value:themeValue(theme.id,node.value,node.prop)}));
        at.after(twin);at=twin;
      }
    }
  }};
}
themePostcss.postcss=true;
