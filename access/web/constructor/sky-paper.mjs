import {parseColor} from '../src/public-research/themes.mjs';

// The day turns the night's canvas over with the page's colour filter and lays it on the page's paper with a multiply
// blend (site-theme.css). The GPU painter can do both in one pass of its own (gpu-canvas.js setPaper) when it reads the
// same filter and paper the page computes. Each filter function is the colour matrix the browser's compositor applies
// for it, in encoded sRGB, clamped after every step. Anything else (another function, another blend, a paper that is
// not a colour or an ellipse of colour stops) stays with the page.

const LUMA=[.213,.715,.072];
const diagonal=(scale,offset)=>({matrix:[scale,0,0,0,scale,0,0,0,scale],offset:[offset,offset,offset]});
const amount=text=>{const value=parseFloat(text);return text.trim().endsWith('%')?value/100:value;};
const angle=text=>{
  const match=/^(-?[\d.]+)(deg|rad|grad|turn)?$/.exec(text.trim());if(!match)return null;
  const value=Number(match[1]),unit=match[2]??'deg';
  return unit==='deg'?value:unit==='rad'?value*180/Math.PI:unit==='grad'?value*.9:value*360;
};
const STEPS={
  invert:a=>diagonal(1-2*a,a),
  brightness:a=>diagonal(a,0),
  contrast:a=>diagonal(a,.5-.5*a),
  saturate:s=>({matrix:[
    LUMA[0]+.787*s,LUMA[1]-.715*s,LUMA[2]-.072*s,
    LUMA[0]-.213*s,LUMA[1]+.285*s,LUMA[2]-.072*s,
    LUMA[0]-.213*s,LUMA[1]-.715*s,LUMA[2]+.928*s],offset:[0,0,0]}),
  'hue-rotate':degrees=>{const c=Math.cos(degrees*Math.PI/180),s=Math.sin(degrees*Math.PI/180);return {matrix:[
    LUMA[0]+c*.787-s*.213,LUMA[1]-c*.715-s*.715,LUMA[2]-c*.072+s*.928,
    LUMA[0]-c*.213+s*.143,LUMA[1]+c*.285+s*.140,LUMA[2]-c*.072-s*.283,
    LUMA[0]-c*.213-s*.787,LUMA[1]-c*.715+s*.715,LUMA[2]+c*.928+s*.072],offset:[0,0,0]};},
};

/** The steps of a computed CSS filter as colour matrices, or null when one of its functions is not a colour matrix. */
export function filterSteps(text){
  const value=String(text??'').trim();if(!value||value==='none')return null;
  const steps=[];let rest=value;
  while(rest){
    const match=/^([a-z-]+)\(([^()]*)\)\s*/.exec(rest);if(!match||!STEPS[match[1]])return null;
    const argument=match[1]==='hue-rotate'?angle(match[2]||'0deg'):match[2].trim()===''?1:amount(match[2]);
    if(!Number.isFinite(argument))return null;
    steps.push(STEPS[match[1]](argument));rest=rest.slice(match[0].length);
  }
  return steps.length&&steps.length<=6?steps:null;
}

/** A colour through the steps, as the page's filter turns it: each channel in 0..1. */
export function throughSteps(steps,rgb){
  let c=rgb.slice(0,3);
  for(const {matrix:m,offset:o} of steps)c=[0,1,2].map(row=>Math.min(1,Math.max(0,m[row*3]*c[0]+m[row*3+1]*c[1]+m[row*3+2]*c[2]+o[row])));
  return c;
}

/** The paper under the canvas: its colour stops and, for an ellipse, its centre and radii in the box's own units. */
export function paperOf(backgroundImage,backgroundColor){
  const image=String(backgroundImage??'none').trim();
  if(image==='none'){const color=parseColor(backgroundColor??'');return color&&color[3]===1?{stops:[[color[0],color[1],color[2],0]],center:[.5,.5],radii:[1,1]}:null;}
  const match=/^radial-gradient\((?:ellipse\s+)?([\d.]+)%\s+([\d.]+)%\s+at\s+([\d.]+)%\s+([\d.]+)%\s*,(.*)\)$/.exec(image);if(!match)return null;
  const stops=match[5].split(/,(?![^(]*\))/).map(part=>{
    const stop=/^\s*(rgba?\([^)]*\)|#[0-9a-f]+)\s+([\d.]+)%\s*$/i.exec(part);const color=stop&&parseColor(stop[1]);
    return color&&color[3]===1?[color[0],color[1],color[2],Number(stop[2])/100]:null;
  });
  if(!stops.length||stops.length>8||stops.some(stop=>!stop)||stops.some((stop,i)=>i&&stop[3]<stops[i-1][3]))return null;
  return {stops,center:[Number(match[3])/100,Number(match[4])/100],radii:[Number(match[1])/100,Number(match[2])/100]};
}

/** What the painter needs to lay the canvas on the page's paper itself, read from the page, or null. */
export function readPaper(canvas,ground){
  const style=getComputedStyle(canvas),under=getComputedStyle(ground);
  if(style.mixBlendMode!=='multiply'||style.opacity!=='1')return null;
  const steps=filterSteps(style.filter),paper=paperOf(under.backgroundImage,under.backgroundColor);
  if(!steps||!paper)return null;
  const box=ground.getBoundingClientRect(),sky=canvas.getBoundingClientRect();
  if(Math.abs(box.left-sky.left)>.5||Math.abs(box.top-sky.top)>.5||Math.abs(box.width-sky.width)>.5||Math.abs(box.height-sky.height)>.5)return null;
  const scaleX=canvas.width/Math.max(1,sky.width),scaleY=canvas.height/Math.max(1,sky.height);
  return {steps,stops:paper.stops,center:[paper.center[0]*canvas.width,paper.center[1]*canvas.height],
    radii:[Math.max(1e-3,paper.radii[0]*sky.width*scaleX),Math.max(1e-3,paper.radii[1]*sky.height*scaleY)]};
}
