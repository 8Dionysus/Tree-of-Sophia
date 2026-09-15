import { SophiaGpuCanvas } from '../src/observatory/gpu-canvas.js';
import {validateSkyPose} from './sky-pose.mjs';

// Spatial view over a session-local graph; preserves the observatory atmosphere.
export function mountConstructorSky(root, {onSelect, onMove, onEdgeSelect=()=>{}}) {
  let canvas=root.querySelector('canvas'),ctx;
  try {ctx=new SophiaGpuCanvas(canvas);root.dataset.renderer='three-webgl2';}
  catch {const next=canvas.cloneNode();canvas.replaceWith(next);canvas=next;ctx=canvas.getContext('2d',{alpha:false});root.dataset.renderer='canvas-fallback';}
  if(!ctx)throw new Error('Canvas unavailable');
  const reduced=matchMedia('(prefers-reduced-motion: reduce)');
  const design={atmosphere:'midnight',density:.85};
  const FOCAL=1250;
  let seed=7239;const rnd=()=>{seed=(seed*1664525+1013904223)>>>0;return seed/4294967296};
  const smooth=(a,b,v)=>{const x=Math.max(0,Math.min(1,(v-a)/(b-a)));return x*x*(3-2*x)};
  const dust=Array.from({length:2200},(_,i)=>{
    const layer=i%25===0?2:i%5<2?1:0;
    const sx=(rnd()-.5)*2.75;
    const sy=rnd()<.58&&layer===0?sx*.34+(rnd()+rnd()+rnd()-1.5)*.54:(rnd()-.5)*2.7;
    const minZ=[-2400,-920,220][layer],range=[1530,1150,570][layer];
    const phase=rnd()*Math.PI*2;
    return{layer,sx,sy,minZ,range,offset:rnd(),cycle:[230,135,92][layer]*(.8+rnd()*.5),
      size:layer===2?1.25+rnd()*1.5:layer===1?.65+rnd()*.85:.38+rnd()*.65,
      phase,freq:.55+rnd()*1.5,tone:Math.floor(rnd()*6),alpha:[.30,.58,.36][layer]*(.55+rnd()*.65),
      flare:rnd()>.95,drift:(rnd()-.5)*26,x:0,y:0};
  });
  const starColors=['#bdd5ed','#95bde0','#e3eaf5','#dfcfb5','#aeb8e7','#f0e3c9'];
  const sprites=new Map();
  function sprite(color){if(sprites.has(color))return sprites.get(color);const c=document.createElement('canvas');c.width=c.height=128;const g=c.getContext('2d'),r=g.createRadialGradient(64,64,0,64,64,64);r.addColorStop(0,color+'c0');r.addColorStop(.05,color+'80');r.addColorStop(.16,color+'30');r.addColorStop(.45,color+'0c');r.addColorStop(1,color+'00');g.fillStyle=r;g.fillRect(0,0,128,128);sprites.set(color,c);return c;}
  const nebula=document.createElement('canvas');nebula.width=1100;nebula.height=780;
  const ng=nebula.getContext('2d');
  function paintNebula(){
    ng.fillStyle='#04070e';ng.fillRect(0,0,1100,780);
    const cool=design.atmosphere==='deepblue';
    const clouds=[[210,260,390,cool?'#10264c':'#111c33'],[620,410,375,cool?'#102d3a':'#142c31'],[930,555,330,'#231d38'],[585,405,170,'#242523']];
    for(const[x,y,r,c]of clouds){const a=ng.createRadialGradient(x,y,0,x,y,r);a.addColorStop(0,c+'b0');a.addColorStop(.5,c+'65');a.addColorStop(1,c+'00');ng.fillStyle=a;ng.fillRect(0,0,1100,780)}
    ctx.invalidateImage?.(nebula);
  }
  function noise(x,y,s){
    const hash=(a,b)=>{let n=Math.imul(a,374761393)+Math.imul(b,668265263)+Math.imul(s,1442695041);n=Math.imul(n^(n>>>13),1274126177);return((n^(n>>>16))>>>0)/4294967295};
    const ix=Math.floor(x),iy=Math.floor(y),fx=x-ix,fy=y-iy,u=fx*fx*(3-2*fx),v=fy*fy*(3-2*fy);
    const a=hash(ix,iy),b=hash(ix+1,iy),c=hash(ix,iy+1),d=hash(ix+1,iy+1);
    return(a+(b-a)*u)*(1-v)+(c+(d-c)*u)*v;
  }
  function cloudTexture(color,seedValue){
    const c=document.createElement('canvas');c.width=320;c.height=240;
    const g=c.getContext('2d'),im=g.createImageData(c.width,c.height),data=im.data;
    for(let y=0;y<c.height;y++)for(let x=0;x<c.width;x++){
      const nx=x/c.width,ny=y/c.height;
      const warp=noise(nx*3,ny*3,seedValue);
      let f=0,amp=.55,freq=3;
      for(let k=0;k<4;k++){f+=noise(nx*freq+warp*.8,ny*freq,seedValue+k)*amp;amp*=.5;freq*=2}
      const band=Math.exp(-Math.pow((ny-(.43+Math.sin(nx*4.4+seedValue)*.14))/ .23,2));
      const edge=Math.sin(Math.PI*nx)*Math.sin(Math.PI*ny);
      const density=Math.max(0,f-.34)*band*edge;
      const j=(y*c.width+x)*4;data[j]=color[0];data[j+1]=color[1];data[j+2]=color[2];data[j+3]=Math.min(130,Math.round(density*220));
    }
    g.putImageData(im,0,0);return c;
  }
  const fogBack=cloudTexture([76,102,151],17),fogFront=cloudTexture([111,91,136],53);

  let w=1,h=1,dpr=1,base=1,skyTime=0,yaw=0,pitch=-.055,zoom=1;
  let tyaw=0,tpitch=-.055,tzoom=1,pan={x:0,y:0},tpan={x:0,y:0},eye={x:0,y:0},calm=0;
  let skyCos=1,skySin=0,skyPitchCos=1,skyPitchSin=0,last=0,raf=0,hover=null,playing=!reduced.matches,drag=null,disposed=false;
  let frameCount=0,drawSum=0,frameSum=0;
  const clamp=(v,a,b)=>Math.max(a,Math.min(b,v));
  const makeLayer=className=>{let el=root.querySelector('.'+className);if(!el){el=document.createElement('div');el.className=className;root.append(el);}Object.assign(el.style,{position:'absolute',inset:'0',pointerEvents:'none'});return el;};
  const layer=makeLayer('tree-stars'),clusterLayer=makeLayer('tree-clusters'),edgeLayer=makeLayer('edge-labels');
  const relationColors={contains:'#829dbb',supports:'#b7c991',interprets:'#bea7e1',questions:'#e5a18f',contrasts:'#e09591',echoes:'#8ecbc4',develops:'#e0c48c',compares:'#96b7e4',translates:'#8ebdc9',relates:'#a5b7cf'};
  const hex=(value,fallback)=>/^#[a-f\d]{6}$/i.test(value??'')?value:/^#[a-f\d]{3}$/i.test(value??'')?'#'+[...value.slice(1)].map(c=>c+c).join(''):fallback;
  let nodes=[],edges=[],clusters=[],selectedId=null,selectedEdgeId=null,hoverEdge=null,linkFrom=null,center=[0,0,0],targetCenter=[0,0,0],fitScale=1,targetFit=1,edgeHits=[],measureLabels=true;
  let viewport={left:0,right:1,top:0,bottom:1},targetViewport={...viewport},uiObstacles=[];
  const colorOf=n=>hex(n.color,({work:'#a8cde5',part:'#88aaca',chapter:'#b2aedc',fragment:'#dac697',dossier:'#c7a5e6',concept:'#f4cf87',interpretation:'#ba9be5',question:'#83cec7',note:'#b9c7d5',excerpt:'#dfc18e',figure:'#b9c9e5',symbol:'#91ccc5',tradition:'#b5abd3'}[n.kind]||'#bdcfdf'));
  function refresh(state,labels={}){
    const old=new Map(nodes.map(n=>[n.id,n])),oldEdges=new Map(edges.map(e=>[e.id,e]));
    edges=state.edges.map(item=>{
      const previous=oldEdges.get(item.id);oldEdges.delete(item.id);
      const el=previous?.el??document.createElement('button');
      if(!previous){el.type='button';el.className='edge-label';el.dataset.edge=item.id;Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'auto'});edgeLayer.append(el);el.addEventListener('click',e=>{e.stopPropagation();onEdgeSelect(item.id);});el.addEventListener('pointerenter',()=>{hoverEdge=item.id;kick();});el.addEventListener('pointerleave',()=>{hoverEdge=null;kick();});el.addEventListener('focus',()=>{hoverEdge=item.id;kick();});el.addEventListener('blur',()=>{hoverEdge=null;kick();});}
      const title=typeof item.label==='string'?item.label:item.kind;
      el.textContent=title;el.setAttribute('aria-label',`${labels[item.from]??item.from} → ${title} → ${labels[item.to]??item.to}`);el.hidden=true;
      return {...item,el,color:hex(item.color,relationColors[item.kind]??relationColors.relates)};
    });oldEdges.forEach(e=>e.el.remove());
    if('selectedEdgeId' in state)selectedEdgeId=state.selectedEdgeId;
    nodes=state.nodes.map(item=>{
      let n=old.get(item.id);old.delete(item.id);
      if(!n){
        const el=document.createElement('button');el.type='button';el.className='tree-star';el.dataset.node=item.id;
        const label=document.createElement('span');el.append(label);layer.append(el);n={id:item.id,el,label,screen:{x:0,y:0,s:1},p:[...item.position],target:[...item.position],dragging:false};
        let nodeDrag=null,suppress=false;
        el.addEventListener('click',()=>{if(suppress){suppress=false;return;}onSelect(item.id)});
        el.addEventListener('pointerdown',e=>{if(e.button!==0)return;suppress=false;n.dragging=true;nodeDrag={id:e.pointerId,x:e.clientX,y:e.clientY,p:[...n.p],moved:false};el.setPointerCapture(e.pointerId);e.stopPropagation()});
        el.addEventListener('pointermove',e=>{if(!nodeDrag||e.pointerId!==nodeDrag.id)return;const dx=e.clientX-nodeDrag.x,dy=e.clientY-nodeDrag.y;if(Math.hypot(dx,dy)<5&&!nodeDrag.moved)return;nodeDrag.moved=true;const vx=dx/Math.max(.1,n.screen.sX??n.screen.s),vy=dy/Math.max(.1,n.screen.s),c=Math.cos(yaw),ss=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch);n.p=[nodeDrag.p[0]+vx*c-vy*ss*sp,nodeDrag.p[1]+vy*cp,nodeDrag.p[2]-vx*ss-vy*c*sp];n.target=[...n.p];kick();});
        const end=e=>{if(!nodeDrag||e.pointerId!==nodeDrag.id)return;const moved=nodeDrag.moved;nodeDrag=null;n.dragging=false;if(moved){suppress=true;onMove?.(item.id,[...n.p]);}kick();};
        el.addEventListener('pointerup',end);el.addEventListener('pointercancel',end);el.addEventListener('lostpointercapture',end);
        el.addEventListener('pointerenter',()=>{hover=item.id;kick()});el.addEventListener('pointerleave',()=>{hover=null;kick()});
        el.addEventListener('focus',()=>{hover=item.id;kick()});el.addEventListener('blur',()=>{hover=null;kick()});
      }
      n.target=[...item.position];n.kind=item.kind;n.color=item.color;n.prominent=!!item.prominent;n.major=item.major??['work','concept','dossier','figure'].includes(item.kind);n.label.textContent=labels[item.id]??item.title??item.id;n.el.setAttribute('aria-label',item.accessibilityLabel??n.label.textContent);
      if(item.hover)n.el.title=item.hover;else n.el.removeAttribute('title');
      if(item.ownerType)n.el.dataset.ownerType=item.ownerType;else delete n.el.dataset.ownerType;
      return n;
    });old.forEach(n=>n.el.remove());
    const oldClusters=new Map(clusters.map(c=>[c.id,c]));
    clusters=(state.clusters??[]).map(item=>{let c=oldClusters.get(item.id);oldClusters.delete(item.id);if(!c){const el=document.createElement('div'),title=document.createElement('span'),subtitle=document.createElement('small');el.className='tree-cluster';title.className='cluster-title';subtitle.className='cluster-subtitle';el.append(title,subtitle);Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'none'});clusterLayer.append(el);c={el,title,subtitle,p:[...item.center]};}c.id=item.id;c.target=[...item.center];c.title.textContent=item.title??'';c.subtitle.textContent=item.subtitle??'';c.el.style.setProperty('--cluster-color',hex(item.color,'#a5b7cf'));c.el.style.color=hex(item.color,'#a5b7cf');return c;});oldClusters.forEach(c=>c.el.remove());
    if(!nodes.some(n=>n.id===hover))hover=null;if(!edges.some(e=>e.id===hoverEdge))hoverEdge=null;
    measureLabels=true;resize();
  }
  function project([x,y,z]) {
    const c=Math.cos(yaw),s=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch);
    x=(x-center[0])*fitScale;y=(y-center[1])*fitScale;z=(z-center[2])*fitScale;
    const rx=x*c-z*s,rz=x*s+z*c,ry=y*cp-rz*sp,zz=y*sp+rz*cp,depth=FOCAL/Math.max(130,FOCAL-zz);
    // Fill a wide viewport with the atlas while stars and atmosphere keep their shape.
    const stretchX=clamp(((viewport.right-viewport.left)/Math.max(1,viewport.bottom-viewport.top))/(880/540),1,2.1),scale=base*fitScale*zoom*depth;
    return {x:(viewport.left+viewport.right)*.5+rx*base*zoom*depth*stretchX+pan.x,y:(viewport.top+viewport.bottom)*.5+ry*base*zoom*depth+pan.y,s:scale,sX:scale*stretchX};
  }
  function resize(){
    const first=w===1;w=Math.max(1,root.clientWidth);h=Math.max(1,root.clientHeight);dpr=Math.min(devicePixelRatio||1,1.5);
    const cw=Math.round(w*dpr),ch=Math.round(h*dpr);if(canvas.width!==cw)canvas.width=cw;if(canvas.height!==ch)canvas.height=ch;
    const desktop=w>760,cinema=root.dataset.cinema==='true',reading=!cinema&&root.dataset.reading==='true',deck=!cinema&&root.dataset.deck==='true',journey=!cinema&&root.dataset.journey==='true';
    targetViewport={left:desktop?166:24,right:w-(desktop?(reading?350:30):24),top:desktop?185:145,bottom:h-(desktop?(journey?260:deck?225:90):(reading?Math.max(180,h*.48):deck?150:65))};
    targetViewport.right=Math.max(targetViewport.left+120,targetViewport.right);targetViewport.bottom=Math.max(targetViewport.top+130,targetViewport.bottom);
    const bounds=root.getBoundingClientRect();uiObstacles=[];
    for(const el of root.querySelectorAll('.view-heading,.header,.lens-rail,.collection-deck,.relation-bar,.footer,.reading,.journey-dock')){const style=getComputedStyle(el),rect=el.getBoundingClientRect();if(el.hidden||style.display==='none'||style.visibility==='hidden'||Number(style.opacity)===0||!rect.width||!rect.height)continue;uiObstacles.push({left:rect.left-bounds.left,top:rect.top-bounds.top,right:rect.right-bounds.left,bottom:rect.bottom-bounds.top});}
    if(first)viewport={...targetViewport};measureLabels=true;kick();
  }
  function paintFog(texture,phase,alpha){
    ctx.save();ctx.globalCompositeOperation='screen';ctx.globalAlpha=alpha;
    ctx.translate(w*.5+Math.sin(skyTime*.025+phase)*w*.026+eye.x*16-yaw*27,h*.51+Math.cos(skyTime*.019+phase)*h*.024+eye.y*12);
    ctx.rotate(Math.sin(skyTime*.013+phase)*.055-yaw*.035);
    ctx.drawImage(texture,-w*.72,-h*.68,w*1.44,h*1.36);ctx.restore();
  }
  function paintStars(layer,count){
    for(let i=0;i<count;i++){
      const s=dust[i];if(s.layer!==layer)continue;
      const age=(s.offset+skyTime/s.cycle)%1,z=s.minZ+age*s.range;
      const birth=smooth(0,.065,age)*(1-smooth(.90,1,age));if(birth<.002)continue;
      const drift=Math.sin(skyTime*.035+s.phase)*s.drift;
      const x=s.x+drift,y=s.y+Math.cos(skyTime*.029+s.phase)*s.drift*.65;
      const rx=x*skyCos-z*skySin,rz=x*skySin+z*skyCos,ry=y*skyPitchCos-rz*skyPitchSin,zz=y*skyPitchSin+rz*skyPitchCos;
      if(zz>FOCAL-130)continue;
      const depth=FOCAL/(FOCAL-zz),factor=base*depth;
      const px=w*.5+rx*factor+pan.x*.18+eye.x*depth*23,py=h*.54+ry*factor+pan.y*.18+eye.y*depth*17;
      if(px<-30||py<-30||px>w+30||py>h+30)continue;
      const wave=Math.sin(skyTime*s.freq+s.phase)*.26+Math.sin(skyTime*s.freq*.43+s.phase*2.13)*.16;
      const flare=s.flare?Math.pow(Math.max(0,Math.sin(skyTime*.23+s.phase*1.7)),28):0;
      const alpha=Math.min(.92,s.alpha*birth*(.66+wave+flare*.8))*(1-calm*.16);
      const size=Math.min(layer===2?3.3:2.2,Math.max(.38,s.size*depth*(.64+base*.32)));
      const color=starColors[s.tone];ctx.fillStyle=color;ctx.globalAlpha=alpha;
      if(layer===2){const haze=17+size*8;ctx.drawImage(sprite(color),px-haze/2,py-haze/2,haze,haze);ctx.globalAlpha=alpha*.7;ctx.beginPath();ctx.arc(px,py,size*.62,0,Math.PI*2);ctx.fill()}
      else{ctx.fillRect(px-size/2,py-size/2,size,size);if(s.flare||size>1.5){const glow=12+size*5+flare*17;ctx.globalAlpha=alpha*.6;ctx.drawImage(sprite(color),px-glow/2,py-glow/2,glow,glow)}}
      if(s.flare&&flare>.25&&layer!==0){ctx.globalAlpha=alpha*flare*.52;const ray=3+flare*7;ctx.lineWidth=.6;ctx.strokeStyle=color;ctx.beginPath();ctx.moveTo(px-ray,py);ctx.lineTo(px+ray,py);ctx.moveTo(px,py-ray);ctx.lineTo(px,py+ray);ctx.stroke()}
      if(i===25)root.dataset.starProbe=[px.toFixed(2),py.toFixed(2),alpha.toFixed(3)].join(',');
    }
    ctx.globalAlpha=1;
  }

  const overlaps=(a,b,gap=5)=>a.left<b.right+gap&&a.right>b.left-gap&&a.top<b.bottom+gap&&a.bottom>b.top-gap;
  const inView=box=>box.left>=targetViewport.left&&box.right<=targetViewport.right&&box.top>=targetViewport.top&&box.bottom<=targetViewport.bottom;
  const box=(x,y,width,height)=>({left:x,top:y,right:x+width,bottom:y+height});
  function measure(){
    if(!measureLabels)return;measureLabels=false;
    for(const n of nodes){const style=getComputedStyle(n.label);ng.font=style.font||'12px sans-serif';n.labelWidth=Math.min(root.dataset.journey==='true'?230:180,Math.max(50,ng.measureText(n.label.textContent).width+4));n.labelHeight=Math.max(16,parseFloat(style.lineHeight)||18);}
    for(const e of edges){const style=getComputedStyle(e.el);ng.font=style.font||'11px sans-serif';e.labelWidth=Math.min(190,Math.max(56,ng.measureText(e.el.textContent).width+20));e.labelHeight=Math.max(24,(parseFloat(style.lineHeight)||16)+10);}
  }
  function strokeSegment(a,b,dashed=false){
    const dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);if(length<1)return;
    ctx.beginPath();
    if(dashed){for(let start=0;start<length;start+=13){const end=Math.min(start+7,length);ctx.moveTo(a.x+dx*start/length,a.y+dy*start/length);ctx.lineTo(a.x+dx*end/length,a.y+dy*end/length);}}
    else{ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);}ctx.stroke();
  }
  function arrow(a,b){
    const dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);if(length<35)return;
    const ux=dx/length,uy=dy/length,x=b.x-ux*13,y=b.y-uy*13;
    ctx.beginPath();ctx.moveTo(x-ux*8-uy*4,y-uy*8+ux*4);ctx.lineTo(x,y);ctx.lineTo(x-ux*8+uy*4,y-uy*8-ux*4);ctx.stroke();
  }
  function hitEdge(x,y){
    if(!inView(box(x,y,0,0)))return null;
    let closest=null,best=8;
    for(const edge of edgeHits){const {a,b}=edge,dx=b.x-a.x,dy=b.y-a.y,d=dx*dx+dy*dy;if(d<1)continue;const t=clamp(((x-a.x)*dx+(y-a.y)*dy)/d,0,1);if(t<.07||t>.93)continue;const distance=Math.hypot(x-a.x-dx*t,y-a.y-dy*t);if(distance<best){closest=edge.id;best=distance;}}
    return closest;
  }
  function labelsForScene(near,focusEdges){
    measure();const occupied=[...uiObstacles];
    // Cluster names orient the atlas before the smaller labels compete for room.
    for(const c of clusters){
      const width=Math.min(230,Math.max(110,c.title.textContent.length*8));let area=null;
      for(const offset of [-101,-76,106]){const p=project([c.p[0],c.p[1]+offset,c.p[2]]),top=offset===-101?Math.max(targetViewport.top+4,p.y-20):p.y-20,candidate=box(p.x-width/2,top,width,c.subtitle.textContent?40:25);if(inView(candidate)&&!occupied.some(o=>overlaps(candidate,o))){area=candidate;break;}}
      c.el.hidden=!area;
      if(area){c.el.style.width=width+'px';c.el.style.transform=`translate3d(${area.left.toFixed(2)}px,${area.top.toFixed(2)}px,0)`;occupied.push(area);}
    }
    const nodeObstacles=nodes.filter(n=>!n.el.hidden).map(n=>({id:n.id,...box(n.screen.x-9,n.screen.y-9,18,18)}));
    const priority=n=>n.id===selectedId?100:n.id===hover?90:n.prominent?80:near.has(n.id)?70:n.major?50:10;
    for(const n of [...nodes].sort((a,b)=>priority(b)-priority(a))){
      const {x,y}=n.screen,width=n.labelWidth,height=n.labelHeight,hot=n.id===selectedId||n.id===hover;
      const possibilities=[box(x-width/2,y+19,width,height),box(x-width/2,y-height-19,width,height),box(x+19,y-height/2,width,height),box(x-width-19,y-height/2,width,height)];
      const within=possibilities.filter(inView),clear=within.find(rect=>!occupied.some(o=>overlaps(rect,o))&&!nodeObstacles.some(o=>o.id!==n.id&&overlaps(rect,o,2)));
      const chosen=clear||(hot?within[0]:n.prominent?within.find(rect=>!occupied.some(o=>overlaps(rect,o))):null),visible=!!chosen&&!n.el.hidden;
      n.labelBox=visible?chosen:null;n.el.dataset.quiet=String(!visible);n.label.style.opacity=visible?'1':'0';n.label.style.pointerEvents=visible?'auto':'none';
      if(chosen){Object.assign(n.label.style,{left:(chosen.left-x+22)+'px',top:(chosen.top-y+22)+'px',bottom:'auto',transform:'none',width:width+'px',maxWidth:width+'px',minWidth:'0'});if(visible)occupied.push(chosen);}
    }
    const candidates=edges.filter(e=>e.id===selectedEdgeId||e.id===hoverEdge||((hover||selectedId)&&focusEdges.has(e.id))).sort((a,b)=>(b.id===hoverEdge?100:b.id===selectedEdgeId?90:b.highlight?70:0)-(a.id===hoverEdge?100:a.id===selectedEdgeId?90:a.highlight?70:0));
    const visibleEdges=new Set();let shown=0;
    for(const e of candidates){
      if(shown>=6)break;const hit=edgeHits.find(hit=>hit.id===e.id);if(!hit)continue;
      const {a,b}=hit,dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy),hot=e.id===hoverEdge||e.id===selectedEdgeId;if(length<70&&!hot)continue;
      let chosen=null;
      for(const t of [.5,.35,.65]){const x=a.x+dx*t,y=a.y+dy*t,area=box(x-e.labelWidth/2,y-e.labelHeight/2,e.labelWidth,e.labelHeight);if(inView(area)&&!occupied.some(o=>overlaps(area,o))&&!nodeObstacles.some(o=>overlaps(area,o))){chosen=area;break;}}
      if(!chosen&&hot){for(const offset of [0,24,-24]){const area=box((a.x+b.x)/2-dy/Math.max(1,length)*offset-e.labelWidth/2,(a.y+b.y)/2+dx/Math.max(1,length)*offset-e.labelHeight/2,e.labelWidth,e.labelHeight);if(inView(area)&&!uiObstacles.some(o=>overlaps(area,o))&&!nodeObstacles.some(o=>overlaps(area,o))&&!nodes.some(n=>(n.id===selectedId||n.id===hover)&&n.labelBox&&overlaps(area,n.labelBox))){chosen=area;break;}}}
      if(!chosen)continue;
      if(hot)for(const n of nodes)if(n.labelBox&&overlaps(chosen,n.labelBox)){n.labelBox=null;n.el.dataset.quiet='true';n.label.style.opacity='0';n.label.style.pointerEvents='none';}
      visibleEdges.add(e.id);e.el.dataset.active=String(hot);e.el.dataset.kind=e.kind;e.el.style.setProperty('--edge-color',e.color);e.el.style.color=e.color;e.el.style.width=e.labelWidth+'px';e.el.style.transform=`translate3d(${chosen.left.toFixed(2)}px,${chosen.top.toFixed(2)}px,0)`;e.el.title=e.el.getAttribute('aria-label');occupied.push(chosen);shown++;
    }
    for(const e of edges)e.el.hidden=!visibleEdges.has(e.id);
  }

  function draw(now,still=false){
    raf=0;if(disposed||(document.hidden&&!still))return;
    const started=performance.now(),elapsed=last?(now-last)/1000:1/60,dt=Math.min(elapsed,.05),ease=(reduced.matches||still)?1:1-Math.exp(-dt*5);last=now;
    if(playing&&!still)skyTime+=dt*.60;
    yaw+=(tyaw-yaw)*ease;pitch+=(tpitch-pitch)*ease;zoom+=(tzoom-zoom)*ease;pan.x+=(tpan.x-pan.x)*ease;pan.y+=(tpan.y-pan.y)*ease;
    let layoutMoving=false;
    const approach=(current,target)=>{if(Math.abs(current-target)>.015)layoutMoving=true;return Math.abs(current-target)<.002?target:current+(target-current)*ease;};
    center=center.map((v,i)=>approach(v,targetCenter[i]));fitScale=approach(fitScale,targetFit);
    for(const side of ['left','right','top','bottom'])viewport[side]=approach(viewport[side],targetViewport[side]);
    for(const n of nodes)if(!n.dragging)n.p=n.p.map((v,i)=>approach(v,n.target[i]));
    for(const c of clusters)c.p=c.p.map((v,i)=>approach(v,c.target[i]));
    const nextBase=Math.max(.1,Math.min((viewport.right-viewport.left)/880,(viewport.bottom-viewport.top)/540,1.7));
    if(Math.abs(base-nextBase)>.00001||frameCount===0){base=nextBase;dust.forEach(s=>{const spread=(FOCAL-s.minZ-s.range*.5)/FOCAL;s.x=s.sx*w*.5/base*spread;s.y=s.sy*h*.5/base*spread;});}
    skyCos=Math.cos(yaw+Math.sin(skyTime*.021)*.011);skySin=Math.sin(yaw+Math.sin(skyTime*.021)*.011);
    skyPitchCos=Math.cos(pitch);skyPitchSin=Math.sin(pitch);
    ctx.beginFrame?.();ctx.setTransform(dpr,0,0,dpr,0,0);ctx.globalAlpha=1;ctx.globalCompositeOperation='source-over';ctx.drawImage(nebula,0,0,w,h);
    const count=Math.round(dust.length*design.density);paintStars(0,count);paintFog(fogBack,.2,.85);paintStars(1,count);paintFog(fogFront,2.4,.46);
    nodes.forEach(n=>n.screen=project(n.p));
    const byId=new Map(nodes.map(n=>[n.id,n]));
    const selectedEdge=edges.find(e=>e.id===selectedEdgeId),hoveredEdge=edges.find(e=>e.id===hoverEdge),focusId=hover??selectedId,near=new Set(focusId?[focusId]:[]),focusEdges=new Set();
    for(const edge of [selectedEdge,hoveredEdge])if(edge){near.add(edge.from);near.add(edge.to);focusEdges.add(edge.id);}
    for(const edge of edges)if(edge.from===focusId||edge.to===focusId){near.add(edge.from);near.add(edge.to);focusEdges.add(edge.id);}
    const focused=near.size>0;edgeHits=[];
    for(const edge of edges){
      const a=byId.get(edge.from)?.screen,b=byId.get(edge.to)?.screen;if(!a||!b)continue;
      const hot=edge.id===hoverEdge||edge.id===selectedEdgeId||edge.highlight,neighbor=focusEdges.has(edge.id);
      ctx.strokeStyle=edge.color;ctx.globalAlpha=hot?.85:neighbor?.57:focused?.055:.24;ctx.lineWidth=hot?1.55:neighbor?1.1:.65;
      strokeSegment(a,b,edge.kind==='questions'||edge.kind==='contrasts');
      if(hot||neighbor)arrow(a,b);
      edgeHits.push({id:edge.id,a,b});
    }
    nodes.forEach(n=>{
      const p=n.screen,hot=n.id===selectedId||n.id===hover,color=colorOf(n),big=n.major,dim=focused&&root.dataset.journey!=='true'&&!near.has(n.id)&&!hot,radius=(hot?5.2:big?4.3:2.8)*Math.max(.70,Math.min(1.5,p.s));
      ctx.globalAlpha=dim?.28:hot?1:.9;const glow=(hot?125:big?86:46)*Math.max(.65,Math.min(1.4,p.s));
      ctx.drawImage(sprite(color),p.x-glow/2,p.y-glow/2,glow,glow);ctx.fillStyle=color;ctx.beginPath();ctx.arc(p.x,p.y,radius,0,Math.PI*2);ctx.fill();
      ctx.fillStyle='#fff9ea';ctx.fillRect(p.x-1,p.y-1,2,2);
      const ray=hot?17:big?10:5;ctx.globalAlpha=dim?.12:hot?.9:.38;ctx.strokeStyle=color;ctx.lineWidth=.65;ctx.beginPath();ctx.moveTo(p.x-ray,p.y);ctx.lineTo(p.x+ray,p.y);ctx.moveTo(p.x,p.y-ray);ctx.lineTo(p.x,p.y+ray);ctx.stroke();
      if(n.id===selectedId||n.id===linkFrom){ctx.globalAlpha=.65;ctx.beginPath();for(const [sx,sy] of [[-1,-1],[-1,1],[1,-1],[1,1]]){ctx.moveTo(p.x+sx*22,p.y+sy*16);ctx.lineTo(p.x+sx*22,p.y+sy*22);ctx.lineTo(p.x+sx*16,p.y+sy*22);}ctx.stroke();}
      n.el.style.transform=`translate3d(${(p.x-22).toFixed(2)}px,${(p.y-22).toFixed(2)}px,0)`;
      n.el.dataset.active=String(n.id===selectedId);n.el.dataset.major=String(!!big);n.el.dataset.prominent=String(n.prominent);n.el.dataset.dim=String(dim);n.el.style.opacity=dim?'.35':'1';n.el.setAttribute('aria-pressed',String(n.id===selectedId));n.el.style.setProperty('--star-color',color);
      n.el.hidden=!inView(box(p.x-5,p.y-5,10,10));
    });
    labelsForScene(near,focusEdges);
    paintStars(2,count);ctx.globalAlpha=1;ctx.endFrame?.();
    frameCount++;drawSum+=performance.now()-started;frameSum+=elapsed*1000;
    if(frameCount%90===0){root.dataset.drawMs=(drawSum/90).toFixed(2);root.dataset.frameMs=(frameSum/90).toFixed(2);root.dataset.frames=String(frameCount);drawSum=frameSum=0;}
    root.dataset.camera=[yaw,pitch,zoom,pan.x,pan.y].map(x=>x.toFixed(3)).join(',');
    const unsettled=Math.abs(yaw-tyaw)+Math.abs(pitch-tpitch)+Math.abs(zoom-tzoom)+Math.abs(pan.x-tpan.x)+Math.abs(pan.y-tpan.y)>.005;
    if(!still&&(playing||unsettled||layoutMoving||drag))kick();else last=0;
  }
  function kick(){if(disposed)return;if(document.hidden){draw(performance.now(),true);return;}if(!raf)raf=requestAnimationFrame(draw)}
  canvas.addEventListener('wheel',e=>{e.preventDefault();tzoom=clamp(tzoom*Math.exp(-e.deltaY*.0012),.65,2.2);kick()},{passive:false});
  const pointer=e=>{const rect=canvas.getBoundingClientRect();return{x:e.clientX-rect.left,y:e.clientY-rect.top};};
  canvas.addEventListener('pointerdown',e=>{if(e.button!==0)return;drag={id:e.pointerId,x:e.clientX,y:e.clientY,yaw:tyaw,pitch:tpitch,pan:{...tpan},shift:e.shiftKey,moved:false};canvas.setPointerCapture(e.pointerId);canvas.classList.add('dragging')});
  canvas.addEventListener('pointermove',e=>{if(!drag){const p=pointer(e),id=hitEdge(p.x,p.y);if(hoverEdge!==id){hoverEdge=id;canvas.style.cursor=id?'pointer':'';kick();}return;}if(e.pointerId!==drag.id)return;const dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.hypot(dx,dy)<5&&!drag.moved)return;drag.moved=true;hoverEdge=null;if(drag.shift){tpan={x:drag.pan.x+dx,y:drag.pan.y+dy}}else{tyaw=clamp(drag.yaw+dx*.0025,-.9,.9);tpitch=clamp(drag.pitch+dy*.0025,-.6,.6)}kick()});
  const release=e=>{if(!drag||e.pointerId!==drag.id)return;const click=e.type==='pointerup'&&!drag.moved;drag=null;canvas.classList.remove('dragging');if(click){const p=pointer(e),id=hitEdge(p.x,p.y);if(id)onEdgeSelect(id);}kick();};canvas.addEventListener('pointerup',release);canvas.addEventListener('pointercancel',release);canvas.addEventListener('lostpointercapture',release);
  canvas.addEventListener('pointerleave',()=>{if(!drag){hoverEdge=null;canvas.style.cursor='';kick();}});
  const visibility=()=>{last=0;if(document.hidden){cancelAnimationFrame(raf);raf=0}else kick()};document.addEventListener('visibilitychange',visibility);
  canvas.addEventListener('sophia-renderer-restored',kick);
  const ro=new ResizeObserver(resize);ro.observe(root);paintNebula();resize();
  return {
    update:refresh,
    capturePose(){return validateSkyPose({v:1,yaw:tyaw,pitch:tpitch,zoom:tzoom,pan:[tpan.x,tpan.y],center:[...targetCenter],fit:targetFit});},
    restorePose(value){const pose=validateSkyPose(value);yaw=tyaw=pose.yaw;pitch=tpitch=pose.pitch;zoom=tzoom=pose.zoom;
      pan={x:pose.pan[0],y:pose.pan[1]};tpan={...pan};center=[...pose.center];targetCenter=[...pose.center];fitScale=targetFit=pose.fit;kick();},
    select(id){selectedId=id;kick()},
    selectEdge(id){selectedEdgeId=id;kick()},
    link(id){linkFrom=id;kick()},
    frame(){if(nodes.length){const min=[0,1,2].map(i=>Math.min(...nodes.map(n=>n.target[i]))),max=[0,1,2].map(i=>Math.max(...nodes.map(n=>n.target[i])));targetCenter=min.map((v,i)=>(v+max[i])/2);targetFit=Math.min(1.6,810/Math.max(510,max[0]-min[0]),455/Math.max(285,max[1]-min[1]),900/Math.max(560,max[2]-min[2]));}tyaw=0;tpitch=-.055;tzoom=1;tpan={x:0,y:0};resize()},
    focus(id){const n=nodes.find(n=>n.id===id);if(!n)return;targetCenter=[...n.target];tzoom=1.3;tpan={x:0,y:0};kick();},
    refresh:resize,
    motion(value){playing=value;kick()},
    dispose(){disposed=true;cancelAnimationFrame(raf);ro.disconnect();document.removeEventListener('visibilitychange',visibility);canvas.removeEventListener('sophia-renderer-restored',kick);nodes.forEach(n=>n.el.remove());edges.forEach(e=>e.el.remove());clusters.forEach(c=>c.el.remove());ctx.dispose?.()}
  };
}
