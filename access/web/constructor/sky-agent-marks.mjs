// Agents at work on a star (C7, sky-agents.mjs), drawn with what both sky painters draw: straight strokes,
// full discs and images. The GPU painter (src/observatory/gpu-canvas.js) fills only discs and strokes only
// straight segments; it has no dashes and no text.
//
// One agent is a small diamond beside the star: a square on its corner, one straight stroke as wide as it is
// long. Several are a broken ring whose size grows with their number. Near enough, the ring names their roles:
// the line is painted once into an image of its own (textImage) and drawn as one.
export const AGENT_COLOR='#7fd0c4',AGENT_NEAR_ZOOM=2.6;

const DIAMOND=3.2,DASH=2.5;

/** The roles a ring names: the three most active, an ellipsis for more. */
export function agentRolesLine(roles){
  const list=(roles??[]).filter(Boolean);
  return list.length?list.slice(0,3).join(' · ')+(list.length>3?' …':''):'';
}

/**
 * Draw the agents of one star at screen point (x, y) of radius `radius`. `alpha` is the star's fade; `roles`
 * are named only when `near`; `textImage(text, dpr)` returns an image of the line in CSS pixels times `dpr`.
 */
export function drawAgentMarks(ctx,{x,y,radius,count,roles=[],near=false,alpha=1,dpr=1,textImage=null}){
  if(!(count>0))return;
  ctx.strokeStyle=ctx.fillStyle=AGENT_COLOR;
  if(count===1){
    // The diamond's corners lie DIAMOND from its centre; its side is DIAMOND·√2, the stroke runs along one diagonal.
    const cx=x+radius+6,cy=y-(radius+6),reach=DIAMOND/2;
    ctx.globalAlpha=.9*alpha;ctx.lineCap='butt';ctx.lineWidth=DIAMOND*Math.SQRT2;
    ctx.beginPath();ctx.moveTo(cx-reach,cy-reach);ctx.lineTo(cx+reach,cy+reach);ctx.stroke();
  }else{
    // Every other side of a polygon fine enough to read round: dashes about DASH long with gaps as long.
    const around=radius+5+3.2*Math.log2(count),steps=2*Math.max(6,Math.round(Math.PI*around/DASH));
    ctx.globalAlpha=.75*alpha;ctx.lineWidth=1.3;ctx.beginPath();
    for(let i=0;i<steps;i+=2){
      const a=i/steps*Math.PI*2,b=(i+1)/steps*Math.PI*2;
      ctx.moveTo(x+Math.cos(a)*around,y+Math.sin(a)*around);ctx.lineTo(x+Math.cos(b)*around,y+Math.sin(b)*around);
    }
    ctx.stroke();
  }
  const line=near?agentRolesLine(roles):'';
  if(line&&textImage){
    const image=textImage(line,dpr);
    if(image){const w=image.width/dpr,h=image.height/dpr;ctx.globalAlpha=.9*alpha;ctx.drawImage(image,x+radius+12,y+radius+9-h/2,w,h);}
  }
}

/** Role lines painted at the painter's pixel ratio and kept, for `drawAgentMarks`'s textImage. */
export function agentTextImages(makeCanvas=()=>document.createElement('canvas'),limit=64){
  const cache=new Map();
  return (text,dpr=1)=>{
    const key=`${dpr}|${text}`;
    if(cache.has(key))return cache.get(key);
    const font='11px system-ui,sans-serif',canvas=makeCanvas(),probe=canvas.getContext('2d');
    probe.font=font;
    const width=Math.ceil(probe.measureText(text).width)+2,height=16;
    canvas.width=Math.max(1,Math.ceil(width*dpr));canvas.height=Math.ceil(height*dpr);
    const g=canvas.getContext('2d');
    g.setTransform(dpr,0,0,dpr,0,0);g.font=font;g.textBaseline='middle';g.fillStyle=AGENT_COLOR;g.fillText(text,1,height/2);
    if(cache.size>=limit)cache.clear();
    cache.set(key,canvas);
    return canvas;
  };
}
