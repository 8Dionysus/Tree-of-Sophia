import { SophiaGpuCanvas } from '../src/observatory/gpu-canvas.js';
import {validateSkyPose} from './sky-pose.mjs';
import {scaleLabelClear} from './scale-labels.mjs';

// Spatial view over a session-local graph; preserves the observatory atmosphere.
// Options are opt-in so the constructor keeps its own behaviour: rotate:false
// makes a plain drag pan and leaves turning to Shift+drag, and getViewport
// supplies the actually free area. For a space that is read in depth: rest is
// the pose the view returns to and frames in; intro lets the space turn into
// that pose once, so its volume shows; holdToMove (ms) picks a star up only
// once it is held still, while a quick drag from a star turns the view with
// turnFromStars; touch gives two fingers the pan and the pinch; secondaryPan
// pans with the right or middle button; inertia lets a turn coast to rest;
// depthCue dims what lies further back; planarMoves keeps a moved star in its
// own depth; onPose hears every turn of the view. Such a space may lay its
// stars on named planes of depth (state.planes), shown while it turns.
export function mountConstructorSky(root, {onSelect, onMove, onEdgeSelect=()=>{}, onBackground=null, getViewport=null, rotate=true, dragNodes=true, rest=null, intro=false, holdToMove=0, turnFromStars=false, touch=false, secondaryPan=false, inertia=false, depthCue=false, planarMoves=false, onPose=null, nativeTitles=true, onEdgeHover=null, onThreadSelect=null, threadsFor=null, obstacles=null}) {
  let canvas=root.querySelector('canvas'),ctx;
  try {ctx=new SophiaGpuCanvas(canvas);root.dataset.renderer='three-webgl2';}
  catch {const next=canvas.cloneNode();canvas.replaceWith(next);canvas=next;ctx=canvas.getContext('2d',{alpha:false});root.dataset.renderer='canvas-fallback';}
  if(!ctx)throw new Error('Canvas unavailable');
  let rendererName='';try{const gl=ctx.renderer?.getContext?.(),debug=gl?.getExtension('WEBGL_debug_renderer_info');if(debug)rendererName=gl.getParameter(debug.UNMASKED_RENDERER_WEBGL)||'';}catch{}
  const softwareRenderer=!rendererName||/swiftshader|llvmpipe|softpipe|lavapipe|software rasterizer/i.test(rendererName);
  if(ctx instanceof SophiaGpuCanvas&&softwareRenderer){
    ctx.dispose();const next=canvas.cloneNode();canvas.replaceWith(next);canvas=next;ctx=canvas.getContext('2d',{alpha:false});root.dataset.renderer='canvas2d-fallback';
    if(!ctx)throw new Error('Canvas unavailable');
  }
  const reduced=matchMedia('(prefers-reduced-motion: reduce)');
  const design={atmosphere:'midnight',density:softwareRenderer?.34:.58};
  const FOCAL=1250;
  // A star's ray in three steps from the star outwards: where each begins and ends along the ray, its light and its width.
  const RAY_STEPS=[[0,.42,1,.85],[.42,.74,.5,.68],[.74,1,.18,.52]],RAY_DIRS=[[1,0],[-1,0],[0,1],[0,-1]];
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

  // ── The day. A light page shows this canvas through invert(1) hue-rotate(180deg) contrast(1.25) saturate(1.25)
  // (site-theme.css): the night turned over. The day's own light and colours are drawn here as the night colours
  // that filter turns into them, so the day is a cosmos turned over: lit paper where the night was dark, tinted
  // shadow where it had clouds, motes of gold where it had stars.
  const LUMA=[.213,.715,.072],DAY_CONTRAST=1.25,DAY_SATURATE=1.25;
  function nightOf(hex){
    let c=[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)/255);
    const luma=v=>LUMA[0]*v[0]+LUMA[1]*v[1]+LUMA[2]*v[2];
    let l=luma(c);c=c.map(v=>l+(v-l)/DAY_SATURATE);            // saturate undone
    c=c.map(v=>(v-.5)/DAY_CONTRAST+.5);                         // contrast undone
    l=luma(c);c=c.map(v=>2*l-v);                                // hue-rotate(180deg) is its own inverse
    return '#'+c.map(v=>Math.round(Math.max(0,Math.min(1,1-v))*255).toString(16).padStart(2,'0')).join('');
  }
  const nightCache=new Map(),night=hex=>{let v=nightCache.get(hex);if(!v){v=nightOf(hex);nightCache.set(hex,v);}return v;};
  const mixHex=(a,b,t)=>{if(t<=0)return a;const p=x=>[1,3,5].map(i=>parseInt(x.slice(i,i+2),16)),A=p(a),B=p(b);return '#'+A.map((v,i)=>Math.round(v+(B[i]-v)*Math.min(1,t)).toString(16).padStart(2,'0')).join('');};
  // The paper is a shade under white, so the noon light can stand brighter than it; washes of warm umber, lilac and
  // sage lie toward the edges as the day's coloured shadow, never round the tree.
  // The day's textures are soft, so they are kept small: they share one atlas with the night's on the GPU.
  const dayGround=document.createElement('canvas');dayGround.width=550;dayGround.height=390;
  {const g=dayGround.getContext('2d');g.scale(.5,.5);g.fillStyle=nightOf('#F8F5EF');g.fillRect(0,0,1100,780);
    for(const [x,y,r,hex,a] of [[90,740,560,'#E8DACA',.75],[1040,150,480,'#E3DEEB',.7],[1010,770,460,'#DEE5D6',.65],[40,90,400,'#F0E6D6',.55],[560,860,420,'#EDE3D5',.5]]){
      const n=nightOf(hex),grad=g.createRadialGradient(x,y,0,x,y,r);grad.addColorStop(0,n+Math.round(a*255).toString(16).padStart(2,'0'));grad.addColorStop(.55,n+Math.round(a*110).toString(16).padStart(2,'0'));grad.addColorStop(1,n+'00');g.fillStyle=grad;g.fillRect(0,0,1100,780);}}
  const radialTexture=(hex,size,stops)=>{const c=document.createElement('canvas');c.width=c.height=size;const g=c.getContext('2d'),h=size/2,grad=g.createRadialGradient(h,h,0,h,h,h),n=nightOf(hex);for(const [at,a] of stops)grad.addColorStop(at,n+Math.round(a*255).toString(16).padStart(2,'0'));g.fillStyle=grad;g.fillRect(0,0,size,size);return c;};
  // The noon light from above, a shadow's soft blot, and a ray of light falling from the top.
  const dayLight=radialTexture('#FFFFFF',256,[[0,1],[.35,.62],[.7,.18],[1,0]]);
  const dayShadow=radialTexture('#5C4E40',256,[[0,.9],[.4,.55],[.75,.16],[1,0]]);
  const dayRay=document.createElement('canvas');dayRay.width=64;dayRay.height=512;
  {const g=dayRay.getContext('2d'),n=nightOf('#FFFFFF'),im=g.createImageData(64,512),d=im.data,[r,gg,b]=[1,3,5].map(i=>parseInt(n.slice(i,i+2),16));
    for(let y=0;y<512;y++){const t=y/512,half=5+26*t,fade=Math.sin(Math.PI*Math.min(1,t*1.15))*(1-t*.55);
      for(let x=0;x<64;x++){const k=(y*64+x)*4,dx=(x-32)/half,a=Math.exp(-dx*dx*2.2)*fade;d[k]=r;d[k+1]=gg;d[k+2]=b;d[k+3]=Math.round(a*120);}}
    g.putImageData(im,0,0);}
  // Stars of the night turn into motes of gold drifting in the light.
  const moteColors=['#B48C3C','#C49E57','#A27A3A','#CFAE6C','#9A8467','#B5966C'].map(nightOf);
  // The tree's ground, when the layout has one: its shadow lies on it in the day.
  let shade=null;

  // The rest the view returns to; a layout may set its own (a lens that measures in volume is seen from above and aside).
  const baseRestYaw=Number.isFinite(rest?.yaw)?rest.yaw:0,baseRestPitch=Number.isFinite(rest?.pitch)?rest.pitch:-.055;
  let restYaw=baseRestYaw,restPitch=baseRestPitch;
  // The view turns all the way round and over the top and the bottom, stopping short of the poles,
  // where up would flip. A return to a pose takes the shorter way round.
  const PITCH=[-1.45,1.45],TURN=Math.PI*2;
  const nearestTurn=(target,from)=>target+TURN*Math.round((from-target)/TURN);
  let w=1,h=1,dpr=1,base=1,skyTime=0,yaw=restYaw,pitch=restPitch,zoom=1;
  let tyaw=restYaw,tpitch=restPitch,tzoom=1,pan={x:0,y:0},tpan={x:0,y:0},eye={x:0,y:0},calm=0,revealId=null,stretchY=1,tstretchY=1,measured=null;
  // The fit the framing chose for the space seen from the front, and the pose and layout the turned fit was last worked out for.
  let frontFit=0,fittedFor='',layoutVersion=0,neighbourhoodPending=null;
  // In a space laid out whole (the tree) a chosen star brings its neighbourhood near and to the middle of the view.
  let closeIn=false,edgeChosenAt=-Infinity;
  // How far the view has gathered to a chosen star: 0 open, 1 gathered.
  let focusMix=0;
  // A turn keeps its last speed when let go and coasts to rest; a pose heard last is not told again.
  let velocity=null,lastTurn=null,introDone=false,introPending=false,introUntil=0,turnShownUntil=0,reveal=0,heardPose='',framedOnce=false;
  let skyCos=1,skySin=0,skyPitchCos=1,skyPitchSin=0,last=0,lastFrame=0,raf=0,hover=null,playing=!reduced.matches&&!softwareRenderer,drag=null,disposed=false;
  let frameCount=0,drawSum=0,frameSum=0;
  const clamp=(v,a,b)=>Math.max(a,Math.min(b,v));
  const makeLayer=className=>{let el=root.querySelector('.'+className);if(!el){el=document.createElement('div');el.className=className;root.append(el);}Object.assign(el.style,{position:'absolute',inset:'0',pointerEvents:'none'});return el;};
  const layer=makeLayer('tree-stars'),clusterLayer=makeLayer('tree-clusters'),edgeLayer=makeLayer('edge-labels');
  // A scale's rules and rows' threads lie under the stars.
  const ruleLayer=makeLayer('tree-rules');if(ruleLayer.nextElementSibling!==layer)root.insertBefore(ruleLayer,layer);
  // A plane's parallels and a measure's rings are clipped to the free area, under the rules.
  const ringLayer=makeLayer('tree-rings');if(ringLayer.nextElementSibling!==ruleLayer)root.insertBefore(ringLayer,ruleLayer);
  const underStars=new Set(['rule','thread','hrule','ring']),clipped=new Set(['hrule','ring']);
  const relationColors={contains:'#829dbb',supports:'#b7c991',interprets:'#bea7e1',questions:'#e5a18f',contrasts:'#e09591',echoes:'#8ecbc4',develops:'#e0c48c',compares:'#96b7e4',translates:'#8ebdc9',relates:'#a5b7cf'};
  const hex=(value,fallback)=>/^#[a-f\d]{6}$/i.test(value??'')?value:/^#[a-f\d]{3}$/i.test(value??'')?'#'+[...value.slice(1)].map(c=>c+c).join(''):fallback;
  // How strongly links and lines are inked: more on a light page, where a thin dark line reads fainter than its twin on the night.
  let ink=1;
  // On a light page depth is the day's air: what lies further back pales into the paper, and a near star
  // stands over the page with a soft shadow. 0 on the night, 1 on a light page.
  const daylight=()=>ink>1?1:0;
  let nodes=[],edges=[],clusters=[],backdrop=[],planes=[],threads=[],lit=null,litGlow=0,selectedId=null,selectedEdgeId=null,hoverEdge=null,linkFrom=null,center=[0,0,0],targetCenter=[0,0,0],fitScale=1,targetFit=1,edgeHits=[],measureLabels=true,aspectIds=null;
  let viewport={left:0,right:1,top:0,bottom:1},targetViewport={...viewport},uiObstacles=[];
  const colorOf=n=>hex(n.color,({work:'#a8cde5',part:'#88aaca',chapter:'#b2aedc',fragment:'#dac697',dossier:'#c7a5e6',concept:'#f4cf87',interpretation:'#ba9be5',question:'#83cec7',note:'#b9c7d5',excerpt:'#dfc18e',figure:'#b9c9e5',symbol:'#91ccc5',tradition:'#b5abd3'}[n.kind]||'#bdcfdf'));
  function refresh(state,labels={}){
    const old=new Map(nodes.map(n=>[n.id,n])),oldEdges=new Map(edges.map(e=>[e.id,e])),before=new Map(nodes.map(n=>[n.id,[...n.p]]));
    edges=state.edges.map(item=>{
      const previous=oldEdges.get(item.id);oldEdges.delete(item.id);
      const el=previous?.el??document.createElement('button');
      if(!previous){el.type='button';el.className='edge-label';el.dataset.edge=item.id;Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'auto'});edgeLayer.append(el);el.addEventListener('click',e=>{e.stopPropagation();onEdgeSelect(item.id);});el.addEventListener('pointerenter',()=>{hoverEdge=item.id;kick();});el.addEventListener('pointerleave',()=>{hoverEdge=null;kick();});el.addEventListener('focus',()=>{hoverEdge=item.id;kick();});el.addEventListener('blur',()=>{hoverEdge=null;kick();});}
      const title=typeof item.label==='string'?item.label:item.kind,tag=typeof item.tag==='string'&&item.tag?item.tag:'';
      if(tag){const word=document.createElement('b'),text=document.createElement('span');word.className='edge-tag';word.textContent=tag;text.textContent=title;el.replaceChildren(word,text);}else el.textContent=title;
      el.setAttribute('aria-label',`${labels[item.from]??item.from} → ${tag?tag+': ':''}${title} → ${labels[item.to]??item.to}`);el.hidden=true;el.dataset.oneLine=String(Boolean(item.labelOneLine&&!tag));
      return {...item,el,color:hex(item.color,relationColors[item.kind]??relationColors.relates)};
    });oldEdges.forEach(e=>e.el.remove());
    if('selectedEdgeId' in state)selectedEdgeId=state.selectedEdgeId;
    // An aspect's stars are what a framing fits; the stars stepped back stay out of its measure.
    aspectIds=Array.isArray(state.aspect?.ids)&&state.aspect.ids.length?new Set(state.aspect.ids):null;
    nodes=state.nodes.map(item=>{
      let n=old.get(item.id);old.delete(item.id);
      if(!n){
        const el=document.createElement('button');el.type='button';el.className='tree-star';el.dataset.node=item.id;
        const label=document.createElement('span');el.append(label);layer.append(el);
        // A new star may rise out of another one (a thought out of what it is about) and move to its own place.
        const origin=typeof item.from==='string'?before.get(item.from):Array.isArray(item.from)&&item.from.length===3&&item.from.every(Number.isFinite)?item.from:null;
        n={id:item.id,el,label,screen:{x:0,y:0,s:1},p:[...(origin??item.position)],target:[...item.position],dragging:false};
        let nodeDrag=null,press=null,swallowed=false;
        const movable=()=>typeof dragNodes==='function'?dragNodes(item.id):dragNodes;
        const pickUp=(pointerId,x,y)=>{n.dragging=true;nodeDrag={id:pointerId,x,y,p:[...n.p],moved:false};};
        // The click that ends a drag or a pinch belongs to it; a new press starts clean, and the keyboard (detail 0) always chooses.
        const swallow=()=>{swallowed=true;};
        el.addEventListener('click',e=>{if(e.detail!==0&&swallowed){swallowed=false;return;}onSelect(item.id)});
        // A star held still is picked up and moves; a quick drag from it turns the view, as the sky around it does.
        el.addEventListener('contextmenu',e=>{if(holdToMove||secondaryPan)e.preventDefault();});
        // What a press began ends without a trace: on a pinch the view goes back and a picked-up star returns;
        // a star that leaves the view lets its press go.
        const settle=restore=>{
          clearTimeout(press?.timer);
          if(press?.view){if(restore){tyaw=press.view.yaw;tpitch=press.view.pitch;tpan={...press.view.pan};}canvas.classList.remove('dragging');}
          n.turningView=false;
          if(nodeDrag){if(restore){n.p=[...nodeDrag.p];n.target=[...n.p];}nodeDrag=null;n.dragging=false;delete el.dataset.held;}
          press=null;presses.delete(cancel);kick();
        };
        const cancel=()=>{settle(true);swallow();};
        n.abandon=()=>settle(false);
        el.addEventListener('pointerdown',e=>{
          if(pinching(e))return;
          swallowed=false;
          const panButton=secondaryPan&&(e.button===1||e.button===2);
          if(e.button!==0&&!panButton)return;
          // A press stops a turn that is still coasting.
          velocity=null;lastTurn=null;
          // The right or middle button moves the view from a star as from the sky.
          if(panButton){e.preventDefault();revealId=null;press={id:e.pointerId,x:e.clientX,y:e.clientY,view:{yaw:tyaw,pitch:tpitch,pan:{...tpan},shift:false,forcePan:true},timer:null,moved:false};presses.add(cancel);el.setPointerCapture(e.pointerId);e.stopPropagation();canvas.classList.add('dragging');return;}
          if(!holdToMove){if(!movable())return;pickUp(e.pointerId,e.clientX,e.clientY);el.setPointerCapture(e.pointerId);e.stopPropagation();return;}
          const can=movable();if(!can&&!turnFromStars)return;
          press={id:e.pointerId,x:e.clientX,y:e.clientY,view:null,moved:false,timer:can?setTimeout(()=>{if(press&&!press.view){pickUp(press.id,press.x,press.y);el.dataset.held='true';kick();}},holdToMove):null};
          presses.add(cancel);el.setPointerCapture(e.pointerId);e.stopPropagation();
        });
        el.addEventListener('pointermove',e=>{
          if(pinching(e))return;
          if(nodeDrag&&e.pointerId===nodeDrag.id){
            const dx=e.clientX-nodeDrag.x,dy=e.clientY-nodeDrag.y;if(Math.hypot(dx,dy)<5&&!nodeDrag.moved)return;nodeDrag.moved=true;
            // The pointer's move in the turned view, in the space's own units before it was widened.
            const u=dx/Math.max(.01,(n.screen.k??base*zoom)*fitScale),v=dy/Math.max(.01,(n.screen.kY??base*zoom)*fitScale),sx=n.screen.stretchX??1,sy=stretchY||1;
            const c=Math.cos(yaw),ss=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch),edge=value=>Math.sign(value||1)*Math.max(.2,Math.abs(value));
            // In a space read in depth a star keeps its depth and moves in its own plane, also when that plane is seen from behind.
            if(planarMoves){const ax=u/(sx*edge(c)),ay=(v+sx*ax*ss*sp)/(sy*Math.max(.2,cp));n.p=[nodeDrag.p[0]+ax,nodeDrag.p[1]+ay,nodeDrag.p[2]];}
            else n.p=[nodeDrag.p[0]+(u*c-v*ss*sp)/sx,nodeDrag.p[1]+v*cp/sy,nodeDrag.p[2]-u*ss-v*c*sp];
            n.target=[...n.p];kick();return;
          }
          if(!press||e.pointerId!==press.id)return;
          const dx=e.clientX-press.x,dy=e.clientY-press.y;
          if(!press.view){if(Math.hypot(dx,dy)<5)return;clearTimeout(press.timer);if(!turnFromStars){press=null;presses.delete(cancel);return;}revealId=null;velocity=null;lastTurn=null;press.view={yaw:tyaw,pitch:tpitch,pan:{...tpan},shift:e.shiftKey,forcePan:false};n.turningView=!(rotate?e.shiftKey:!e.shiftKey);canvas.classList.add('dragging');}
          if(Math.hypot(dx,dy)>=5)press.moved=true;
          hoverEdge=null;dragBy(press.view,dx,dy);
        });
        const end=e=>{
          if(nodeDrag&&e.pointerId===nodeDrag.id){const moved=nodeDrag.moved;nodeDrag=null;n.dragging=false;delete el.dataset.held;press=null;presses.delete(cancel);if(moved){swallow();onMove?.(item.id,[...n.p]);}kick();return;}
          if(!press||e.pointerId!==press.id)return;
          clearTimeout(press.timer);presses.delete(cancel);
          if(press.view){if(press.moved||press.view.forcePan)swallow();n.turningView=false;canvas.classList.remove('dragging');letGo();}
          press=null;kick();
        };
        el.addEventListener('pointerup',end);el.addEventListener('pointercancel',end);el.addEventListener('lostpointercapture',end);
        el.addEventListener('pointerenter',()=>{hover=item.id;kick()});el.addEventListener('pointerleave',()=>{hover=null;kick()});
        el.addEventListener('focus',()=>{hover=item.id;kick()});el.addEventListener('blur',()=>{hover=null;kick()});
      }
      n.target=[...item.position];n.kind=item.kind;n.color=item.color;n.prominent=!!item.prominent;n.dimmed=!!item.dim;n.labelSide=['left','right','above','below'].includes(item.labelSide)?item.labelSide:null;n.quiet=!!item.quiet;n.form=item.form==='proposal'||item.form==='personal'?item.form:'source';n.el.dataset.form=n.form;n.major=item.major??['work','concept','dossier','figure'].includes(item.kind);n.labelFrom=Number.isFinite(item.labelFrom)?item.labelFrom:0;
      n.titleText=labels[item.id]??item.title??item.id;n.subtitle=typeof item.subtitle==='string'?item.subtitle:'';
      n.labelMaxWidth=Number.isFinite(item.labelMaxWidth)?Math.max(80,Math.min(320,item.labelMaxWidth)):null;n.labelLines=item.labelLines===2?2:1;
      const clampStyle={whiteSpace:n.labelLines>1?'normal':'',display:n.labelLines>1?'-webkit-box':'',webkitBoxOrient:n.labelLines>1?'vertical':'',webkitLineClamp:n.labelLines>1?String(n.labelLines):''};
      // How the name is clamped when it is not shown whole.
      n.clampStyle=clampStyle;n.fullShown=false;
      if(n.subtitle){
        // A second line names the source; the name keeps its own clamp.
        if(!n.titleEl){n.titleEl=document.createElement('b');n.titleEl.className='star-title';n.subEl=document.createElement('small');n.subEl.className='star-sub';}
        n.titleEl.textContent=n.titleText;n.subEl.textContent=n.subtitle;n.label.replaceChildren(n.titleEl,n.subEl);
        Object.assign(n.label.style,{whiteSpace:'normal',display:'block',webkitBoxOrient:'',webkitLineClamp:''});Object.assign(n.titleEl.style,{...clampStyle,display:n.labelLines>1?'-webkit-box':'block',overflow:'hidden'});
      }else{n.label.textContent=n.titleText;Object.assign(n.label.style,clampStyle);}
      n.el.setAttribute('aria-label',item.accessibilityLabel??n.titleText);
      if(item.hover&&nativeTitles)n.el.title=item.hover;else n.el.removeAttribute('title');
      if(item.ownerType)n.el.dataset.ownerType=item.ownerType;else delete n.el.dataset.ownerType;
      return n;
    });old.forEach(n=>{n.abandon?.();n.el.remove();});
    const oldClusters=new Map(clusters.map(c=>[c.id,c]));
    clusters=(state.clusters??[]).map(item=>{let c=oldClusters.get(item.id);oldClusters.delete(item.id);if(!c){const el=document.createElement('div'),title=document.createElement('span'),subtitle=document.createElement('small');el.className='tree-cluster';title.className='cluster-title';subtitle.className='cluster-subtitle';el.append(title,subtitle);Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'none'});clusterLayer.append(el);c={el,title,subtitle,p:[...item.center]};}c.id=item.id;c.target=[...item.center];c.variant=typeof item.variant==='string'?item.variant:'';c.recede=!!item.recede;c.yields=!!item.yields;if(c.variant)c.el.dataset.variant=c.variant;else delete c.el.dataset.variant;c.strip=Number.isFinite(item.strip)?item.strip:null;c.span=Array.isArray(item.span)&&item.span.length===2&&item.span.every(Number.isFinite)?[...item.span]:null;c.align=item.align==='start'||item.align==='end'?item.align:'';if(c.align)c.el.dataset.align=c.align;else delete c.el.dataset.align;c.maxWidth=Number.isFinite(item.maxWidth)?item.maxWidth:null;c.radius=Number.isFinite(item.radius)?item.radius:null;const home=clipped.has(c.variant)?ringLayer:underStars.has(c.variant)?ruleLayer:clusterLayer;if(c.el.parentElement!==home)home.append(c.el);if(item.ref){c.el.dataset.ref=String(item.ref);c.el.style.pointerEvents='auto';c.el.setAttribute('role','button');c.el.tabIndex=0;}else{delete c.el.dataset.ref;c.el.style.pointerEvents='none';c.el.removeAttribute('role');c.el.removeAttribute('tabindex');}if(item.tone)c.el.dataset.tone=String(item.tone);else delete c.el.dataset.tone;if(item.hint&&nativeTitles)c.el.title=item.hint;else c.el.removeAttribute('title');if(item.hint&&!nativeTitles)c.el.dataset.hint=item.hint;else delete c.el.dataset.hint;c.title.textContent=item.title??'';c.subtitle.textContent=item.subtitle??'';c.el.style.setProperty('--cluster-color',hex(item.color,'#a5b7cf'));c.el.style.color=hex(item.color,'#a5b7cf');return c;});oldClusters.forEach(c=>c.el.remove());
    // A measured plane may lie on a backdrop of lines in its own units, such as coastlines.
    // A backdrop line is its points, or its points with a colour, an alpha and a weight in the layout's units.
    shade=state.shade&&Array.isArray(state.shade.center)&&Number.isFinite(state.shade.radius)?state.shade:null;
    backdrop=Array.isArray(state.backdrop)?state.backdrop.filter(line=>(Array.isArray(line)?line:line?.points)?.length>1):[];
    // A space read in depth may lay its stars on planes, each at its own depth and named at a corner.
    const oldPlanes=new Map(planes.map(plane=>[plane.id,plane]));
    planes=(Array.isArray(state.planes)?state.planes:[]).filter(item=>typeof item?.id==='string'&&Number.isFinite(item.z)&&[item.x,item.y].every(range=>Array.isArray(range)&&range.length===2&&range.every(Number.isFinite))).map(item=>{
      let plane=oldPlanes.get(item.id);oldPlanes.delete(item.id);
      if(!plane){const el=document.createElement('div');el.className='tree-plane';el.dataset.plane=item.id;el.hidden=true;Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'none'});ruleLayer.append(el);plane={id:item.id,el};}
      plane.el.textContent=typeof item.label==='string'?item.label:'';plane.z=item.z;plane.x=[...item.x];plane.y=[...item.y];plane.color=hex(item.color,'#a9bdd6');plane.el.style.color=plane.color;return plane;
    });oldPlanes.forEach(plane=>plane.el.remove());
    updateThreads();
    if(!nodes.some(n=>n.id===hover))hover=null;if(!edges.some(e=>e.id===hoverEdge))hoverEdge=null;
    layoutVersion++;measureLabels=true;resize();
  }
  // Threads lead from the chosen star into the dark, toward stars the space does not show; each ends in a small door.
  // The page says which threads a star has; they follow the chosen star wherever it is chosen from.
  function updateThreads(){
    const list=typeof threadsFor==='function'&&selectedId?threadsFor(selectedId)??[]:[];
    const oldThreads=new Map(threads.map(thread=>[thread.id,thread]));
    threads=(Array.isArray(list)?list:[]).filter(item=>typeof item?.id==='string'&&typeof item.from==='string'&&Array.isArray(item.direction)&&item.direction.length===2&&item.direction.every(Number.isFinite)).map(item=>{
      let thread=oldThreads.get(item.id);oldThreads.delete(item.id);
      if(!thread){const el=document.createElement('button');el.type='button';el.className='tree-thread';el.dataset.thread=item.id;el.hidden=true;Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'auto'});el.addEventListener('click',event=>{event.stopPropagation();onThreadSelect?.(item.id);});layer.append(el);thread={id:item.id,el,grown:0};}
      thread.from=item.from;thread.direction=[...item.direction];thread.color=hex(item.color,'#c9b88e');thread.el.setAttribute('aria-label',typeof item.label==='string'?item.label:item.id);return thread;
    });oldThreads.forEach(thread=>thread.el.remove());
    // A thread's door comes right after its star in the keyboard's order.
    let after=nodes.find(n=>n.id===selectedId)?.el;for(const thread of threads)if(after?.isConnected){if(after.nextSibling!==thread.el)after.after(thread.el);after=thread.el;}
  }
  // How much wider than the atlas's own proportion the free area is. A plane measures both axes in one unit,
  // so it is never widened; other layouts fill a wide viewport.
  const widen=(area=viewport)=>measured?.axes==='xy'?1:clamp(((area.right-area.left)/Math.max(1,area.bottom-area.top))/(880/540),1,2.1);
  function project([x,y,z]) {
    const c=Math.cos(yaw),s=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch),stretchX=widen();
    // The space is widened in its own coordinates, before it turns, so that a turn moves it as one rigid body
    // and never stretches what comes round from the depth.
    x=(x-center[0])*fitScale*stretchX;y=(y-center[1])*fitScale*stretchY;z=(z-center[2])*fitScale;
    const rx=x*c-z*s,rz=x*s+z*c,ry=y*cp-rz*sp,zz=y*sp+rz*cp,depth=FOCAL/Math.max(130,FOCAL-zz),scale=base*fitScale*zoom*depth;
    // A scale zooms and pans along its axis only and its rows keep their height; a plane zooms and pans both ways.
    const scaleOnly=measured?.axes==='x',zoomY=scaleOnly?1:zoom;
    return {x:(viewport.left+viewport.right)*.5+rx*base*zoom*depth+pan.x,y:(viewport.top+viewport.bottom)*.5+ry*base*zoomY*depth+(scaleOnly?0:pan.y),s:scale,sX:scale*stretchX,sY:base*fitScale*zoomY*depth*stretchY,k:base*zoom*depth,kY:base*zoomY*depth,stretchX,d:depth};
  }
  // The fit that keeps the whole space in the free area as it is turned: the front framing's fit, made smaller
  // where the turned space is wider, taller or deeper than the front one. The space never grows past the front fit,
  // so a flat layout seen edge-on keeps its scale.
  function turnFit(){
    if(measured||!frontFit||!nodes.length)return;
    const key=`${tyaw.toFixed(3)},${tpitch.toFixed(3)},${layoutVersion},${frontFit},${targetCenter.join(',')},${Object.values(targetViewport).map(v=>v.toFixed(0)).join(',')}`;
    if(key===fittedFor)return;fittedFor=key;
    const c=Math.cos(tyaw),s=Math.sin(tyaw),cp=Math.cos(tpitch),sp=Math.sin(tpitch),sx=widen(targetViewport);
    const low=[Infinity,Infinity,Infinity],high=[-Infinity,-Infinity,-Infinity],points=[];
    for(const n of nodes){
      const x=(n.target[0]-targetCenter[0])*sx,y=n.target[1]-targetCenter[1],z=n.target[2]-targetCenter[2];
      const rx=x*c-z*s,rz=x*s+z*c,turned=[rx,y*cp-rz*sp,y*sp+rz*cp];points.push(turned);
      for(let i=0;i<3;i++){low[i]=Math.min(low[i],turned[i]);high[i]=Math.max(high[i],turned[i]);}
    }
    const [dx,dy,dz]=[0,1,2].map(i=>high[i]-low[i]);
    let fit=Math.min(frontFit,810*sx/Math.max(510*sx,dx),455/Math.max(285,dy),900/Math.max(560,dz));
    // What comes nearer is drawn larger: the fit narrows until the nearer side fits as well.
    for(let pass=0;pass<2;pass++){
      let x0=Infinity,x1=-Infinity,y0=Infinity,y1=-Infinity;
      for(const [rx,ry,zz] of points){const d=FOCAL/Math.max(130,FOCAL-zz*fit);x0=Math.min(x0,rx*fit*d);x1=Math.max(x1,rx*fit*d);y0=Math.min(y0,ry*fit*d);y1=Math.max(y1,ry*fit*d);}
      fit*=Math.min(1,810*sx/Math.max(1,x1-x0),455/Math.max(1,y1-y0));
    }
    targetFit=fit;
  }
  function resize(){
    const first=w===1;w=Math.max(1,root.clientWidth);h=Math.max(1,root.clientHeight);dpr=Math.min(devicePixelRatio||1,softwareRenderer?1:1.25);
    const cw=Math.round(w*dpr),ch=Math.round(h*dpr);if(canvas.width!==cw)canvas.width=cw;if(canvas.height!==ch)canvas.height=ch;
    const desktop=w>760,cinema=root.dataset.cinema==='true',reading=!cinema&&root.dataset.reading==='true',deck=!cinema&&root.dataset.deck==='true',journey=!cinema&&root.dataset.journey==='true';
    targetViewport={left:desktop?166:24,right:w-(desktop?(reading?350:30):24),top:desktop?185:145,bottom:h-(desktop?(journey?260:deck?225:90):(reading?Math.max(180,Math.min(h*.44,420)+112):deck?150:65))};
    targetViewport.right=Math.max(targetViewport.left+120,targetViewport.right);targetViewport.bottom=Math.max(targetViewport.top+130,targetViewport.bottom);
    const bounds=root.getBoundingClientRect();uiObstacles=[];
    // A bar that lives beside the space but stands over it (`obstacles`) keeps names from under it as well.
    for(const el of [...root.querySelectorAll('.view-heading,.header,.lens-rail,.collection-deck,.relation-bar,.footer,.reading,.journey-dock,.ps-map-tools,.ps-column-tab,.ps-search-results'),...(obstacles?.()??[])]){const style=getComputedStyle(el),rect=el.getBoundingClientRect();if(el.hidden||style.display==='none'||style.visibility==='hidden'||Number(style.opacity)===0||!rect.width||!rect.height)continue;uiObstacles.push({left:rect.left-bounds.left,top:rect.top-bounds.top,right:rect.right-bounds.left,bottom:rect.bottom-bounds.top});}
    // A workspace can supply its actual free area; the constructor keeps its own layout.
    const supplied=getViewport?.({width:w,height:h});
    if(supplied&&['left','right','top','bottom'].every(k=>Number.isFinite(supplied[k]))&&supplied.right-supplied.left>=120&&supplied.bottom-supplied.top>=120)targetViewport={...supplied};
    if(first)viewport={...targetViewport};measureLabels=true;kick();
  }
  // The day's paper under the noon light: the light breathes slowly, its rays sway, and the tree's shadow lies on its ground.
  function paintDay(){
    ctx.drawImage(dayGround,0,0,w,h);
    const breath=Math.sin(skyTime*.07),size=Math.max(w,h)*1.35*(1+.035*breath),lx=w*.53+Math.sin(skyTime*.021)*w*.02-Math.sin(yaw)*w*.04,ly=-h*.18;
    ctx.globalAlpha=.88+.12*breath;ctx.drawImage(dayLight,lx-size/2,ly-size/2,size,size);
    for(let i=0;i<5;i++){
      const angle=(i-2)*.19+Math.sin(skyTime*.045+i*1.7)*.03-Math.sin(yaw)*.05,length=h*1.45,width=w*(.07+.025*(i%2));
      ctx.save();ctx.translate(lx,ly+h*.06);ctx.rotate(angle);ctx.globalAlpha=(.42+.14*Math.sin(skyTime*.06+i*2.3))*(i===2?1:.8);ctx.drawImage(dayRay,-width/2,0,width,length);ctx.restore();
    }
    ctx.globalAlpha=1;
  }
  function paintShade(){
    if(!shade||!daylight())return;
    const [cx,cy,cz]=shade.center,r=shade.radius,mid=project([cx,cy,cz]);
    const rim=[[cx+r,cy,cz],[cx-r,cy,cz],[cx,cy,cz+r],[cx,cy,cz-r]].map(project);
    const rx=Math.max(...rim.map(p=>Math.abs(p.x-mid.x)))*1.18,ry=Math.max(6,Math.max(...rim.map(p=>Math.abs(p.y-mid.y)))*1.18);
    // A soft shadow over the whole ground, and a closer, darker one where the trunk meets it; it steps back with the
    // rest of the space when a chosen star gathers the view.
    const keep=1-.9*focusMix;
    ctx.globalAlpha=.4*keep;ctx.drawImage(dayShadow,mid.x-rx,mid.y-ry,rx*2,ry*2);
    ctx.globalAlpha=.3*keep;ctx.drawImage(dayShadow,mid.x-rx*.32,mid.y-ry*.32,rx*.64,ry*.64);
    ctx.globalAlpha=1;
  }
  function paintFog(texture,phase,alpha){
    // The night's clouds turn over into blue blots on the day's paper; the day has its air and its lamp instead.
    if(daylight())return;
    ctx.save();ctx.globalCompositeOperation='screen';ctx.globalAlpha=alpha;
    // The fog leans with the turn and comes back round with it.
    ctx.translate(w*.5+Math.sin(skyTime*.025+phase)*w*.026+eye.x*16-Math.sin(yaw)*27,h*.51+Math.cos(skyTime*.019+phase)*h*.024+eye.y*12);
    ctx.rotate(Math.sin(skyTime*.013+phase)*.055-Math.sin(yaw)*.035);
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
      const color=daylight()?moteColors[s.tone]:starColors[s.tone];ctx.fillStyle=color;ctx.globalAlpha=daylight()?alpha*.8:alpha;
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
  // How many lines a text wraps to at a width, in the type last set on the measuring canvas: words wrap at spaces,
  // and a word wider than the line (a CJK phrase, a long compound) wraps between its graphemes, as CSS does.
  function lineCount(text,limit){
    let lines=1,line='';
    for(const word of String(text).split(/\s+/).filter(Boolean)){
      if(ng.measureText(word).width>limit){
        if(line){lines++;line='';}
        const characters=typeof Intl.Segmenter==='function'?Array.from(new Intl.Segmenter(undefined,{granularity:'grapheme'}).segment(word),part=>part.segment):Array.from(word);
        for(const character of characters){const next=line+character;if(line&&ng.measureText(next).width>limit){lines++;line=character;}else line=next;}
      }else{const next=line?line+' '+word:word;if(line&&ng.measureText(next).width>limit){lines++;line=word;}else line=next;}
    }
    return lines;
  }
  function measure(){
    if(!measureLabels)return;measureLabels=false;
    for(const n of nodes){
      const text=n.titleText??n.label.textContent,sub=n.subtitle&&n.subEl?getComputedStyle(n.subEl):null;
      let subWidth=0,subHeight=0;if(sub){ng.font=sub.font||'11px sans-serif';subWidth=ng.measureText(n.subtitle).width;subHeight=Math.max(14,parseFloat(sub.lineHeight)||15)+2;}
      const style=getComputedStyle(n.subtitle&&n.titleEl?n.titleEl:n.label);ng.font=style.font||'12px sans-serif';
      n.labelWidth=Math.min(n.labelMaxWidth??(root.dataset.journey==='true'?230:180),Math.max(50,Math.max(ng.measureText(text).width,subWidth)+(n.labelLines>1?8:4)));
      let lines=1,line='';
      if(n.labelLines>1)for(const word of text.split(/\s+/)){
        const limit=n.labelWidth-8;
        if(ng.measureText(word).width>limit){
          // CJK phrases and long compounds can wrap without a space. Measure
          // their graphemes, as CSS does, instead of treating them as one line.
          if(line){lines++;line='';}
          const characters=typeof Intl.Segmenter==='function'?Array.from(new Intl.Segmenter(undefined,{granularity:'grapheme'}).segment(word),part=>part.segment):Array.from(word);
          for(const character of characters){const next=line+character;if(line&&ng.measureText(next).width>limit){lines++;line=character;}else line=next;}
        }else{const next=line?line+' '+word:word;if(line&&ng.measureText(next).width>limit){lines++;line=word;}else line=next;}
      }
      n.titleHeight=Math.max(16,parseFloat(style.lineHeight)||18)*Math.min(lines,n.labelLines)+(n.labelLines>1?2:0);n.labelHeight=n.titleHeight+subHeight;
      // A name that must be read is shown whole: as wide as it needs up to 260 px, over as many as three lines.
      n.fullWidth=Math.min(n.labelMaxWidth??260,Math.max(n.labelWidth,Math.max(ng.measureText(text).width,subWidth)+8));
      n.fullTitleHeight=Math.max(16,parseFloat(style.lineHeight)||18)*Math.min(3,lineCount(text,n.fullWidth-8))+2;n.fullHeight=n.fullTitleHeight+subHeight;
    }
    // A scale's labels are measured in their own type, so a name never loses its end to a guess.
    for(const c of clusters)if(c.variant&&!underStars.has(c.variant)){ng.font=getComputedStyle(c.title).font||'12px sans-serif';c.textWidth=ng.measureText(c.title.textContent).width;}
    // A group's name and subtitle are measured too, so the room it keeps is the room it takes: a subtitle keeps to two
    // lines where the name's width allows, and the height counts every line it wraps to.
    for(const c of clusters){
      if(c.variant)continue;
      const titleStyle=getComputedStyle(c.title),titleText=c.title.textContent,size=parseFloat(titleStyle.fontSize)||14;
      ng.font=titleStyle.font||`italic ${size}px serif`;
      let width=Math.min(230,Math.max(c.el.dataset.tone==='group'?0:110,Math.ceil(ng.measureText(titleText).width+(parseFloat(titleStyle.letterSpacing)||0)*titleText.length)+12));
      let height=parseFloat(titleStyle.lineHeight)||size*1.25;
      if(c.subtitle.textContent){
        const subStyle=getComputedStyle(c.subtitle),spacing=parseFloat(subStyle.letterSpacing)||0,upper=subStyle.textTransform==='uppercase';
        ng.font=subStyle.font||'8px sans-serif';
        const words=(upper?c.subtitle.textContent.toLocaleUpperCase():c.subtitle.textContent).split(/\s+/).filter(Boolean),wide=word=>ng.measureText(word).width+spacing*word.length,space=wide(' ');
        width=Math.min(230,Math.max(width,Math.ceil((words.reduce((sum,word)=>sum+wide(word),0)+space*(words.length-1))/2)+12));
        let lines=1,line=0;for(const word of words){const next=line?line+space+wide(word):wide(word);if(line&&next>width-4){lines++;line=wide(word);}else line=next;}
        height+=(parseFloat(subStyle.marginTop)||5)+lines*(parseFloat(subStyle.lineHeight)||12.8);
      }
      c.labelWidth=width;c.labelHeight=Math.ceil(height)+4;
    }
    for(const e of edges){
      const style=getComputedStyle(e.el),lineHeight=parseFloat(style.lineHeight)||16;ng.font=style.font||'11px sans-serif';
      // A name given a narrower measure wraps within it, and is as tall as its lines.
      // A verb kept to one line (where rows are close) ends in an ellipsis; its whole text is in its hint and its card.
      if(!e.tag&&e.labelOneLine){const text=ng.measureText(e.el.textContent).width,limit=Number.isFinite(e.labelMax)?e.labelMax:190;e.labelWidth=Math.min(limit,Math.max(56,text+20));e.labelHeight=Math.max(22,lineHeight+8);continue;}
      if(!e.tag){const text=ng.measureText(e.el.textContent).width,limit=Number.isFinite(e.labelMax)?e.labelMax:190;e.labelWidth=Math.min(limit,Math.max(56,text+20));e.labelHeight=Math.max(24,lineHeight*Math.max(1,Math.min(3,lineCount(e.el.textContent,e.labelWidth-18)))+10);continue;}
      // A typed link shows its kind on its own line above the authored label.
      const text=ng.measureText(typeof e.label==='string'?e.label:'').width,word=ng.measureText(e.tag).width;
      e.labelWidth=Math.min(230,Math.max(70,Math.max(text,word)+22));e.labelHeight=lineHeight*(1+(text+22>230?2:1))+12;
      // Without room for the full label, the kind alone still names the line.
      ng.font=`600 10px ${style.fontFamily||'sans-serif'}`;e.compactWidth=Math.ceil(ng.measureText(e.tag.toLocaleUpperCase()).width*1.12)+18;e.compactHeight=21;
    }
  }
  function strokeSegment(a,b,pattern='solid',from=0,to=Infinity){
    const dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);if(length<1)return;to=Math.min(to,length);if(to<=from)return;
    // A dotted link is a row of round dots, evenly spaced from end to end.
    if(pattern==='dotted'){
      const r=Math.max(.6,ctx.lineWidth*.7),count=Math.max(1,Math.round(length/Math.max(4.6,r*6))),step=length/count;ctx.fillStyle=ctx.strokeStyle;ctx.beginPath();
      for(let i=0;i<count;i++){const at=(i+.5)*step;if(at<from||at>to)continue;const x=a.x+dx*at/length,y=a.y+dy*at/length;ctx.moveTo(x+r,y);ctx.arc(x,y,r,0,Math.PI*2);}
      ctx.fill();return;
    }
    const [on,period]=pattern===true||pattern==='dashed'?[6,10.5]:[0,0];
    ctx.beginPath();
    if(on){for(let start=0;start<to;start+=period){const begin=Math.max(start,from),end=Math.min(start+on,to);if(end<=begin)continue;ctx.moveTo(a.x+dx*begin/length,a.y+dy*begin/length);ctx.lineTo(a.x+dx*end/length,a.y+dy*end/length);}}
    else{ctx.moveTo(a.x+dx*from/length,a.y+dy*from/length);ctx.lineTo(a.x+dx*to/length,a.y+dy*to/length);}ctx.stroke();
  }
  // A stretch of a route, measured along it from its first point.
  function strokePath(path,pattern,from=0,to=Infinity){
    let at=0;
    for(let i=1;i<path.length;i++){const a=path[i-1],b=path[i],length=Math.hypot(b.x-a.x,b.y-a.y);if(to>at&&from<at+length)strokeSegment(a,b,pattern,Math.max(0,from-at),to-at);at+=length;}
  }
  const routeLength=path=>{let sum=0;for(let i=1;i<path.length;i++)sum+=Math.hypot(path[i].x-path[i-1].x,path[i].y-path[i-1].y);return sum;};
  function pointAlong(path,distance){
    let at=0;
    for(let i=1;i<path.length;i++){const a=path[i-1],b=path[i],length=Math.hypot(b.x-a.x,b.y-a.y);if(at+length>=distance||i===path.length-1){const t=length?clamp((distance-at)/length,0,1):0;return {x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t};}at+=length;}
    return path[0];
  }
  // A link's head, smaller on a short link so that the line still shows behind it.
  function arrow(a,b,offset=13){
    const dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);if(length<offset+12)return;
    const ux=dx/length,uy=dy/length,x=b.x-ux*offset,y=b.y-uy*offset,size=Math.min(7,(length-offset)*.36),wing=size*.52;
    ctx.beginPath();ctx.moveTo(x-ux*size-uy*wing,y-uy*size+ux*wing);ctx.lineTo(x,y);ctx.lineTo(x-ux*size+uy*wing,y-uy*size-ux*wing);ctx.stroke();
  }
  // How large a star is drawn, and so how much room a link leaves round it: a pointed or chosen star grows to its full
  // size as it warms.
  function starRadius(n){const rest=n.major?4.3:2.8;return (rest+(5.2-rest)*(n.warm??0))*Math.max(.7,Math.min(1.5,n.screen?.s??1));}
  const gapOf=n=>n?starRadius(n)+(n.form==='source'?2.6:3.5):0;
  // A route with its first point moved off its star by one gap and its last by the other; nothing where the stars touch.
  function trimRoute(route,gapA,gapB){
    const first=route[0],second=route[1],last=route[route.length-1],before=route[route.length-2];
    const lengthA=Math.hypot(second.x-first.x,second.y-first.y),lengthB=Math.hypot(last.x-before.x,last.y-before.y);
    if(route.length===2&&lengthA<=gapA+gapB+2)return null;if(lengthA<=gapA||lengthB<=gapB)return null;
    const start={...first,x:first.x+(second.x-first.x)*gapA/lengthA,y:first.y+(second.y-first.y)*gapA/lengthA};
    const end={...last,x:last.x+(before.x-last.x)*gapB/lengthB,y:last.y+(before.y-last.y)*gapB/lengthB};
    return [start,...route.slice(1,-1),end];
  }
  function hitEdge(x,y){
    if(!inView(box(x,y,0,0)))return null;
    let closest=null,best=8;
    for(const edge of edgeHits){const {a,b}=edge,dx=b.x-a.x,dy=b.y-a.y,d=dx*dx+dy*dy;if(d<1)continue;const t=clamp(((x-a.x)*dx+(y-a.y)*dy)/d,0,1);if(t<.07||t>.93)continue;const distance=Math.hypot(x-a.x-dx*t,y-a.y-dy*t);if(distance<best){closest=edge.id;best=distance;}}
    return closest;
  }
  const overlapArea=(a,b)=>Math.max(0,Math.min(a.right,b.right)-Math.max(a.left,b.left))*Math.max(0,Math.min(a.bottom,b.bottom)-Math.max(a.top,b.top));
  // A box moved the least way into the free area.
  const intoView=r=>{const width=r.right-r.left,height=r.bottom-r.top,v=targetViewport;return box(clamp(r.left,v.left,Math.max(v.left,v.right-width)),clamp(r.top,v.top,Math.max(v.top,v.bottom-height)),width,height);};
  // Of the given places, the first clear one; else the one that covers the least of what is already named, never a panel.
  function placeAlways(possibilities,occupied,stars,ownId=null,guarded=[]){
    const within=possibilities.map(intoView),clear=within.find(rect=>!occupied.some(o=>overlaps(rect,o))&&!stars.some(o=>o.id!==ownId&&overlaps(rect,o,2)));
    if(clear)return {area:clear,clear:true};
    let best=null,cost=Infinity;
    // The chosen star's own name is guarded: covering it costs as much as covering a panel.
    for(const rect of within){const covered=occupied.reduce((sum,o)=>sum+overlapArea(rect,o),0)+uiObstacles.reduce((sum,o)=>sum+overlapArea(rect,o)*20,0)+guarded.reduce((sum,o)=>sum+overlapArea(rect,o)*20,0)+stars.reduce((sum,o)=>sum+(o.id===ownId?0:overlapArea(rect,o)*.3),0);if(covered<cost){best=rect;cost=covered;}}
    return best?{area:best,clear:false}:null;
  }
  // A name comes and goes softly. A new one stands once it has had its room for a moment, so that a turn or a zoom
  // passing over it does not make it flicker; a name that leaves fades where it stood, and one that moves to another of
  // its places slides there. A name already standing keeps its place while that place stays clear.
  const APPEAR_MS=110,GLIDE='opacity .25s,color .25s,left .24s cubic-bezier(.2,.7,.2,1),top .24s cubic-bezier(.2,.7,.2,1)';
  // A group's name that yields stands over at most this many stars.
  const GROUP_CROWD=3;
  let labelsSettling=false;
  function presence(item,want,now,dt,instant){
    if(want){if(item.wantSince==null)item.wantSince=now;if(instant||item.fade==null||item.present||now-item.wantSince>=APPEAR_MS)item.present=true;}
    else{item.wantSince=null;item.present=false;}
    const target=item.present?1:0;
    item.fade=item.fade==null||instant?target:item.fade+(target-item.fade)*(1-Math.exp(-dt*14));
    if(Math.abs(item.fade-target)<.02)item.fade=target;
    if(item.fade!==target||(want&&!item.present))labelsSettling=true;
    return item.fade;
  }
  function labelsForScene(near,focusEdges,kept=new Set(),keptEdges=new Set(),now=performance.now(),dt=1/36,instant=false){
    measure();const occupied=[...uiObstacles];labelsSettling=false;
    // Cluster names orient the atlas before the smaller labels compete for room.
    // A scale stays readable at any zoom: its eras, ticks and the chosen move's span keep to rows of a strip
    // above the map at the x of their value, its rows' names to a strip at its left, and its rules and rows'
    // threads lie under the stars. Its labels give way only to one another; a band's name keeps to the band in view.
    const scaleLabels=[],vl=targetViewport.left,vr=targetViewport.right,vt=targetViewport.top,vb=targetViewport.bottom;
    ringLayer.style.clipPath=`inset(${vt.toFixed(1)}px ${Math.max(0,w-vr).toFixed(1)}px ${Math.max(0,h-vb).toFixed(1)}px ${vl.toFixed(1)}px)`;
    for(const c of clusters.filter(c=>c.variant).sort((a,b)=>(b.variant==='span')-(a.variant==='span'))){
      const p=project(c.p),xs=c.span?c.span.map(x=>project([x,c.p[1],c.p[2]]).x).sort((a,b)=>a-b):null;
      if(c.variant==='ring'){
        // A ring keeps its radius in the layout's units, so it grows and shrinks with the zoom.
        const r=Math.abs(project([c.p[0]+(c.radius??0),c.p[1],c.p[2]]).x-p.x),shown=r>=2;
        c.el.hidden=!shown;
        if(shown)Object.assign(c.el.style,{width:(2*r).toFixed(1)+'px',height:(2*r).toFixed(1)+'px',transform:`translate3d(${(p.x-r).toFixed(2)}px,${(p.y-r).toFixed(2)}px,0)`});
        continue;
      }
      if(c.variant==='hrule'){
        const shown=p.y>=vt-1&&p.y<=vb+1;c.el.hidden=!shown;
        if(shown)Object.assign(c.el.style,{width:Math.max(0,vr-vl).toFixed(1)+'px',height:'1px',transform:`translate3d(${vl.toFixed(2)}px,${p.y.toFixed(2)}px,0)`});
        continue;
      }
      if(c.variant==='facet'){
        // A header narrows to the room between its stars and the edge of the free area; its name ends in an ellipsis there.
        const room=c.align==='end'?p.x-14-vl:vr-(p.x+14),width=Math.max(56,Math.min(c.maxWidth??250,Math.ceil(c.textWidth??c.title.textContent.length*7)+48,room)),height=24;
        // Headers too close to stand apart give way to the one above, as the area is too low for them all.
        const area=box(c.align==='end'?p.x-width-14:p.x+14,p.y-height/2,width,height),shown=inView(area)&&!uiObstacles.some(o=>overlaps(area,o,0))&&!occupied.some(o=>overlaps(area,o,1));
        c.el.hidden=!shown;
        if(shown){c.el.style.width=width+'px';c.el.style.transform=`translate3d(${area.left.toFixed(2)}px,${area.top.toFixed(2)}px,0)`;occupied.push(area);}
        continue;
      }
      if(underStars.has(c.variant)){
        const rule=c.variant==='rule',from=rule?p.x:Math.max(vl,xs?.[0]??p.x),to=rule?p.x:Math.min(vr,xs?.[1]??p.x),top=rule?vt-(c.strip??4):p.y;
        const shown=rule?p.x>=vl-1&&p.x<=vr+1:to-from>=2&&p.y>=vt-2&&p.y<=vb+2;
        c.el.hidden=!shown;
        if(shown)Object.assign(c.el.style,{width:(rule?1:to-from).toFixed(1)+'px',height:(rule?Math.max(0,vb-top):1).toFixed(1)+'px',transform:`translate3d(${from.toFixed(2)}px,${top.toFixed(2)}px,0)`});
        continue;
      }
      const height=16,width=Math.min(c.maxWidth??260,Math.max(24,Math.ceil(c.textWidth??c.title.textContent.length*7.4)+(c.variant==='lane'?4:12)));
      let area,text;
      if(c.variant==='lane'){const right=vl-14,left=Math.max(4,right-width);area=text=box(left,p.y-height/2,right-left,height);}
      // A name at the edge of the free area, at the height its point has on the screen: it stays in view however the space turns.
      else if(c.variant==='edge'){const y=project([center[0],c.p[1],center[2]]).y;area=text=box(vl+8,y-height/2,width,height);}
      else if(c.variant==='ytick'){const right=vl-6,left=Math.max(4,right-width);area=text=box(left,p.y-height/2,right-left,height);}
      else if(c.variant==='place'||c.variant==='tick'){
        // A name at a point of the plane: a place beside its dot — to the right, else to the left, else above or below
        // without its dot — and a tick centred on its mark; neither leaves the free area nor covers another name.
        const wide=width+(c.variant==='place'?10:0);
        const candidates=c.variant==='place'?[['right',p.x-3,p.y-height/2],['left',p.x-wide+3,p.y-height/2],['above',p.x-width/2,p.y-height-12],['below',p.x-width/2,p.y+12]]
          :[['',c.align==='end'?p.x-width-6:c.align==='start'?p.x+6:p.x-width/2,p.y-height/2]];
        let chosen=null;
        for(const [side,left,top] of candidates){const candidate=box(left,top,side==='above'||side==='below'?width:wide,height);if(inView(candidate)&&!scaleLabels.some(o=>overlaps(candidate,o,2))&&!uiObstacles.some(o=>overlaps(candidate,o,0))){chosen={side,area:candidate};break;}}
        c.el.hidden=!chosen;
        if(chosen){area=chosen.area;if(chosen.side)c.el.dataset.side=chosen.side;c.el.style.width=(area.right-area.left).toFixed(1)+'px';c.el.style.transform=`translate3d(${area.left.toFixed(2)}px,${area.top.toFixed(2)}px,0)`;scaleLabels.push(area);occupied.push(area);}
        continue;
      }
      else{
        if(xs&&(xs[1]<vl-8||xs[0]>vr+8)){c.el.hidden=true;continue;}
        // A band keeps a visible width however short its span; its name keeps to the part in view.
        let band=xs?[Math.max(xs[0],vl-8),Math.min(xs[1],vr+8)]:null;
        if(band&&band[1]-band[0]<12){const mid=(band[0]+band[1])/2;band=[mid-6,mid+6];}
        let middle=band?(band[0]+band[1])/2:c.align==='end'?p.x-8-width/2:c.align==='start'?p.x+8+width/2:p.x;
        if(band&&band[1]-band[0]>width)middle=clamp(middle,band[0]+width/2,band[1]-width/2);
        // A name at the edge of the map moves in by up to half its width rather than disappear.
        const low=vl-12+width/2,high=vr+12-width/2;if(middle<low&&low-middle<=width/2)middle=low;else if(middle>high&&middle-high<=width/2)middle=high;
        const top=vt-(c.strip??22);text=box(middle-width/2,top,width,height);
        const left=Math.min(text.left,band?.[0]??Infinity),right=Math.max(text.right,band?.[1]??-Infinity);
        area=box(left,top,right-left,height+(band?6:0));
        c.el.style.setProperty('--text-left',(text.left-left).toFixed(1)+'px');c.el.style.setProperty('--text-width',width.toFixed(1)+'px');
        c.el.style.setProperty('--band-left',band?(band[0]-left).toFixed(1)+'px':'0px');c.el.style.setProperty('--band-width',band?(band[1]-band[0]).toFixed(1)+'px':'0px');
        if(band)c.el.dataset.band='true';else delete c.el.dataset.band;
      }
      // A name at the edge of the view needs a view wide enough to leave the stars their room beside it.
      const fits=c.variant==='lane'||c.variant==='ytick'||c.variant==='edge'?area.right-area.left>=24&&area.top>=vt-2&&area.bottom<=vb+2&&(c.variant!=='edge'||vr-vl>=600):text.left>=vl-12&&text.right<=vr+12;
      // A scale never writes over the page's own panels, even when the map is squeezed under a sheet.
      const shown=fits&&scaleLabelClear(area,scaleLabels,uiObstacles),fade=presence(c,shown,now,dt,instant||c.variant==='axis');
      c.el.hidden=!fade;
      if(fade){c.el.style.width=(area.right-area.left).toFixed(1)+'px';c.el.style.transform=`translate3d(${area.left.toFixed(2)}px,${area.top.toFixed(2)}px,0)`;c.el.style.filter=fade<1?`opacity(${fade.toFixed(3)})`:'';}
      if(shown){scaleLabels.push(area);occupied.push(area);}
    }
    // A star the view has stepped back lies under the names in front of it, and so does one a chosen star or link has
    // sent back.
    const nodeObstacles=nodes.filter(n=>!n.el.hidden&&!n.dimmed&&n.el.dataset.outside!=='true').map(n=>({id:n.id,...box(n.screen.x-9,n.screen.y-9,18,18)}));
    // The names of groups nearer the eye take their places first, as nearer stars do.
    const groups=clusters.filter(c=>!c.variant).map(c=>({c,d:project(c.p).d})).sort((a,b)=>b.d-a.d).map(item=>item.c);
    for(const c of groups){
      const width=c.labelWidth??110,height=c.labelHeight??(c.subtitle.textContent?40:25);
      const places=[-101,-76,106,-136,140].map((offset,slot)=>{const p=project([c.p[0],c.p[1]+offset,c.p[2]]),top=offset===-101?Math.max(targetViewport.top+4,p.y-20):p.y-20;return Object.assign(box(p.x-width/2,top,width,height),{slot});});
      const sticky=c.present?c.slot:null,candidates=places.filter(inView).sort((a,b)=>(b.slot===sticky)-(a.slot===sticky));
      // A group's name keeps the place it stands in while that stays clear; else it takes the first place clear of the
      // stars, and never one over another name. With no place clear of stars it takes the one over the fewest; a name
      // that yields, such as a route's among many, waits for a nearer view rather than lie over a crowd of stars.
      const clear=candidates.find(candidate=>!occupied.some(o=>overlaps(candidate,o))&&!nodeObstacles.some(o=>overlaps(candidate,o,2)));
      const covered=candidate=>nodeObstacles.reduce((sum,o)=>sum+(overlaps(candidate,o,0)?1:0),0);
      const least=clear?null:candidates.filter(candidate=>!occupied.some(o=>overlaps(candidate,o))).map(candidate=>({candidate,under:covered(candidate)})).sort((a,b)=>a.under-b.under)[0]??null;
      const area=clear??(least&&(!c.yields||least.under<=(c.present?GROUP_CROWD+2:GROUP_CROWD))?least.candidate:null);
      if(area&&!clear)c.el.dataset.crowded='true';else delete c.el.dataset.crowded;
      // A group that recedes is quieter further back in a space read in depth, as its stars are: its letters fade, and
      // the ground under them stays whole, so the lines behind never cut through a far name.
      if(depthCue&&!measured&&c.recede)c.el.style.setProperty('--depth-alpha',(.38+.62*clamp((project(c.p).d-.86)/.3,0,1)).toFixed(2));else c.el.style.removeProperty('--depth-alpha');
      const fade=presence(c,!!area,now,dt,instant);
      if(area&&c.at&&c.slot!==area.slot&&fade>0&&!instant)c.slide={x:c.at.left+(c.slide?.x??0)-area.left,y:c.at.top+(c.slide?.y??0)-area.top};
      if(area){c.slot=area.slot;occupied.push(area);}
      const at=area??places[c.slot]??null;c.at=at;
      if(c.slide){const k=Math.exp(-dt*12);c.slide.x*=k;c.slide.y*=k;if(Math.abs(c.slide.x)+Math.abs(c.slide.y)<.6)c.slide=null;else labelsSettling=true;}
      c.el.hidden=!fade||!at;
      if(at&&fade){c.el.style.width=width+'px';c.el.style.transform=`translate3d(${(at.left+(c.slide?.x??0)).toFixed(2)}px,${(at.top+(c.slide?.y??0)).toFixed(2)}px,0)`;c.el.style.filter=fade<1?`opacity(${fade.toFixed(3)})`:'';}
    }
    // The chosen star, the stars it links to and the links between them are always named. They take their places
    // first, their links' names next, and only then do the other names look for the room that is left.
    const priority=n=>n.id===selectedId?100:kept.has(n.id)?95:n.id===hover?90:n.prominent?80:near.has(n.id)?70:n.major?50:10;
    const showLabel=(n,chosen,short,full=false)=>{
      const {x,y}=n.screen;n.el.dataset.short=String(short);const visible=!!chosen&&!n.el.hidden;
      // A whole name wraps over up to three lines; another keeps its own clamp.
      if(full!==n.fullShown){n.fullShown=full;const target=n.subtitle&&n.titleEl?n.titleEl:n.label;Object.assign(target.style,full?{whiteSpace:'normal',display:'-webkit-box',webkitBoxOrient:'vertical',webkitLineClamp:'3',overflow:'hidden'}:{...n.clampStyle,overflow:'',...(n.subtitle&&n.titleEl?{display:n.labelLines>1?'-webkit-box':'block',overflow:'hidden'}:{})});}
      n.labelBox=visible?chosen:null;n.el.dataset.quiet=String(!visible);n.label.style.opacity=visible?'1':'0';n.label.style.pointerEvents=visible?'auto':'none';
      // A name already standing slides to its new side; a name coming into view takes its place before it shows.
      if(chosen){const glide=!!n.labelShown&&visible&&!instant;if(glide!==n.labelGlide){n.labelGlide=glide;n.label.style.transition=glide?GLIDE:'';}}
      n.labelShown=visible;n.labelSeen=true;if(visible){n.labelSideNow=chosen.side??null;n.wantSince=null;}
      if(chosen){const fit=chosen.right-chosen.left;Object.assign(n.label.style,{left:(chosen.left-x+22)+'px',top:(chosen.top-y+22)+'px',bottom:'auto',transform:'none',width:fit+'px',maxWidth:fit+'px',minWidth:'0'});if(visible)occupied.push(chosen);}
    };
    // Below, above, right and left of the star, and further out for a name that must be shown.
    // A star in an aspect's row names itself on its outer side first, the chosen star above itself.
    const around=(n,height,gap=19,width=n.labelWidth)=>{
      const {x,y}=n.screen,narrow=Math.max(90,width-28);
      const below=box(x-width/2,y+gap,width,height),above=box(x-width/2,y-height-gap,width,height),right=box(x+gap,y-height/2,width,height),left=box(x-width-gap,y-height/2,width,height);
      const narrowRight=narrow<width?[box(x+gap,y-height/2,narrow,height)]:[],narrowLeft=narrow<width?[box(x-narrow-gap,y-height/2,narrow,height)]:[];
      below.side='below';above.side='above';right.side='right';left.side='left';for(const r of narrowRight)r.side='right';for(const r of narrowLeft)r.side='left';
      if(n.labelSide==='right')return [right,...narrowRight,below,above,left];
      if(n.labelSide==='left')return [left,...narrowLeft,below,above,right];
      if(n.labelSide==='above')return [above,below,right,left,...narrowRight,...narrowLeft];
      return [below,above,right,left,...narrowRight,...narrowLeft];
    };
    const visibleEdges=new Set();
    const showEdge=(e,chosen,compact,hot)=>{visibleEdges.add(e.id);e.el.dataset.active=String(hot);e.el.dataset.kind=e.kind;e.el.dataset.compact=String(compact);e.el.style.setProperty('--edge-color',e.color);e.el.style.color=e.color;e.el.style.width=(compact?e.compactWidth:e.labelWidth)+'px';e.el.style.transform=`translate3d(${chosen.left.toFixed(2)}px,${chosen.top.toFixed(2)}px,0)`;if(nativeTitles)e.el.title=e.el.getAttribute('aria-label');occupied.push(chosen);};
    // Within its rank a name already standing goes first, so that it keeps its room.
    // Among equals the nearer star names itself first.
    const rank=n=>priority(n)+(n.labelShown?4:0),ordered=[...nodes].sort((a,b)=>rank(b)-rank(a)||(b.screen?.d??1)-(a.screen?.d??1));
    for(const n of ordered){
      if(!kept.has(n.id)||n.el.hidden)continue;
      // A kept name keeps its source line where there is free room for it, else its name alone where that is free;
      // with no free room at all it takes the place that covers least.
      const width=n.fullWidth??n.labelWidth,height=n.fullHeight??n.labelHeight,titleHeight=n.fullTitleHeight??n.titleHeight;
      const whole=placeAlways([...around(n,height,19,width),...around(n,height,40,width)],occupied,nodeObstacles,n.id);
      let chosen=whole?.area??null,short=false;
      if(whole&&!whole.clear&&n.subtitle&&titleHeight){const brief=placeAlways([...around(n,titleHeight,19,width),...around(n,titleHeight,40,width)],occupied,nodeObstacles,n.id);if(brief?.clear){chosen=brief.area;short=true;}}
      showLabel(n,chosen,short,true);
    }
    // An aspect's names take their own rows first; a name whose row is taken, and every other kept name, looks along its link after.
    const deferred=new Set();
    const placeKept=(e,rowOnly,roomOnly=false)=>{
      const hit=edgeHits.find(item=>item.id===e.id);if(!hit)return;
      // The part of the link inside the free area carries its whole name, however short the link is.
      const cut=clipSegment(hit.a,hit.b);if(!cut)return;
      const [a,b]=cut,dx=b.x-a.x,dy=b.y-a.y,length=Math.max(1,Math.hypot(dx,dy)),hot=e.id===selectedEdgeId||e.id===hoverEdge,spots=[];
      // A name rides on its link. A link too short to show past its name has the name beside it first, across or along
      // the screen, whichever clears the line by the smaller step.
      const along=(width,height)=>{
        const list=[],put=(cx,cy)=>list.push(box(cx-width/2,cy-height/2,width,height));
        if(Math.abs(dx)/length*width+Math.abs(dy)/length*height>length*.6){
          const sx=Math.abs(dy)>1?width/2+8+Math.abs(dx/dy)*height/2:Infinity,sy=Math.abs(dx)>1?height/2+8+Math.abs(dy/dx)*width/2:Infinity;
          // The nearer way first, each a little further out if the first place is taken; the other way only if not much further.
          const near=Math.min(sx,sy),ways=[[1,0,sx],[0,1,sy]].filter(w=>w[2]<=near*1.6).sort((p,q)=>p[2]-q[2]);
          for(const [ux,uy,reach] of ways)for(const more of [0,12,24])for(const sign of [1,-1])for(const t of [.5,.38,.62])put(a.x+dx*t+sign*ux*(reach+more),a.y+dy*t+sign*uy*(reach+more));
        }
        for(const offset of [0,16,-16,32,-32,48,-48,64,-64])for(const t of [.5,.38,.62,.26,.74,.15,.85])put(a.x+dx*t-dy/length*offset,a.y+dy*t+dx/length*offset);
        return list;
      };
      // Where links crowd round the chosen star and no free place is left for a whole name, a free place for the link's
      // kind is better than names laid over one another; the whole name stays in its title.
      const guarded=[nodes.find(n=>n.id===selectedId)?.labelBox].filter(Boolean);
      // In an aspect a link's name stands in the row of the star it reaches, beside that star, or over the chosen link.
      const pinned=(width,height)=>{
        const at=e.labelAt;if(!at)return [];
        if(at.mid){const mx=(a.x+b.x)/2,my=(a.y+b.y)/2;return [at.align==='start'?box(mx+12,my-height/2,width,height):box(mx-width/2,my-height-6,width,height)];}
        const star=nodes.find(n=>n.id===at.id);if(!star||star.el.hidden)return [];
        const {x}=star.screen,y=Number.isFinite(at.dy)?project([star.p[0],star.p[1]+at.dy,star.p[2]]).y:star.screen.y;
        return [box(at.align==='end'?x-width-14:at.align==='start'?x+14:x-width/2,y-height/2,width,height)];
      };
      if(rowOnly){
        // In its row a name stands whole, or as its kind alone; a taken row waits for the second pass.
        const whole=placeAlways(pinned(e.labelWidth,e.labelHeight),occupied,nodeObstacles,null,guarded);
        if(whole?.clear){showEdge(e,whole.area,false,hot);return;}
        const brief=e.tag&&e.compactWidth?placeAlways(pinned(e.compactWidth,e.compactHeight),occupied,nodeObstacles,null,guarded):null;
        if(brief?.clear){showEdge(e,brief.area,true,hot);return;}
        deferred.add(e.id);return;
      }
      let spot=placeAlways([...pinned(e.labelWidth,e.labelHeight),...along(e.labelWidth,e.labelHeight)],occupied,nodeObstacles,null,guarded),compact=false;
      // With no free place for either, the smaller kind-only label covers the least.
      if(spot&&!spot.clear&&e.tag&&e.compactWidth){const brief=placeAlways([...pinned(e.compactWidth,e.compactHeight),...along(e.compactWidth,e.compactHeight)],occupied,nodeObstacles,null,guarded);if(brief){spot=brief;compact=true;}}
      if(!spot||(roomOnly&&!spot.clear))return false;
      showEdge(e,spot.area,compact,hot);return true;
    };
    for(const e of edges)if(keptEdges.has(e.id)&&!e.muted&&e.labelAt)placeKept(e,true);
    // Round the chosen star each name its links carry stands once, the first four of them where they cover least; every
    // further name stands only where it has room of its own, and no more than ten stand in all. The panel lists every
    // link; a pointed link always names itself.
    const keptNames=new Set();let keptShown=0,keptForced=0;
    for(const e of edges){
      if(!keptEdges.has(e.id)||e.muted||(e.labelAt&&!deferred.has(e.id)))continue;
      // In a space read in depth a chosen star's links keep to their colours and lines; a link names itself when pointed at.
      if(depthCue&&!measured&&e.id!==selectedEdgeId&&e.id!==hoverEdge)continue;
      const hot=e.id===selectedEdgeId||e.id===hoverEdge,name=`${e.from===selectedId?e.from:e.to===selectedId?e.to:''}|${e.el.textContent}`,again=keptNames.has(name);
      if(!hot&&keptShown>=10)continue;
      const forced=!again&&keptForced<4;
      if(placeKept(e,false,!forced&&!hot)){keptNames.add(name);keptShown++;if(forced)keptForced++;}
    }
    for(const n of ordered){
      if(kept.has(n.id)&&!n.el.hidden)continue;
      const hot=n.id===selectedId||n.id===hover;
      if(focusMix>.3&&!hot){showLabel(n,null,false);continue;}
      // A star the view has stepped back keeps its light and gives its name's room to the stars in front.
      if((n.dimmed||n.quiet)&&!hot){showLabel(n,null,false);continue;}
      // In a space read in depth the stars far behind give their names' room to the ones in front.
      // A name already standing waits a little longer before it goes than a new one waits to come.
      const was=!!n.labelShown;
      if(depthCue&&!measured&&n.labelFrom&&n.screen.d<(was?.87:.9)&&!hot&&!near.has(n.id)){showLabel(n,null,false);continue;}
      // A name that waits for a nearer view stands once the view comes that near (`labelFrom` counts the view's zoom
      // over the whole, a nearer star counting nearer), or while its star is reached.
      if(n.labelFrom&&zoom*n.screen.d<n.labelFrom*(was?.9:1)&&!hot&&!near.has(n.id)){showLabel(n,null,false);continue;}
      const place=height=>{
        // A name that stands keeps its side while that side stays nearly clear: a star passing under it, or another
        // name grazing its edge, does not move it.
        const within=around(n,height).filter(inView).sort((a,b)=>was?(b.side===n.labelSideNow)-(a.side===n.labelSideNow):0);
        const own=was?within.find(rect=>rect.side===n.labelSideNow):null,size=rect=>(rect.right-rect.left)*(rect.bottom-rect.top);
        if(own&&!occupied.some(o=>overlapArea(own,o)>size(own)*.12))return own;
        const clear=within.find(rect=>!occupied.some(o=>overlaps(rect,o))&&!nodeObstacles.some(o=>o.id!==n.id&&overlaps(rect,o,2)));
        return clear||(hot?within[0]:n.prominent?within.find(rect=>!occupied.some(o=>overlaps(rect,o))):null);
      };
      // Without room for its source line, a star keeps its name rather than losing both.
      let chosen=place(n.labelHeight),short=false;
      if(!chosen&&n.subtitle&&n.titleHeight){chosen=place(n.titleHeight);short=!!chosen;}
      // A new name stands once it has had its room for a moment; meanwhile that room is kept for it.
      if(!chosen)n.wantSince=null;
      else if(!was&&!hot&&!n.prominent&&!near.has(n.id)&&n.labelSeen&&!instant){if(n.wantSince==null)n.wantSince=now;if(now-n.wantSince<APPEAR_MS){occupied.push(chosen);labelsSettling=true;showLabel(n,null,false);continue;}}
      showLabel(n,chosen,short);
    }
    // In a space read in depth only a pointed or chosen link names itself; the star's other links keep to their colours.
    const candidates=edges.filter(e=>!e.muted&&!visibleEdges.has(e.id)&&(e.id===selectedEdgeId||e.id===hoverEdge||(!(depthCue&&!measured)&&(hover||selectedId)&&focusEdges.has(e.id)))).sort((a,b)=>(b.id===hoverEdge?100:b.id===selectedEdgeId?90:b.highlight?70:0)-(a.id===hoverEdge?100:a.id===selectedEdgeId?90:a.highlight?70:0));
    let shown=0;
    for(const e of candidates){
      if(shown>=6)break;const hit=edgeHits.find(hit=>hit.id===e.id);if(!hit)continue;
      const {a,b}=hit,dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy),hot=e.id===hoverEdge||e.id===selectedEdgeId;if(length<70&&!hot)continue;
      let chosen=null,compact=false;
      for(const t of [.5,.35,.65]){const x=a.x+dx*t,y=a.y+dy*t,area=box(x-e.labelWidth/2,y-e.labelHeight/2,e.labelWidth,e.labelHeight);if(inView(area)&&!occupied.some(o=>overlaps(area,o))&&!nodeObstacles.some(o=>overlaps(area,o))){chosen=area;break;}}
      if(!chosen&&e.tag&&e.compactWidth)for(const t of [.5,.4,.6,.3,.7]){const x=a.x+dx*t,y=a.y+dy*t,area=box(x-e.compactWidth/2,y-e.compactHeight/2,e.compactWidth,e.compactHeight);if(inView(area)&&!occupied.some(o=>overlaps(area,o,2))&&!nodeObstacles.some(o=>overlaps(area,o,2))){chosen=area;compact=true;break;}}
      if(!chosen&&hot){for(const offset of [0,24,-24]){const area=box((a.x+b.x)/2-dy/Math.max(1,length)*offset-e.labelWidth/2,(a.y+b.y)/2+dx/Math.max(1,length)*offset-e.labelHeight/2,e.labelWidth,e.labelHeight);if(inView(area)&&!uiObstacles.some(o=>overlaps(area,o))&&!nodeObstacles.some(o=>overlaps(area,o))&&!nodes.some(n=>(n.id===selectedId||n.id===hover)&&n.labelBox&&overlaps(area,n.labelBox))){chosen=area;break;}}}
      if(!chosen&&hot&&e.tag&&e.compactWidth)for(const offset of [16,-16,30,-30]){const cx=(a.x+b.x)/2-dy/Math.max(1,length)*offset,cy=(a.y+b.y)/2+dx/Math.max(1,length)*offset,area=box(cx-e.compactWidth/2,cy-e.compactHeight/2,e.compactWidth,e.compactHeight);if(inView(area)&&!uiObstacles.some(o=>overlaps(area,o))&&!nodeObstacles.some(o=>overlaps(area,o,2))){chosen=area;compact=true;break;}}
      // A hovered or chosen link always names its kind, even where the stars leave no free gap.
      if(!chosen&&hot&&e.tag&&e.compactWidth){const area=box((a.x+b.x)/2-e.compactWidth/2,(a.y+b.y)/2-e.compactHeight/2,e.compactWidth,e.compactHeight);if(inView(area)&&!uiObstacles.some(o=>overlaps(area,o)))[chosen,compact]=[area,true];}
      if(!chosen)continue;
      // A hovered link's name may cover another name, never a kept one.
      if(hot)for(const n of nodes)if(n.labelBox&&!kept.has(n.id)&&overlaps(chosen,n.labelBox)){n.labelBox=null;n.labelShown=false;n.el.dataset.quiet='true';n.label.style.opacity='0';n.label.style.pointerEvents='none';}
      showEdge(e,chosen,compact,hot);shown++;
    }
    for(const e of edges)e.el.hidden=!visibleEdges.has(e.id);
    return occupied;
  }

  // A frame that fails keeps the sky alive: the error is reported once, and the next frame is drawn as usual.
  let failed='';
  function draw(now,still=false){
    try{drawFrame(now,still);}
    catch(error){raf=0;const said=String(error?.message??error);if(said!==failed){failed=said;console.error(error);}if(!still)kick();}
  }
  function drawFrame(now,still){
    raf=0;if(disposed||(document.hidden&&!still))return;
    const frameInterval=softwareRenderer?1000/28:1000/36;if(!still&&lastFrame&&now-lastFrame<frameInterval){raf=requestAnimationFrame(draw);return;}lastFrame=now;
    const started=performance.now(),elapsed=last?(now-last)/1000:1/60,dt=Math.min(elapsed,.05),ease=(reduced.matches||still)?1:1-Math.exp(-dt*5);last=now;
    if(playing&&!still)skyTime+=dt*.60;
    if(velocity&&!drag&&!still&&!nodes.some(n=>n.turningView)){
      const nextYaw=tyaw+velocity.yaw*dt,nextPitch=clamp(tpitch+velocity.pitch*dt,...PITCH),decay=Math.exp(-dt*3.6);
      velocity.yaw=nextYaw===tyaw+velocity.yaw*dt?velocity.yaw*decay:0;velocity.pitch=nextPitch===tpitch+velocity.pitch*dt?velocity.pitch*decay:0;
      tyaw=nextYaw;tpitch=nextPitch;if(Math.abs(velocity.yaw)+Math.abs(velocity.pitch)<.015)velocity=null;
    }
    // The first view of a space turns slowly into its rest, so that its depth is seen moving once.
    if(introPending&&nodes.length&&!still){introPending=false;yaw=restYaw+(Math.sign(restYaw)||1)*.6;pitch=clamp(restPitch-(Math.sign(restPitch)||1)*.2,...PITCH);introUntil=now+2200;}
    // As the space turns, its fit follows, so that what comes round from the depth stays inside the free area.
    turnFit();
    const turnEase=now<introUntil&&!drag?1-Math.exp(-dt*1.5):ease;
    yaw+=(tyaw-yaw)*turnEase;pitch+=(tpitch-pitch)*turnEase;zoom+=(tzoom-zoom)*ease;pan.x+=(tpan.x-pan.x)*ease;pan.y+=(tpan.y-pan.y)*ease;
    let layoutMoving=false;
    const approach=(current,target)=>{if(Math.abs(current-target)>.015)layoutMoving=true;return Math.abs(current-target)<.002?target:current+(target-current)*ease;};
    center=center.map((v,i)=>approach(v,targetCenter[i]));fitScale=approach(fitScale,targetFit);stretchY=approach(stretchY,tstretchY);
    // A measured scale pans no further than its own overhang at the zoom it is heading to.
    if(measured){
      const ratio=tzoom/Math.max(.01,zoom),overhang=(a,b,room)=>Math.max(0,(Math.abs(b-a)*ratio-room)/2+24);
      const sx=overhang(project([measured.x[0],center[1],0]).x,project([measured.x[1],center[1],0]).x,targetViewport.right-targetViewport.left);tpan.x=clamp(tpan.x,-sx,sx);
      if(measured.axes==='xy'){const sy=overhang(project([center[0],measured.y[0],0]).y,project([center[0],measured.y[1],0]).y,targetViewport.bottom-targetViewport.top);tpan.y=clamp(tpan.y,-sy,sy);}else tpan.y=0;
    }
    for(const side of ['left','right','top','bottom'])viewport[side]=approach(viewport[side],targetViewport[side]);
    for(const n of nodes)if(!n.dragging)n.p=n.p.map((v,i)=>approach(v,n.target[i]));
    for(const c of clusters)c.p=c.p.map((v,i)=>approach(v,c.target[i]));
    const nextBase=Math.max(.1,Math.min((viewport.right-viewport.left)/880,(viewport.bottom-viewport.top)/540,1.7));
    if(Math.abs(base-nextBase)>.00001||frameCount===0){base=nextBase;dust.forEach(s=>{const spread=(FOCAL-s.minZ-s.range*.5)/FOCAL;s.x=s.sx*w*.5/base*spread;s.y=s.sy*h*.5/base*spread;});}
    skyCos=Math.cos(yaw+Math.sin(skyTime*.021)*.011);skySin=Math.sin(yaw+Math.sin(skyTime*.021)*.011);
    skyPitchCos=Math.cos(pitch);skyPitchSin=Math.sin(pitch);
    ctx.beginFrame?.();ctx.setTransform(dpr,0,0,dpr,0,0);ctx.globalAlpha=1;ctx.globalCompositeOperation='source-over';
    // The night's nebula is the night's own; turned over for the day it would lie on the paper as a cold stain.
    if(daylight())paintDay();else ctx.drawImage(nebula,0,0,w,h);
    const count=Math.round(dust.length*design.density);paintStars(0,count);paintFog(fogBack,.2,.85);paintStars(1,count);paintFog(fogFront,2.4,.46);
    nodes.forEach(n=>n.screen=project(n.p));
    paintShade();
    if(revealId&&!drag)revealPending();
    if(neighbourhoodPending&&!drag)revealNeighbourhood();
    const byId=new Map(nodes.map(n=>[n.id,n]));
    const selectedEdge=edges.find(e=>e.id===selectedEdgeId),hoveredEdge=edges.find(e=>e.id===hoverEdge),focusId=hover??selectedId,near=new Set(focusId&&byId.has(focusId)?[focusId]:[]),focusEdges=new Set();
    // A chosen star that this view does not show dims nothing: the space stays as it is until a star of it is chosen.
    for(const edge of [selectedEdge,hoveredEdge])if(edge){near.add(edge.from);near.add(edge.to);focusEdges.add(edge.id);}
    for(const edge of edges)if(!edge.muted&&(edge.from===focusId||edge.to===focusId)){near.add(edge.from);near.add(edge.to);focusEdges.add(edge.id);}
    // The chosen star and the chosen link keep their neighbourhood lit and named while the pointer is elsewhere.
    const kept=new Set(),keptEdges=new Set();
    if(selectedId&&byId.has(selectedId)){kept.add(selectedId);for(const edge of edges)if(!edge.muted&&(edge.from===selectedId||edge.to===selectedId)&&byId.has(edge.from)&&byId.has(edge.to)){kept.add(edge.from);kept.add(edge.to);keptEdges.add(edge.id);}}
    if(selectedEdge&&!selectedEdge.muted&&byId.has(selectedEdge.from)&&byId.has(selectedEdge.to)){kept.add(selectedEdge.from);kept.add(selectedEdge.to);keptEdges.add(selectedEdge.id);}
    // A chosen star in a space read in depth gathers the view to itself: every star outside it and its neighbours steps
    // back, nameless and out of reach, with the links and lines that do not touch it, until the star is let go. A chosen
    // link does the same for its own two stars.
    const chosenFocus=depthCue&&!measured&&(!!selectedId||!!selectedEdge)&&kept.size>0,focusTo=chosenFocus?1:0;
    focusMix=still||reduced.matches?focusTo:focusMix+(focusTo-focusMix)*(1-Math.exp(-dt*9));if(Math.abs(focusMix-focusTo)<.01)focusMix=focusTo;else layoutMoving=true;
    if(root.dataset.focus!==(chosenFocus?'chosen':''))root.dataset.focus=chosenFocus?'chosen':'';
    // An aspect names every link it stands in rows, and every star at their ends.
    for(const edge of edges)if(edge.labelAt&&!edge.muted&&byId.has(edge.from)&&byId.has(edge.to)){kept.add(edge.from);kept.add(edge.to);keptEdges.add(edge.id);}
    const focused=near.size>0;edgeHits=[];
    // The planes of depth show while the space turns, by a drag, a key, the globe or its first turn, and fade once it rests:
    // the turn itself shows what depth means, and the space at rest keeps only its stars.
    const turning=!measured&&!!((drag?.moved&&!(drag.forcePan||(rotate?drag.shift:!drag.shift)))||velocity||now<introUntil||now<turnShownUntil||nodes.some(n=>n.turningView));
    if(planes.length){reveal+=((turning?1:0)-reveal)*(1-Math.exp(-dt*(turning?7:1.8)));if(reveal<.003)reveal=0;}else reveal=0;
    for(const plane of planes){
      const z=plane.z,[x0,x1]=plane.x,[y0,y1]=plane.y;plane.corners=[[x0,y0],[x1,y0],[x1,y1],[x0,y1]].map(([x,y])=>project([x,y,z]));
      if(reveal<.02)continue;
      ctx.strokeStyle=plane.color;ctx.lineWidth=.9;ctx.globalAlpha=.3*reveal;
      for(let i=0;i<4;i++){const a=plane.corners[i],b=plane.corners[(i+1)%4];ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.stroke();}
      // Its parallels let the eye read the plane's slant.
      ctx.globalAlpha=.09*reveal;ctx.lineWidth=.7;
      for(const f of [.2,.4,.6,.8]){const a=project([x0,y0+(y1-y0)*f,z]),b=project([x1,y0+(y1-y0)*f,z]);ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.stroke();}
      ctx.globalAlpha=1;
    }
    if(backdrop.length){
      // The lines under a chosen or pointed star step back with the stars it does not reach; in a space read in depth
      // a line also dims as it goes back, as the stars do.
      const fade=(focused?.42:1)*(1-.72*focusMix);
      for(const line of backdrop){
        const points=Array.isArray(line)?line:line.points,style=Array.isArray(line)?{}:line;
        ctx.beginPath();
        let previous=null,depth=0,count=0;
        for(const [x,y,z=0] of points){const point=project([x,y,z]);depth+=point.d;count++;if(previous){const cut=measured?clipSegment(previous,point):[previous,point];if(cut){ctx.moveTo(cut[0].x,cut[0].y);ctx.lineTo(cut[1].x,cut[1].y);}}previous=point;}
        const back=depthCue&&!measured&&count?.35+.65*clamp((depth/count-.82)/.34,0,1):1;
        // In the day what lies back takes the cool of the air as well as its paleness.
        ctx.strokeStyle=daylight()&&back<1?mixHex(style.color??'#8fa8c4',night('#B3B6C4'),(1-back)*.75):style.color??'#8fa8c4';
        ctx.globalAlpha=Math.min(1,(style.alpha??.3)*fade*ink)*(daylight()&&back<1?.2+.8*back*back:back);ctx.lineWidth=Number.isFinite(style.weight)?clamp(style.weight*(previous?.s??1),.6,7):.9;ctx.stroke();
      }
      ctx.globalAlpha=1;
    }
    // A star warms when pointed at or chosen and cools when let go; its halo opens as it is chosen and its orbit follows;
    // a pointed link warms the same way, and a chosen one lights from its source to its target. Each takes a few frames.
    const settle=still||reduced.matches,warmBy=settle?1:1-Math.exp(-dt*16),haloBy=settle?1:1-Math.exp(-dt*10),orbitBy=settle?1:1-Math.exp(-dt*5.5);
    const toward=(value,target,by)=>{if(value==null)return target;const next=value+(target-value)*by;if(Math.abs(next-target)<.01)return target;layoutMoving=true;return next;};
    for(const n of nodes){const warmNow=n.id===selectedId||n.id===hover||(holdToMove>0&&n.dragging),ringed=n.id===selectedId||n.id===linkFrom;n.warm=toward(n.warm,warmNow?1:0,warmBy);n.halo=toward(n.halo,ringed?1:0,haloBy);n.orbit=toward(n.orbit,ringed?1:0,orbitBy);}
    for(const edge of edges)edge.warm=toward(edge.warm,edge.id===hoverEdge||edge.id===selectedEdgeId||edge.highlight?1:0,warmBy);
    // The light waits a moment for the view to come round to the link, then runs along it.
    const sweep=settle||measured?1:clamp((now-edgeChosenAt-220)/600,0,1),swept=sweep*sweep*(3-2*sweep);if(sweep<1)layoutMoving=true;
    for(const edge of edges){
      const from=byId.get(edge.from)?.screen,to=byId.get(edge.to)?.screen;if(!from||!to)continue;
      const hot=edge.id===hoverEdge||edge.id===selectedEdgeId||edge.highlight,neighbor=focusEdges.has(edge.id)||keptEdges.has(edge.id);
      // A latent link is drawn only while one of its stars is pointed at or chosen.
      if(edge.latent&&!hot&&!neighbor)continue;
      const awayEdge=!hot&&!keptEdges.has(edge.id)&&!focusEdges.has(edge.id)?focusMix:0;if(awayEdge>.98)continue;
      // A link may run through points of its own, so the links between distant parts of a layout gather on their way.
      const route=Array.isArray(edge.via)&&edge.via.length?[from,...edge.via.map(project),to]:[from,to];
      if(edge.muted){ctx.strokeStyle=edge.color;ctx.globalAlpha=.05;ctx.lineWidth=.6;for(let i=1;i<route.length;i++){let a=route[i-1],b=route[i];if(measured){const cut=clipSegment(a,b);if(!cut)continue;[a,b]=cut;}strokeSegment(a,b,edge.dash);}continue;}
      const rest=Number.isFinite(edge.baseAlpha)?edge.baseAlpha:.24,weight=Number.isFinite(edge.weight)?edge.weight:1;
      // In the day's air a link pales as it goes back, by the depth of its two ends; a pointed or chosen one stays clear.
      const air=depthCue&&!measured&&daylight()&&!hot?.22+.78*clamp(((from.d+to.d)/2-.84)/.34,0,1):1;
      // A link's light and weight run from its calm measure to its hot one as it warms.
      const calm=neighbor?.57:focused?rest*.23:rest,calmWidth=neighbor?1.1:.65,heat=edge.id===selectedEdgeId?1:edge.warm??(hot?1:0);
      const level=on=>Math.min(1,(calm+(.85-calm)*on)*ink)*(1-awayEdge)*air,girth=on=>(calmWidth+(1.55-calmWidth)*on)*weight*(1+(ink-1)*.35),alpha=level(heat),width=girth(heat);
      ctx.strokeStyle=air<1?mixHex(edge.color,night('#B3B6C4'),(1-air)*.8):edge.color;
      // A link leaves a little room round each of its stars, so that the stars stand clear of the lines they join.
      const path=trimRoute(route,gapOf(byId.get(edge.from)),gapOf(byId.get(edge.to)));if(!path)continue;
      const dash=edge.dash??(edge.kind==='questions'||edge.kind==='contrasts'?'dashed':'solid');let longest=null;
      // A chosen or pointed unbroken link glows: two faint strokes under its line, the broader the fainter. A broken
      // link keeps its gaps and is lit by its own marks.
      const unbroken=dash!==true&&dash!=='dashed'&&dash!=='dotted',glows=[[3.2,.06],[1.9,.1]],dayGlow=daylight()?.7:1;
      if(edge.id===selectedEdgeId&&swept<1){
        // A link just chosen lights from its source to its target; its head shows as the light arrives.
        const reach=routeLength(path)*swept;
        ctx.globalAlpha=level(0);ctx.lineWidth=girth(0);strokePath(path,dash,reach);
        if(unbroken)for(const [spread,light] of glows){ctx.globalAlpha=level(1)*light*dayGlow;ctx.lineWidth=girth(1)*spread;strokePath(path,'solid',0,reach);}
        ctx.globalAlpha=level(1);ctx.lineWidth=girth(1);strokePath(path,dash,0,reach);
        if(swept>.8){ctx.globalAlpha=level(1)*(swept-.8)/.2;arrow(path[path.length-2],path[path.length-1],1.5);}
        for(let i=1;i<path.length;i++){const a=path[i-1],b=path[i],length=Math.hypot(b.x-a.x,b.y-a.y);if(!longest||length>longest.length)longest={a,b,length};}
      }else{
        if(hot&&unbroken)for(const [spread,light] of glows){ctx.globalAlpha=alpha*light*heat*dayGlow;ctx.lineWidth=width*spread;for(let i=1;i<path.length;i++){let a=path[i-1],b=path[i];if(measured){const cut=clipSegment(a,b);if(!cut)continue;[a,b]=cut;}strokeSegment(a,b);}}
        ctx.globalAlpha=alpha;ctx.lineWidth=width;
        for(let i=1;i<path.length;i++){
          let a=path[i-1],b=path[i],whole=true;if(measured){const cut=clipSegment(a,b);if(!cut)continue;[a,b,whole]=cut;}
          strokeSegment(a,b,dash);if((hot||neighbor)&&whole&&i===path.length-1)arrow(a,b,1.5);
          const length=Math.hypot(b.x-a.x,b.y-a.y);if(!longest||length>longest.length)longest={a,b,length};
        }
        // While a link stays chosen, a spark runs along it now and then, from its source to its target.
        if(edge.id===selectedEdgeId&&playing&&!settle&&!measured){
          const t=(now/1000%3.6)/1.5;
          if(t<1){const total=routeLength(path),at=pointAlong(path,total*t*t*(3-2*t)),shine=Math.sin(Math.PI*t),spark=!daylight()&&/^#[0-9a-f]{6}$/i.test(edge.color)?mixHex(edge.color,'#ffffff',.78):edge.color;
            ctx.fillStyle=spark;ctx.globalAlpha=level(1)*.3*shine;ctx.beginPath();ctx.arc(at.x,at.y,4.4,0,Math.PI*2);ctx.fill();
            ctx.globalAlpha=Math.min(1,level(1)*1.15)*shine;ctx.beginPath();ctx.arc(at.x,at.y,2,0,Math.PI*2);ctx.fill();}
        }
      }
      if(longest)edgeHits.push({id:edge.id,a:longest.a,b:longest.b});
    }
    // A route's way lights up while its card is pointed at: a golden line through its stars in the order it walks them.
    const litTarget=lit?1:0;litGlow+=(litTarget-litGlow)*(1-Math.exp(-dt*(lit?9:4)));if(Math.abs(litTarget-litGlow)<.01)litGlow=litTarget;
    if(litGlow&&lit){
      const points=lit.map(id=>byId.get(id)?.screen).filter(Boolean);
      ctx.strokeStyle='#e8cf8f';ctx.lineWidth=1.6;ctx.globalAlpha=.7*litGlow;
      for(let i=1;i<points.length;i++){ctx.beginPath();ctx.moveTo(points[i-1].x,points[i-1].y);ctx.lineTo(points[i].x,points[i].y);ctx.stroke();}
      ctx.globalAlpha=.85*litGlow;ctx.lineWidth=1.2;for(const point of points){ctx.beginPath();for(let i=0;i<=20;i++){const angle=i/20*Math.PI*2,x=point.x+Math.cos(angle)*8,y=point.y+Math.sin(angle)*8;if(i)ctx.lineTo(x,y);else ctx.moveTo(x,y);}ctx.stroke();}
      ctx.globalAlpha=1;
    }
    // A thread grows out of its star and fades into the dark; its door sits where it fades.
    for(const thread of threads){
      const from=byId.get(thread.from);thread.grown+=(1-thread.grown)*(1-Math.exp(-dt*4));
      if(!from||from.el.hidden){thread.el.hidden=true;continue;}
      const a=from.screen,ahead=project([from.p[0]+thread.direction[0]*60,from.p[1]+thread.direction[1]*60,from.p[2]]);
      let dx=ahead.x-a.x,dy=ahead.y-a.y;const length=Math.hypot(dx,dy);if(length<.5){thread.el.hidden=true;continue;}dx/=length;dy/=length;
      const start=11,reach=start+68*thread.grown,steps=6;ctx.strokeStyle=thread.color;ctx.lineWidth=1.1;
      for(let i=0;i<steps;i++){const t0=start+(reach-start)*i/steps,t1=start+(reach-start)*(i+1)/steps;ctx.globalAlpha=.62*(1-i/steps)*thread.grown;ctx.beginPath();ctx.moveTo(a.x+dx*t0,a.y+dy*t0);ctx.lineTo(a.x+dx*t1,a.y+dy*t1);ctx.stroke();}
      const ex=a.x+dx*(reach+7),ey=a.y+dy*(reach+7);ctx.globalAlpha=.55*thread.grown;ctx.fillStyle=thread.color;ctx.beginPath();ctx.arc(ex,ey,1.8,0,Math.PI*2);ctx.fill();ctx.globalAlpha=1;
      thread.el.hidden=!inView(box(ex-4,ey-4,8,8));thread.el.style.transform=`translate3d(${(ex-12).toFixed(2)}px,${(ey-12).toFixed(2)}px,0)`;
      if(thread.grown<.995)layoutMoving=true;
    }
    nodes.forEach(n=>{
      const p=n.screen,hot=n.id===selectedId||n.id===hover||(holdToMove>0&&n.dragging),own=colorOf(n),big=n.major,dim=(focused&&root.dataset.journey!=='true'&&!near.has(n.id)&&!kept.has(n.id)||n.dimmed)&&!hot,radius=starRadius(n),warm=n.warm??(hot?1:0);
      const near01=depthCue&&!measured?clamp((p.d-.86)/.3,0,1):1,fade=hot?1:daylight()?.3+.7*near01:.5+.5*near01;
      const color=daylight()&&!hot&&near01<1?mixHex(own,night('#B3B6C4'),(1-near01)*.6):own;
      // A measured layout draws only the stars inside its free area; every star still keeps its type, so its label is measured right.
      if(n.el.dataset.major!==String(!!big)){n.el.dataset.major=String(!!big);measureLabels=true;}
      if(measured&&!inView(box(p.x-5,p.y-5,10,10))){n.el.hidden=true;return;}
      // On a light page a star's glow turns to a shadow, so there it keeps only a trace of it and its ink stays clear.
      const away=!kept.has(n.id)&&!hot?focusMix:0,starAlpha=(dim?.28:hot?1:.9)*fade*(1-.86*away),glow=((big?86:46)+(125-(big?86:46))*warm)*Math.max(.65,Math.min(1.4,p.s));
      // On a light page the glow turns to a shadow: the light falls from above, so a near star casts its shadow
      // below and aside and stands over the page; a far one lies flat in the air.
      if(daylight()){const lift=hot?1:near01,size=glow*.5,dx=1.5+4.5*lift,dy=2.5+7*lift;ctx.globalAlpha=starAlpha*(hot?.55:.06+.32*lift*lift);ctx.drawImage(dayShadow,p.x+dx-size/2,p.y+dy-size/2,size,size);}
      else{ctx.globalAlpha=starAlpha;ctx.drawImage(sprite(color),p.x-glow/2,p.y-glow/2,glow,glow);}
      ctx.globalAlpha=starAlpha;
      // A source is a disc; an authored proposal a ring; a personal record a broken ring.
      if(n.form==='source'){ctx.fillStyle=color;ctx.beginPath();ctx.arc(p.x,p.y,radius,0,Math.PI*2);ctx.fill();ctx.fillStyle='#fff9ea';ctx.beginPath();ctx.arc(p.x,p.y,Math.max(.75,radius*.32),0,Math.PI*2);ctx.fill();}
      else ring(p.x,p.y,radius+.9,color,n.form==='personal',warm);
      // Its four rays narrow and fade towards their tips, as a star's light does in the telescope; a faint star far
      // back keeps only its point.
      const restRay=big?12:6.5,restLight=dim?.12:big?.52:.34,ray=restRay+(19-restRay)*warm,rayAlpha=(restLight+(.92-restLight)*warm)*(1-.9*away);
      if(rayAlpha>.03&&!(dim&&!big)){ctx.strokeStyle=color;const from=(radius+.6)/ray;
        for(const [start,end,fade,width] of RAY_STEPS){const a=Math.max(start,from);if(a>=end)continue;ctx.globalAlpha=rayAlpha*fade;ctx.lineWidth=width*(1+.2*warm);ctx.beginPath();for(const [ux,uy] of RAY_DIRS){ctx.moveTo(p.x+ux*ray*a,p.y+uy*ray*a);ctx.lineTo(p.x+ux*ray*end,p.y+uy*ray*end);}ctx.stroke();}}
      // A chosen star is ringed by a halo of its own colour and a faint orbit beyond it. The halo opens out from the star
      // as it is chosen and the orbit follows a moment later, turning slowly while the star stays chosen. The star a link
      // is being drawn from keeps its halo broken and turning, waiting for the other end.
      const halo=n.halo??0,orbit=n.orbit??0;
      if(halo>.01||orbit>.01){
        const open=1-Math.pow(1-halo,3),wide=1-Math.pow(1-orbit,3),waiting=n.id===linkFrom&&n.id!==selectedId,turn=playing&&!reduced.matches?skyTime*(waiting?.5:.12):0;ctx.strokeStyle=color;
        if(halo>.01){ctx.lineWidth=1.1;ctx.globalAlpha=.8*halo;circle(p.x,p.y,radius+2+4.5*open,40,waiting,turn);}
        if(orbit>.01){ctx.lineWidth=.75;ctx.globalAlpha=.3*orbit;circle(p.x,p.y,radius+7+4.5*wide,56,true,turn);}
      }
      n.el.style.transform=`translate3d(${(p.x-22).toFixed(2)}px,${(p.y-22).toFixed(2)}px,0)`;
      // The nearer star lies over the farther, so that the pointer reaches the star it is over, and the chosen one over all.
      const layer=String(hot?3000:100+Math.round(clamp(p.d,0,2.4)*400));if(n.el.dataset.layer!==layer){n.el.dataset.layer=layer;n.el.style.zIndex=layer;}
      const outside=String(away>.5);if(n.el.dataset.outside!==outside){n.el.dataset.outside=outside;n.el.tabIndex=away>.5?-1:0;}
      n.el.dataset.active=String(n.id===selectedId);n.el.dataset.prominent=String(n.prominent);n.el.dataset.dim=String(dim);n.el.style.opacity=dim?'.35':fade<1?(.62+.38*near01).toFixed(2):'1';n.el.setAttribute('aria-pressed',String(n.id===selectedId));n.el.style.setProperty('--star-color',color);
      n.el.hidden=!inView(box(p.x-5,p.y-5,10,10));
    });
    const taken=labelsForScene(near,focusEdges,kept,keptEdges,now,dt,still||reduced.matches);if(labelsSettling)layoutMoving=true;
    // A plane's name keeps to one of its corners or edges where no other name stands.
    for(const plane of planes){
      if(reveal<.05||!plane.el.textContent||!plane.corners){plane.el.hidden=true;continue;}
      if(plane.measuredText!==plane.el.textContent){ng.font='600 9px system-ui, sans-serif';plane.measuredText=plane.el.textContent;plane.textWidth=Math.ceil(ng.measureText(plane.measuredText.toLocaleUpperCase()).width+plane.measuredText.length*2+4);}
      const [tl,tr,br,bl]=plane.corners,width=plane.textWidth,height=12;
      const candidates=[[tl.x+6,tl.y-height-5],[bl.x+6,bl.y+5],[tr.x-width-6,tr.y-height-5],[br.x-width-6,br.y+5],[(tl.x+bl.x)/2+6,(tl.y+bl.y)/2-height/2],[(bl.x+br.x)/2-width/2,(bl.y+br.y)/2+5]];
      let chosen=null;for(const [x,y] of candidates){const area=box(x,y,width,height);if(inView(area)&&!taken.some(o=>overlaps(area,o,3))){chosen=area;break;}}
      plane.el.hidden=!chosen;
      if(chosen){taken.push(chosen);plane.el.style.opacity=(.8*reveal).toFixed(3);plane.el.style.transform=`translate3d(${chosen.left.toFixed(2)}px,${chosen.top.toFixed(2)}px,0)`;}
    }
    paintStars(2,count);ctx.globalAlpha=1;ctx.endFrame?.();
    frameCount++;drawSum+=performance.now()-started;frameSum+=elapsed*1000;
    if(frameCount%90===0){root.dataset.drawMs=(drawSum/90).toFixed(2);root.dataset.frameMs=(frameSum/90).toFixed(2);root.dataset.frames=String(frameCount);drawSum=frameSum=0;}
    root.dataset.camera=[yaw,pitch,zoom,pan.x,pan.y].map(x=>x.toFixed(3)).join(',');
    if(onPose){const heard=`${yaw.toFixed(3)},${pitch.toFixed(3)},${!!measured}`;if(heard!==heardPose){heardPose=heard;onPose({yaw,pitch,restYaw,restPitch,measured:!!measured});}}
    const unsettled=Math.abs(yaw-tyaw)+Math.abs(pitch-tpitch)+Math.abs(zoom-tzoom)+Math.abs(pan.x-tpan.x)+Math.abs(pan.y-tpan.y)>.005;
    if(!still&&(playing||unsettled||layoutMoving||drag||velocity||reveal>0||litGlow!==(lit?1:0)))kick();else last=0;
  }
  function kick(){if(disposed||document.hidden)return;if(!raf)raf=requestAnimationFrame(draw)}
  // The part of a segment inside the free area, and whether its end is the segment's own.
  function clipSegment(a,b){
    const r=targetViewport,dx=b.x-a.x,dy=b.y-a.y;let t0=0,t1=1;
    for(const [p,q] of [[-dx,a.x-r.left],[dx,r.right-a.x],[-dy,a.y-r.top],[dy,r.bottom-a.y]]){
      if(p===0){if(q<0)return null;continue;}
      const t=q/p;if(p<0){if(t>t1)return null;if(t>t0)t0=t;}else{if(t<t0)return null;if(t<t1)t1=t;}
    }
    return [{...a,x:a.x+dx*t0,y:a.y+dy*t0},{...b,x:a.x+dx*t1,y:a.y+dy*t1},t1>=1];
  }
  // Both renderers fill full discs and stroke straight segments, so a ring is a dark core under a polygon outline.
  function ring(x,y,r,color,broken,hot){
    const alpha=ctx.globalAlpha;ctx.fillStyle='#050b16';ctx.globalAlpha=Math.min(1,alpha+.2);ctx.beginPath();ctx.arc(x,y,Math.max(1.2,r-1),0,Math.PI*2);ctx.fill();ctx.globalAlpha=alpha;
    ctx.strokeStyle=color;ctx.lineWidth=1.1+.4*Number(hot);circle(x,y,r,broken?12:hot>.5?28:20,broken);
  }
  // A circle stroked as a polygon fine enough to read round (both renderers stroke only straight segments); a broken
  // one keeps every other side.
  function circle(x,y,r,steps,broken=false,phase=0){
    ctx.beginPath();
    if(broken)for(let i=0;i<steps;i+=2){const a=i/steps*Math.PI*2+phase,b=(i+1)/steps*Math.PI*2+phase;ctx.moveTo(x+Math.cos(a)*r,y+Math.sin(a)*r);ctx.lineTo(x+Math.cos(b)*r,y+Math.sin(b)*r);}
    else{ctx.moveTo(x+r,y);for(let i=1;i<=steps;i++){const a=i/steps*Math.PI*2;ctx.lineTo(x+Math.cos(a)*r,y+Math.sin(a)*r);}}
    ctx.stroke();
  }
  // Where a star comes to rest once the camera and layout settle.
  function projectSettled(point){
    const saved={yaw,pitch,zoom,pan,center,fitScale,viewport,base,stretchY};
    yaw=tyaw;pitch=tpitch;zoom=tzoom;pan=tpan;center=targetCenter;fitScale=targetFit;viewport=targetViewport;stretchY=tstretchY;
    base=Math.max(.1,Math.min((viewport.right-viewport.left)/880,(viewport.bottom-viewport.top)/540,1.7));
    try{return project(point);}finally{({yaw,pitch,zoom,pan,center,fitScale,viewport,base,stretchY}=saved);}
  }
  // A chosen star that would rest outside the free area is brought just inside it:
  // the camera pans the shortest way, never turns and never changes scale.
  function revealPending(){
    const n=nodes.find(item=>item.id===revealId);revealId=null;if(!n)return;
    const p=projectSettled(n.target),area=targetViewport,mx=Math.min(96,(area.right-area.left)*.18),my=Math.min(72,(area.bottom-area.top)*.18);
    const dx=p.x<area.left+mx?area.left+mx-p.x:p.x>area.right-mx?area.right-mx-p.x:0,dy=p.y<area.top+my?area.top+my-p.y:p.y>area.bottom-my?area.bottom-my-p.y:0;
    if(dx||dy){tpan={x:tpan.x+dx,y:tpan.y+dy};kick();}
  }
  // A chosen star's neighbours are brought into the free area with it, so that every name of its neighbourhood can show:
  // the camera pans the shortest way and never turns; it draws back when the neighbourhood is wider than the area and
  // closer when its names would crowd.
  function revealNeighbourhood(){
    const pending=neighbourhoodPending;neighbourhoodPending=null;if(measured||!pending)return;
    if(Array.isArray(pending)){revealSet(new Set(pending));return;}
    // A chosen link brings its two stars in, as a chosen star brings its neighbours.
    const link=pending.edge?edges.find(e=>e.id===pending.edge):null;if(pending.edge&&!link)return;
    const id=link?null:pending,ids=new Set(link?[link.from,link.to]:[id]);
    if(!link)for(const e of edges)if(!e.muted&&(e.from===id||e.to===id)){ids.add(e.from);ids.add(e.to);}
    const points=nodes.filter(n=>ids.has(n.id)).map(n=>projectSettled(n.target));if(points.length<2)return;
    const area=targetViewport,mx=Math.min(110,(area.right-area.left)*.16),my=Math.min(80,(area.bottom-area.top)*.16);
    const cx=(area.left+area.right)/2,cy=(area.top+area.bottom)/2;
    const extent=list=>({left:Math.min(...list.map(p=>p.x)),right:Math.max(...list.map(p=>p.x)),top:Math.min(...list.map(p=>p.y)),bottom:Math.max(...list.map(p=>p.y))});
    let span=extent(points);
    const roomX=Math.max(40,area.right-area.left-2*mx),roomY=Math.max(40,area.bottom-area.top-2*my);
    if(closeIn){
      // The neighbourhood comes to the middle of the free area and nearer, until it fills about three quarters of it,
      // never further than the whole view and never more than two and a half times nearer at once.
      // A chosen link comes near enough to read as a line between its two stars, about two hundred points long, and may
      // come up to six times nearer at once to get there.
      const fill=Math.min(roomX*.75/Math.max(1,span.right-span.left),roomY*.75/Math.max(1,span.bottom-span.top)),reach=Math.hypot(span.right-span.left,span.bottom-span.top);
      const factor=link?clamp(Math.min(fill,200/Math.max(1,reach)),1,6):clamp(fill,1,2.5);
      if(factor>1.05){const before=tzoom;zoomTo(tzoom*factor);const ratio=tzoom/before;span=extent(points.map(p=>({x:cx+(p.x-cx-tpan.x)*ratio+tpan.x,y:cy+(p.y-cy-tpan.y)*ratio+tpan.y})));}
      tpan={x:tpan.x+cx-(span.left+span.right)/2,y:tpan.y+cy-(span.top+span.bottom)/2};kick();return;
    }
    const back=Math.min(1,roomX/Math.max(1,span.right-span.left),roomY/Math.max(1,span.bottom-span.top));
    // A crowded neighbourhood is drawn closer, so that its names have room to stand apart: the room its names need is
    // set against the room around its stars, never past what the free area holds or twice the present zoom.
    const near=link?[link]:edges.filter(e=>!e.muted&&(e.from===id||e.to===id));
    const demand=nodes.filter(n=>ids.has(n.id)).reduce((sum,n)=>sum+(n.fullWidth??n.labelWidth??120)*(n.fullHeight??n.labelHeight??18),0)+near.reduce((sum,e)=>sum+(e.labelWidth??120)*(e.labelHeight??30),0);
    const around=(span.right-span.left+120)*(span.bottom-span.top+120);
    const reach=Math.hypot(span.right-span.left,span.bottom-span.top);
    const grow=back<1?1:Math.min(Math.max(Math.sqrt(demand*2.4/Math.max(1,around)),link?200/Math.max(1,reach):0),roomX/Math.max(1,span.right-span.left),roomY/Math.max(1,span.bottom-span.top),link?4:2);
    const factor=back<1?back:grow>1.15?grow:1;
    if(factor!==1){
      const before=tzoom;zoomTo(tzoom*factor);const ratio=tzoom/before;
      span=extent(points.map(p=>({x:cx+(p.x-cx-tpan.x)*ratio+tpan.x,y:cy+(p.y-cy-tpan.y)*ratio+tpan.y})));
    }
    const shift=(low,high,from,to)=>high-low>to-from?(from+to)/2-(low+high)/2:low<from?from-low:high>to?to-high:0;
    const dx=shift(span.left,span.right,area.left+mx,area.right-mx),dy=shift(span.top,span.bottom,area.top+my,area.bottom-my);
    if(dx||dy){tpan={x:tpan.x+dx,y:tpan.y+dy};kick();}
  }
  // An aspect's stars are fitted to the free area, in or out, with room for the names that stand outside them: a star
  // named on its outer side at either end of the rows widens that side by its name. The whole is centred in the area.
  function revealSet(ids){
    measure();
    const points=nodes.filter(n=>ids.has(n.id)).map(n=>({n,p:projectSettled(n.target)}));if(points.length<2)return;
    const area=targetViewport,pad=24,my=Math.min(56,(area.bottom-area.top)*.12);
    const xs=points.map(({p})=>p.x),ys=points.map(({p})=>p.y),left=Math.min(...xs),right=Math.max(...xs),top=Math.min(...ys),bottom=Math.max(...ys);
    const name=n=>n.quiet?0:(n.fullWidth??n.labelWidth??120)+22,centred=n=>n.labelSide==='above'||n.labelSide==='below';
    // A name beside a star at an edge of the set reaches past it; a name above or below reaches half its width each way.
    const overLeft=Math.max(0,...points.filter(({n,p})=>p.x-left<4&&(n.labelSide==='left'||centred(n))).map(({n})=>centred(n)?name(n)/2:name(n)));
    const overRight=Math.max(0,...points.filter(({n,p})=>right-p.x<4&&(n.labelSide==='right'||centred(n))).map(({n})=>centred(n)?name(n)/2:name(n)));
    const roomX=Math.max(40,area.right-area.left-2*pad-overLeft-overRight),roomY=Math.max(40,area.bottom-area.top-2*my);
    // An aspect is laid out in units meant to be read as pixels: the view comes no nearer than that natural size.
    const probe=points[0].n.target,near0=projectSettled(probe),near1=projectSettled([probe[0]+100,probe[1],probe[2]]),natural=100/Math.max(1,Math.hypot(near1.x-near0.x,near1.y-near0.y));
    const before=tzoom;zoomTo(tzoom*Math.min(roomX/Math.max(1,right-left),roomY/Math.max(1,bottom-top),natural),16);const ratio=tzoom/before;
    const cx=(area.left+area.right)/2,cy=(area.top+area.bottom)/2,at=(v,c,shift)=>c+(v-c-shift)*ratio+shift;
    const l=at(left,cx,tpan.x)-overLeft,r=at(right,cx,tpan.x)+overRight,t=at(top,cy,tpan.y),b=at(bottom,cy,tpan.y);
    tpan={x:tpan.x+(cx-(l+r)/2),y:tpan.y+(cy-(t+b)/2)};kick();
  }
  // A turning space can be drawn close enough to open a crowded part, or far enough to see it whole.
  // A framing may come nearer than the reader's own zoom goes (`max`); from there the reader's zoom only moves back.
  // How near the reader may come; a space laid out whole (the tree) lets the reader come much nearer than a map.
  let zoomCeiling=4;
  const zoomTo=(value,max=zoomCeiling)=>{tzoom=measured?clamp(value,1,measured.axes==='xy'?8:4):clamp(value,.5,Math.max(max,Math.min(tzoom,16)));kick();};
  const wheel=e=>{
    e.preventDefault();const before=tzoom;zoomTo(tzoom*Math.exp(-e.deltaY*.0012));
    // On a measured scale the moment under the pointer stays under it.
    if(measured&&before>0){const rect=canvas.getBoundingClientRect(),x=e.clientX-rect.left-(targetViewport.left+targetViewport.right)/2,y=e.clientY-rect.top-(targetViewport.top+targetViewport.bottom)/2;tpan={x:x-(x-tpan.x)*tzoom/before,y:measured.axes==='xy'?y-(y-tpan.y)*tzoom/before:0};kick();}
  };
  // The wheel zooms over the sky and over what lies on it: stars, link labels and a scale's names.
  for(const target of [canvas,layer,edgeLayer,clusterLayer])target.addEventListener('wheel',wheel,{passive:false});
  const pointer=e=>{const rect=canvas.getBoundingClientRect();return{x:e.clientX-rect.left,y:e.clientY-rect.top};};
  // One drag from where it began: a measured layout and Shift (or the pan button) move the view, the rest turns it.
  function dragBy(state,dx,dy){
    introUntil=0;
    if(measured||state.forcePan||(rotate?state.shift:!state.shift)){tpan={x:state.pan.x+dx,y:measured?.axes==='x'?0:state.pan.y+dy};kick();return;}
    const nextYaw=state.yaw+dx*.0025,nextPitch=clamp(state.pitch+dy*.0025,...PITCH);
    if(inertia&&!reduced.matches){
      const now=performance.now();
      if(lastTurn&&now>lastTurn.t){const dt=(now-lastTurn.t)/1000,vy=(nextYaw-lastTurn.yaw)/dt,vp=(nextPitch-lastTurn.pitch)/dt,limit=v=>clamp(v,-3,3);velocity={yaw:limit(velocity?velocity.yaw*.35+vy*.65:vy),pitch:limit(velocity?velocity.pitch*.35+vp*.65:vp),t:now};}
      lastTurn={t:now,yaw:nextYaw,pitch:nextPitch};
    }
    tyaw=nextYaw;tpitch=nextPitch;kick();
  }
  // A turn let go while still moving coasts; one that had already stopped stays.
  function letGo(){if(!(inertia&&velocity&&performance.now()-velocity.t<90))velocity=null;lastTurn=null;}
  // Two fingers move and pinch the view wherever they land, on the sky or on its stars; the turn belongs to one finger.
  const touches=new Map(),presses=new Set();let gesture=null,touchHeld=false;
  const touchPair=()=>{const [a,b]=[...touches.values()];return {distance:Math.max(1,Math.hypot(b.x-a.x,b.y-a.y)),mid:{x:(a.x+b.x)/2,y:(a.y+b.y)/2}};};
  // While two fingers pinch, or the one left after a pinch is still down, no single-finger gesture starts or goes on.
  const pinching=e=>e.pointerType==='touch'&&(!!gesture||touchHeld);
  // Only fingers on the sky itself, its stars and its names pinch; a finger on a panel belongs to the panel.
  const onSky=e=>e.target===canvas||layer.contains(e.target)||edgeLayer.contains(e.target)||clusterLayer.contains(e.target);
  // A pinch keeps the point between the fingers under them: it zooms about it and follows it.
  const startPinch=()=>{gesture={...touchPair(),zoom:tzoom,pan:{...tpan},rect:canvas.getBoundingClientRect()};};
  const applyPinch=()=>{
    const pair=touchPair(),before=gesture.zoom;zoomTo(before*pair.distance/gesture.distance);
    const ratio=tzoom/before,r=gesture.rect,cx=(targetViewport.left+targetViewport.right)/2,cy=(targetViewport.top+targetViewport.bottom)/2;
    const from={x:gesture.mid.x-r.left-cx,y:gesture.mid.y-r.top-cy},to={x:pair.mid.x-r.left-cx,y:pair.mid.y-r.top-cy};
    tpan={x:to.x-(from.x-gesture.pan.x)*ratio,y:measured?.axes==='x'?0:to.y-(from.y-gesture.pan.y)*ratio};kick();
  };
  if(touch){
    root.addEventListener('pointerdown',e=>{
      if(e.pointerType!=='touch')return;
      // A new touch forgets fingers whose lifting was never heard.
      if(e.isPrimary){touches.clear();gesture=null;touchHeld=false;}
      if(!onSky(e))return;
      touches.set(e.pointerId,{x:e.clientX,y:e.clientY});
      if(touches.size<2)return;
      if(!gesture){
        // A pinch takes over from what one finger began: a turn goes back to where it started, a picked-up star stays put.
        if(drag){tyaw=drag.yaw;tpitch=drag.pitch;tpan={...drag.pan};drag=null;canvas.classList.remove('dragging');}
        for(const cancel of [...presses])cancel();
        velocity=null;introUntil=0;touchHeld=true;
      }
      // A finger more starts the pinch again from where the fingers are, so nothing jumps.
      startPinch();
    },{capture:true});
    root.addEventListener('pointermove',e=>{
      if(e.pointerType!=='touch'||!touches.has(e.pointerId))return;
      touches.set(e.pointerId,{x:e.clientX,y:e.clientY});
      if(gesture&&touches.size>=2)applyPinch();
    },{capture:true});
    const lift=e=>{if(e.pointerType==='touch'&&touches.delete(e.pointerId)){if(touches.size<2)gesture=null;else if(gesture)startPinch();if(!touches.size)touchHeld=false;}};
    root.addEventListener('pointerup',lift,{capture:true});root.addEventListener('pointercancel',lift,{capture:true});
  }
  canvas.addEventListener('pointerdown',e=>{
    if(pinching(e))return;
    const panButton=secondaryPan&&(e.button===1||e.button===2);
    if(e.button!==0&&!panButton)return;
    if(panButton)e.preventDefault();
    revealId=null;velocity=null;lastTurn=null;
    drag={id:e.pointerId,x:e.clientX,y:e.clientY,yaw:tyaw,pitch:tpitch,pan:{...tpan},shift:e.shiftKey,forcePan:panButton,moved:false};canvas.setPointerCapture(e.pointerId);canvas.classList.add('dragging');
  });
  if(secondaryPan)canvas.addEventListener('contextmenu',e=>e.preventDefault());
  // With a fine pointer the star whose centre lies nearest the pointer is the one it reaches: among close stars, and
  // under another star's name, the pointer takes the star it is over.
  const finePointer=matchMedia('(pointer: fine)');let aimed=null;
  const aim=id=>{if(id===aimed)return;const before=nodes.find(n=>n.id===aimed);if(before)delete before.el.dataset.aim;aimed=id;const next=id?nodes.find(n=>n.id===id):null;if(next)next.el.dataset.aim='true';root.dataset.aiming=String(!!next);};
  root.addEventListener('pointermove',e=>{
    if(!finePointer.matches||e.pointerType==='touch'||drag)return;
    const bounds=root.getBoundingClientRect(),x=e.clientX-bounds.left,y=e.clientY-bounds.top;
    let best=null,reach=14;
    for(const n of nodes){if(n.el.hidden||!n.screen||n.el.dataset.outside==='true')continue;const d=Math.hypot(n.screen.x-x,n.screen.y-y);if(d<reach){reach=d;best=n;}}
    aim(best?.id??null);
  },{passive:true,capture:true});
  root.addEventListener('pointerleave',()=>aim(null));
  // A tap on a star's own disc among close stars reaches the star whose centre lies nearest the finger; a tap on a name
  // reaches that name's star, and a finger that moved has turned the space and chooses nothing.
  let lastTouch=null;
  root.addEventListener('pointerdown',e=>{if(e.pointerType!=='touch')return;const b=root.getBoundingClientRect();lastTouch={x:e.clientX-b.left,y:e.clientY-b.top,cx:e.clientX,cy:e.clientY,at:performance.now()};},{capture:true,passive:true});
  root.addEventListener('click',e=>{
    const touch=lastTouch;lastTouch=null;const star=e.target?.closest?.('.tree-star');
    if(!touch||!star||e.target!==star||performance.now()-touch.at>900||Math.hypot(e.clientX-touch.cx,e.clientY-touch.cy)>10)return;
    let best=null,reach=26;for(const n of nodes){if(n.el.hidden||!n.screen||n.el.dataset.outside==='true')continue;const d=Math.hypot(n.screen.x-touch.x,n.screen.y-touch.y);if(d<reach){reach=d;best=n;}}
    if(best&&best.el!==star){e.stopPropagation();e.preventDefault();best.el.click();}
  },{capture:true});
  canvas.addEventListener('pointermove',e=>{
    if(pinching(e))return;
    if(!drag){const p=pointer(e),id=hitEdge(p.x,p.y);if(hoverEdge!==id){hoverEdge=id;canvas.style.cursor=id?'pointer':'';onEdgeHover?.(id);kick();}return;}
    if(e.pointerId!==drag.id)return;const dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.hypot(dx,dy)<5&&!drag.moved)return;drag.moved=true;hoverEdge=null;dragBy(drag,dx,dy);
  });
  const release=e=>{
    if(!drag||e.pointerId!==drag.id)return;const click=e.type==='pointerup'&&!drag.moved&&!drag.forcePan;drag=null;canvas.classList.remove('dragging');letGo();
    if(click){const p=pointer(e),id=hitEdge(p.x,p.y);if(id)onEdgeSelect(id);else onBackground?.();}kick();
  };canvas.addEventListener('pointerup',release);canvas.addEventListener('pointercancel',release);canvas.addEventListener('lostpointercapture',release);
  canvas.addEventListener('pointerleave',()=>{if(!drag){if(hoverEdge)onEdgeHover?.(null);hoverEdge=null;canvas.style.cursor='';kick();}});
  const visibility=()=>{last=0;if(document.hidden){cancelAnimationFrame(raf);raf=0}else kick()};document.addEventListener('visibilitychange',visibility);
  canvas.addEventListener('sophia-renderer-restored',kick);
  const ro=new ResizeObserver(resize);ro.observe(root);paintNebula();resize();
  // Labels are measured in their type; once the page's fonts are ready they are measured again.
  document.fonts?.ready?.then(()=>{if(!disposed){measureLabels=true;kick();}});
  return {
    update:refresh,
    setCloseIn(on){closeIn=!!on;},
    setZoomCeiling(value){if(Number.isFinite(value)&&value>=1)zoomCeiling=value;},
    setInk(value){const next=clamp(Number(value)||1,1,2.5);if(next!==ink){ink=next;kick();}},
    capturePose(){return validateSkyPose({v:1,yaw:tyaw,pitch:tpitch,zoom:tzoom,pan:[tpan.x,tpan.y],center:[...targetCenter],fit:targetFit});},
    restorePose(value){const pose=validateSkyPose(value);yaw=tyaw=pose.yaw;pitch=tpitch=pose.pitch;zoom=tzoom=pose.zoom;
      pan={x:pose.pan[0],y:pose.pan[1]};tpan={...pan};center=[...pose.center];targetCenter=[...pose.center];fitScale=targetFit=pose.fit;
      // A pose restored before any framing turns about its own fit.
      if(!frontFit)frontFit=pose.fit;fittedFor='';kick();},
    select(id){const changed=id!==selectedId;selectedId=id;updateThreads();if(changed&&id)neighbourhoodPending=id;kick()},
    selectEdge(id){const changed=id!==selectedEdgeId;selectedEdgeId=id;if(changed&&id){neighbourhoodPending={edge:id};edgeChosenAt=performance.now();}kick()},
    link(id){linkFrom=id;kick()},
    zoomBy(factor){if(Number.isFinite(factor)&&factor>0)zoomTo(tzoom*factor);},
    frame({fill=false,x=null,y=null,plane=false}={}){
      if(fill)resize();tstretchY=1;measured=null;
      if(nodes.length){
        const points=(aspectIds&&nodes.some(n=>aspectIds.has(n.id))?nodes.filter(n=>aspectIds.has(n.id)):nodes).map(n=>n.target),min=[0,1,2].map(i=>Math.min(...points.map(p=>p[i]))),max=[0,1,2].map(i=>Math.max(...points.map(p=>p[i])));
        if(Array.isArray(x)&&x.length===2&&x.every(Number.isFinite)){min[0]=Math.min(min[0],x[0]);max[0]=Math.max(max[0],x[1]);}
        if(Array.isArray(y)&&y.length===2&&y.every(Number.isFinite)){min[1]=Math.min(min[1],y[0]);max[1]=Math.max(max[1],y[1]);}
        targetCenter=min.map((v,i)=>(v+max[i])/2);
        const vw=targetViewport.right-targetViewport.left,vh=targetViewport.bottom-targetViewport.top,b=Math.max(.1,Math.min(vw/880,vh/540,1.7)),sx=clamp((vw/vh)/(880/540),1,2.1);
        const dx=Math.max(1,max[0]-min[0]),dy=Math.max(1,max[1]-min[1]);
        if(fill&&plane){
          // A plane keeps one unit on both axes: the whole extent fits the free area without stretching.
          targetFit=Math.min(.92*vw/(dx*b),.88*vh/(dy*b));measured={axes:'xy',x:[min[0],max[0]],y:[min[1],max[1]]};
        }else if(fill){
          // A scale fills its free area: the width holds the scale, its rows share the height.
          targetFit=Math.min(1.6,.94*vw/(dx*b*sx));tstretchY=clamp(.86*vh/(dy*targetFit*b),.5,6);targetFit=Math.min(targetFit,.86*vh/(dy*b*tstretchY));measured={axes:'x',x:[min[0],max[0]]};
        }else targetFit=Math.min(1.6,810/Math.max(510,max[0]-min[0]),455/Math.max(285,max[1]-min[1]),900/Math.max(560,max[2]-min[2]));
        // A space that turns keeps this as its front fit and is fitted again for each pose it is turned to.
        frontFit=measured?0:targetFit;fittedFor='';
      }
      // A measured layout is seen straight on, so the view's tilt does not bend its measure; a space rests in its own pose.
      velocity=null;tyaw=nearestTurn(measured?0:restYaw,tyaw);tpitch=measured?0:restPitch;tzoom=1;tpan={x:0,y:0};
      const introNow=intro&&!measured&&!introDone&&!reduced.matches;
      // The first framing is where the view begins, unless the space is to turn into it; later framings ease there.
      if(!framedOnce){framedOnce=true;if(rest&&!introNow){yaw=tyaw;pitch=tpitch;}}
      if(introNow){introDone=true;introPending=true;}
      resize()},
    /** Turn the view by a step, as a drag would; a measured layout does not turn. */
    turnBy(dyaw=0,dpitch=0){if(measured)return;velocity=null;introUntil=0;turnShownUntil=performance.now()+900;tyaw+=dyaw;tpitch=clamp(tpitch+dpitch,...PITCH);kick();},
    /** Face the space straight on, so that rows read level; a measured layout already does. */
    faceFront(){if(measured)return;velocity=null;introUntil=0;tyaw=nearestTurn(0,tyaw);tpitch=0;kick();},
    /** Bring the view back to its resting pose, the shorter way round. */
    // Turn the space about its vertical axis so that the given turn faces the reader; the tilt stays, unless given.
    turnTo(yawTo,pitchTo=null){if(measured)return;velocity=null;introUntil=0;turnShownUntil=performance.now()+900;if(Number.isFinite(yawTo))tyaw=nearestTurn(yawTo,tyaw);if(Number.isFinite(pitchTo))tpitch=clamp(pitchTo,...PITCH);kick();},
    setRest(yawTo=null,pitchTo=null){restYaw=Number.isFinite(yawTo)?yawTo:baseRestYaw;restPitch=Number.isFinite(pitchTo)?pitchTo:baseRestPitch;},
    turnToRest(){if(measured)return;velocity=null;introUntil=0;turnShownUntil=performance.now()+900;tyaw=nearestTurn(restYaw,tyaw);tpitch=restPitch;kick();},
    pose(){return {yaw,pitch,targetYaw:tyaw,targetPitch:tpitch,restYaw,restPitch,measured:!!measured};},
    focus(id){const n=nodes.find(n=>n.id===id);if(!n)return;targetCenter=[...n.target];tzoom=1.3;tpan={x:0,y:0};kick();},
    reveal(id){revealId=id??null;kick();},
    /** Bring a set of stars into the free area together, drawing back or closer as the neighbourhood is. */
    revealAll(ids){if(Array.isArray(ids)&&ids.length>1){neighbourhoodPending=[...ids];kick();}},
    /** Light a way through the given stars, in order (null puts it out). */
    light(ids){lit=Array.isArray(ids)&&ids.length>1?[...ids]:null;kick();},
    refresh:resize,
    motion(value){playing=value;kick()},
    moving(){return playing;},
    dispose(){disposed=true;cancelAnimationFrame(raf);ro.disconnect();document.removeEventListener('visibilitychange',visibility);for(const target of [canvas,layer,edgeLayer,clusterLayer])target.removeEventListener('wheel',wheel);canvas.removeEventListener('sophia-renderer-restored',kick);nodes.forEach(n=>n.el.remove());edges.forEach(e=>e.el.remove());clusters.forEach(c=>c.el.remove());planes.forEach(plane=>plane.el.remove());threads.forEach(thread=>thread.el.remove());ctx.dispose?.()}
  };
}
