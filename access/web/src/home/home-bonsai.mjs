// The bonsai of the site's choice: a tree of branches and stars, grown from a seed, so the same stand
// always shows the same tree and a new stand grows its own.
const SVG='http://www.w3.org/2000/svg';
const drawn=(tag,attributes)=>{const node=document.createElementNS(SVG,tag);for(const [name,value] of Object.entries(attributes))node.setAttribute(name,String(value));return node;};
const seedOf=text=>[...text].reduce((hash,char)=>Math.imul(hash^char.charCodeAt(0),16777619)>>>0,2166136261);
function random(seed){let state=seed>>>0;return ()=>{state=(state+0x6D2B79F5)>>>0;let t=state;t=Math.imul(t^(t>>>15),t|1);t^=t+Math.imul(t^(t>>>7),t|61);return ((t^(t>>>14))>>>0)/4294967296;};}
const fixed=value=>Number(value.toFixed(1));

/** A bonsai of branches and stars, grown from its seed: the same seed grows the same tree. */
export function growBonsai({seed=1,height=1,spread=1,tilt=0,depth=4}={}){
  const next=random(seed),branches=[],stars=[];
  const branch=(x,y,angle,length,level)=>{
    const turn=angle*Math.PI/180,x2=x+Math.cos(turn)*length,y2=y+Math.sin(turn)*length;
    // Wood bends a little between its forks.
    const bend=(next()-0.5)*length*0.35,cx=(x+x2)/2-Math.sin(turn)*bend,cy=(y+y2)/2+Math.cos(turn)*bend;
    branches.push({d:`M${fixed(x)} ${fixed(y)}Q${fixed(cx)} ${fixed(cy)} ${fixed(x2)} ${fixed(y2)}`,level,width:fixed(Math.max(1,(depth-level+1)*1.3))});
    if(level>=depth){stars.push({x:x2,y:y2,level:level+1,r:2.4+next()*1.8});return;}
    const forks=level===0||next()>=0.3?2:3;
    for(let fork=0;fork<forks;fork++){
      const side=forks===2?(fork?1:-1):fork-1;
      branch(x2,y2,angle+side*(18+next()*22)*spread+(next()-0.5)*12,length*(0.7+next()*0.14),level+1);
    }
  };
  branch(100,170,-90+tilt,48*height,0);
  // The highest star is the bonsai's own.
  const crown=stars.reduce((best,star)=>star.y<best.y?star:best,stars[0]);crown.r=5.2;crown.crown=true;
  return {branches,stars};
}
const starPoints=({x,y,r})=>[[0,-r],[r*0.26,-r*0.26],[r,0],[r*0.26,r*0.26],[0,r],[-r*0.26,r*0.26],[-r,0],[-r*0.26,-r*0.26]].map(([dx,dy])=>`${fixed(x+dx)},${fixed(y+dy)}`).join(' ');

/** The bonsai of the stand `id` as an SVG that grows in (home.css): branches by depth, then its stars. */
export function bonsaiDrawing(id,index,grows=null){
  grows??={seed:seedOf(id)};
  const {branches,stars}=growBonsai(grows),twinkle=random(grows.seed+7);
  const xs=[...branches.flatMap(item=>item.d.match(/-?[\d.]+/g).map(Number).filter((_,at)=>at%2===0)),...stars.map(star=>star.x)];
  const ys=[...branches.flatMap(item=>item.d.match(/-?[\d.]+/g).map(Number).filter((_,at)=>at%2===1)),...stars.map(star=>star.y),184];
  const size=Math.max(Math.max(...xs)-Math.min(...xs),Math.max(...ys)-Math.min(...ys))+16;
  const cx=(Math.max(...xs)+Math.min(...xs))/2,cy=(Math.max(...ys)+Math.min(...ys))/2;
  const svg=drawn('svg',{class:'home-tree',viewBox:`${fixed(cx-size/2)} ${fixed(cy-size/2)} ${fixed(size)} ${fixed(size)}`,'aria-hidden':'true',focusable:'false'});
  svg.style.setProperty('--i',index);
  svg.append(drawn('path',{class:'home-pot',d:'M84 172H116L112 184H88Z'}));
  for(const item of branches){const path=drawn('path',{class:'home-branch',d:item.d,'stroke-width':item.width,pathLength:1});path.style.setProperty('--d',item.level);svg.append(path);}
  for(const star of stars){const shape=drawn('polygon',{class:star.crown?'home-star home-crown':'home-star',points:starPoints(star)});shape.style.setProperty('--d',star.level);shape.style.setProperty('--t',fixed(twinkle()*3));svg.append(shape);}
  return svg;
}

