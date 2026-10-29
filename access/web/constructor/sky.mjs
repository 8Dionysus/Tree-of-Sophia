import { SophiaGpuCanvas } from '../src/observatory/gpu-canvas.js';
import {validateSkyPose} from './sky-pose.mjs';

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
export function mountConstructorSky(root, {onSelect, onMove, onEdgeSelect=()=>{}, getViewport=null, rotate=true, dragNodes=true, rest=null, intro=false, holdToMove=0, turnFromStars=false, touch=false, secondaryPan=false, inertia=false, depthCue=false, planarMoves=false, onPose=null, nativeTitles=true, onEdgeHover=null, onThreadSelect=null, threadsFor=null}) {
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

  const restYaw=Number.isFinite(rest?.yaw)?rest.yaw:0,restPitch=Number.isFinite(rest?.pitch)?rest.pitch:-.055;
  const YAW=[-.9,.9],PITCH=[-.6,.6];
  let w=1,h=1,dpr=1,base=1,skyTime=0,yaw=restYaw,pitch=restPitch,zoom=1;
  let tyaw=restYaw,tpitch=restPitch,tzoom=1,pan={x:0,y:0},tpan={x:0,y:0},eye={x:0,y:0},calm=0,revealId=null,stretchY=1,tstretchY=1,measured=null;
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
  let nodes=[],edges=[],clusters=[],backdrop=[],planes=[],threads=[],lit=null,litGlow=0,selectedId=null,selectedEdgeId=null,hoverEdge=null,linkFrom=null,center=[0,0,0],targetCenter=[0,0,0],fitScale=1,targetFit=1,edgeHits=[],measureLabels=true;
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
      el.setAttribute('aria-label',`${labels[item.from]??item.from} → ${tag?tag+': ':''}${title} → ${labels[item.to]??item.to}`);el.hidden=true;
      return {...item,el,color:hex(item.color,relationColors[item.kind]??relationColors.relates)};
    });oldEdges.forEach(e=>e.el.remove());
    if('selectedEdgeId' in state)selectedEdgeId=state.selectedEdgeId;
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
            const vx=dx/Math.max(.1,n.screen.sX??n.screen.s),vy=dy/Math.max(.1,n.screen.sY??n.screen.s),c=Math.cos(yaw),ss=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch);
            // In a space read in depth a star keeps its depth and moves in its own plane.
            if(planarMoves){const ax=vx/Math.max(.2,c),ay=(vy+ax*ss*sp)/Math.max(.2,cp);n.p=[nodeDrag.p[0]+ax,nodeDrag.p[1]+ay,nodeDrag.p[2]];}
            else n.p=[nodeDrag.p[0]+vx*c-vy*ss*sp,nodeDrag.p[1]+vy*cp,nodeDrag.p[2]-vx*ss-vy*c*sp];
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
      n.target=[...item.position];n.kind=item.kind;n.color=item.color;n.prominent=!!item.prominent;n.dimmed=!!item.dim;n.form=item.form==='proposal'||item.form==='personal'?item.form:'source';n.el.dataset.form=n.form;n.major=item.major??['work','concept','dossier','figure'].includes(item.kind);
      n.titleText=labels[item.id]??item.title??item.id;n.subtitle=typeof item.subtitle==='string'?item.subtitle:'';
      n.labelMaxWidth=Number.isFinite(item.labelMaxWidth)?Math.max(80,Math.min(320,item.labelMaxWidth)):null;n.labelLines=item.labelLines===2?2:1;
      const clampStyle={whiteSpace:n.labelLines>1?'normal':'',display:n.labelLines>1?'-webkit-box':'',webkitBoxOrient:n.labelLines>1?'vertical':'',webkitLineClamp:n.labelLines>1?String(n.labelLines):''};
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
    clusters=(state.clusters??[]).map(item=>{let c=oldClusters.get(item.id);oldClusters.delete(item.id);if(!c){const el=document.createElement('div'),title=document.createElement('span'),subtitle=document.createElement('small');el.className='tree-cluster';title.className='cluster-title';subtitle.className='cluster-subtitle';el.append(title,subtitle);Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'none'});clusterLayer.append(el);c={el,title,subtitle,p:[...item.center]};}c.id=item.id;c.target=[...item.center];c.variant=typeof item.variant==='string'?item.variant:'';if(c.variant)c.el.dataset.variant=c.variant;else delete c.el.dataset.variant;c.strip=Number.isFinite(item.strip)?item.strip:null;c.span=Array.isArray(item.span)&&item.span.length===2&&item.span.every(Number.isFinite)?[...item.span]:null;c.align=item.align==='start'||item.align==='end'?item.align:'';if(c.align)c.el.dataset.align=c.align;else delete c.el.dataset.align;c.maxWidth=Number.isFinite(item.maxWidth)?item.maxWidth:null;c.radius=Number.isFinite(item.radius)?item.radius:null;const home=clipped.has(c.variant)?ringLayer:underStars.has(c.variant)?ruleLayer:clusterLayer;if(c.el.parentElement!==home)home.append(c.el);if(item.ref){c.el.dataset.ref=String(item.ref);c.el.style.pointerEvents='auto';}else{delete c.el.dataset.ref;c.el.style.pointerEvents='none';}if(item.tone)c.el.dataset.tone=String(item.tone);else delete c.el.dataset.tone;if(item.hint&&nativeTitles)c.el.title=item.hint;else c.el.removeAttribute('title');if(item.hint&&!nativeTitles)c.el.dataset.hint=item.hint;else delete c.el.dataset.hint;c.title.textContent=item.title??'';c.subtitle.textContent=item.subtitle??'';c.el.style.setProperty('--cluster-color',hex(item.color,'#a5b7cf'));c.el.style.color=hex(item.color,'#a5b7cf');return c;});oldClusters.forEach(c=>c.el.remove());
    // A measured plane may lie on a backdrop of lines in its own units, such as coastlines.
    backdrop=Array.isArray(state.backdrop)?state.backdrop.filter(line=>Array.isArray(line)&&line.length>1):[];
    // A space read in depth may lay its stars on planes, each at its own depth and named at a corner.
    const oldPlanes=new Map(planes.map(plane=>[plane.id,plane]));
    planes=(Array.isArray(state.planes)?state.planes:[]).filter(item=>typeof item?.id==='string'&&Number.isFinite(item.z)&&[item.x,item.y].every(range=>Array.isArray(range)&&range.length===2&&range.every(Number.isFinite))).map(item=>{
      let plane=oldPlanes.get(item.id);oldPlanes.delete(item.id);
      if(!plane){const el=document.createElement('div');el.className='tree-plane';el.dataset.plane=item.id;el.hidden=true;Object.assign(el.style,{position:'absolute',left:'0',top:'0',pointerEvents:'none'});ruleLayer.append(el);plane={id:item.id,el};}
      plane.el.textContent=typeof item.label==='string'?item.label:'';plane.z=item.z;plane.x=[...item.x];plane.y=[...item.y];plane.color=hex(item.color,'#a9bdd6');plane.el.style.color=plane.color;return plane;
    });oldPlanes.forEach(plane=>plane.el.remove());
    updateThreads();
    if(!nodes.some(n=>n.id===hover))hover=null;if(!edges.some(e=>e.id===hoverEdge))hoverEdge=null;
    measureLabels=true;resize();
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
  }
  function project([x,y,z]) {
    const c=Math.cos(yaw),s=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch);
    x=(x-center[0])*fitScale;y=(y-center[1])*fitScale;z=(z-center[2])*fitScale;
    const rx=x*c-z*s,rz=x*s+z*c,ry=y*cp-rz*sp,zz=y*sp+rz*cp,depth=FOCAL/Math.max(130,FOCAL-zz);
    // Fill a wide viewport with the atlas while stars and atmosphere keep their shape.
    // A plane measures both axes in one unit, so it is never stretched; other layouts fill a wide viewport.
    const plane=measured?.axes==='xy',stretchX=plane?1:clamp(((viewport.right-viewport.left)/Math.max(1,viewport.bottom-viewport.top))/(880/540),1,2.1),scale=base*fitScale*zoom*depth;
    // A scale zooms and pans along its axis only and its rows keep their height; a plane zooms and pans both ways.
    const scaleOnly=measured?.axes==='x',zoomY=scaleOnly?1:zoom;
    return {x:(viewport.left+viewport.right)*.5+rx*base*zoom*depth*stretchX+pan.x,y:(viewport.top+viewport.bottom)*.5+ry*base*zoomY*depth*stretchY+(scaleOnly?0:pan.y),s:scale,sX:scale*stretchX,sY:base*fitScale*zoomY*depth*stretchY,d:depth};
  }
  function resize(){
    const first=w===1;w=Math.max(1,root.clientWidth);h=Math.max(1,root.clientHeight);dpr=Math.min(devicePixelRatio||1,softwareRenderer?1:1.25);
    const cw=Math.round(w*dpr),ch=Math.round(h*dpr);if(canvas.width!==cw)canvas.width=cw;if(canvas.height!==ch)canvas.height=ch;
    const desktop=w>760,cinema=root.dataset.cinema==='true',reading=!cinema&&root.dataset.reading==='true',deck=!cinema&&root.dataset.deck==='true',journey=!cinema&&root.dataset.journey==='true';
    targetViewport={left:desktop?166:24,right:w-(desktop?(reading?350:30):24),top:desktop?185:145,bottom:h-(desktop?(journey?260:deck?225:90):(reading?Math.max(180,h*.48):deck?150:65))};
    targetViewport.right=Math.max(targetViewport.left+120,targetViewport.right);targetViewport.bottom=Math.max(targetViewport.top+130,targetViewport.bottom);
    const bounds=root.getBoundingClientRect();uiObstacles=[];
    for(const el of root.querySelectorAll('.view-heading,.header,.lens-rail,.collection-deck,.relation-bar,.footer,.reading,.journey-dock,.ps-map-tools,.ps-column-tab,.ps-search-results')){const style=getComputedStyle(el),rect=el.getBoundingClientRect();if(el.hidden||style.display==='none'||style.visibility==='hidden'||Number(style.opacity)===0||!rect.width||!rect.height)continue;uiObstacles.push({left:rect.left-bounds.left,top:rect.top-bounds.top,right:rect.right-bounds.left,bottom:rect.bottom-bounds.top});}
    // A workspace can supply its actual free area; the constructor keeps its own layout.
    const supplied=getViewport?.({width:w,height:h});
    if(supplied&&['left','right','top','bottom'].every(k=>Number.isFinite(supplied[k]))&&supplied.right-supplied.left>=120&&supplied.bottom-supplied.top>=120)targetViewport={...supplied};
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
    for(const n of nodes){
      const text=n.titleText??n.label.textContent,sub=n.subtitle&&n.subEl?getComputedStyle(n.subEl):null;
      let subWidth=0,subHeight=0;if(sub){ng.font=sub.font||'11px sans-serif';subWidth=ng.measureText(n.subtitle).width;subHeight=Math.max(14,parseFloat(sub.lineHeight)||15)+2;}
      const style=getComputedStyle(n.subtitle&&n.titleEl?n.titleEl:n.label);ng.font=style.font||'12px sans-serif';
      n.labelWidth=Math.min(n.labelMaxWidth??(root.dataset.journey==='true'?230:180),Math.max(50,Math.max(ng.measureText(text).width,subWidth)+(n.labelLines>1?8:4)));
      let lines=1,line='';if(n.labelLines>1)for(const word of text.split(/\s+/)){const next=line?line+' '+word:word;if(line&&ng.measureText(next).width>n.labelWidth-8){lines++;line=word;}else line=next;}
      n.titleHeight=Math.max(16,parseFloat(style.lineHeight)||18)*Math.min(lines,n.labelLines)+(n.labelLines>1?2:0);n.labelHeight=n.titleHeight+subHeight;
    }
    // A scale's labels are measured in their own type, so a name never loses its end to a guess.
    for(const c of clusters)if(c.variant&&!underStars.has(c.variant)){ng.font=getComputedStyle(c.title).font||'12px sans-serif';c.textWidth=ng.measureText(c.title.textContent).width;}
    // A group's name and subtitle are measured too, so the room it keeps is the room it takes: a subtitle keeps to two
    // lines where the name's width allows, and the height counts every line it wraps to.
    for(const c of clusters){
      if(c.variant)continue;
      const titleStyle=getComputedStyle(c.title),titleText=c.title.textContent,size=parseFloat(titleStyle.fontSize)||14;
      ng.font=titleStyle.font||`italic ${size}px serif`;
      let width=Math.min(230,Math.max(110,Math.ceil(ng.measureText(titleText).width+(parseFloat(titleStyle.letterSpacing)||0)*titleText.length)+12));
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
      if(!e.tag){e.labelWidth=Math.min(190,Math.max(56,ng.measureText(e.el.textContent).width+20));e.labelHeight=Math.max(24,lineHeight+10);continue;}
      // A typed link shows its kind on its own line above the authored label.
      const text=ng.measureText(typeof e.label==='string'?e.label:'').width,word=ng.measureText(e.tag).width;
      e.labelWidth=Math.min(230,Math.max(70,Math.max(text,word)+22));e.labelHeight=lineHeight*(1+(text+22>230?2:1))+12;
      // Without room for the full label, the kind alone still names the line.
      ng.font=`600 10px ${style.fontFamily||'sans-serif'}`;e.compactWidth=Math.ceil(ng.measureText(e.tag.toLocaleUpperCase()).width*1.12)+18;e.compactHeight=21;
    }
  }
  function strokeSegment(a,b,pattern='solid'){
    const dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);if(length<1)return;
    const [on,period]=pattern===true||pattern==='dashed'?[7,13]:pattern==='dotted'?[2,6]:[0,0];
    ctx.beginPath();
    if(on){for(let start=0;start<length;start+=period){const end=Math.min(start+on,length);ctx.moveTo(a.x+dx*start/length,a.y+dy*start/length);ctx.lineTo(a.x+dx*end/length,a.y+dy*end/length);}}
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
      const fits=c.variant==='lane'||c.variant==='ytick'?area.right-area.left>=24&&area.top>=vt-2&&area.bottom<=vb+2:text.left>=vl-12&&text.right<=vr+12;
      // A scale never writes over the page's own panels, even when the map is squeezed under a sheet.
      const shown=fits&&!scaleLabels.some(o=>overlaps(area,o,2))&&!uiObstacles.some(o=>overlaps(area,o,0));
      c.el.hidden=!shown;
      if(shown){c.el.style.width=(area.right-area.left).toFixed(1)+'px';c.el.style.transform=`translate3d(${area.left.toFixed(2)}px,${area.top.toFixed(2)}px,0)`;scaleLabels.push(area);occupied.push(area);}
    }
    const nodeObstacles=nodes.filter(n=>!n.el.hidden).map(n=>({id:n.id,...box(n.screen.x-9,n.screen.y-9,18,18)}));
    for(const c of clusters){
      if(c.variant)continue;
      const width=c.labelWidth??110,height=c.labelHeight??(c.subtitle.textContent?40:25);
      const candidates=[-101,-76,106,-136,140].map(offset=>{const p=project([c.p[0],c.p[1]+offset,c.p[2]]),top=offset===-101?Math.max(targetViewport.top+4,p.y-20):p.y-20;return box(p.x-width/2,top,width,height);}).filter(inView);
      // A group's name takes a place clear of the stars where it can, and never one over another name.
      const area=candidates.find(candidate=>!occupied.some(o=>overlaps(candidate,o))&&!nodeObstacles.some(o=>overlaps(candidate,o,2)))??candidates.find(candidate=>!occupied.some(o=>overlaps(candidate,o)))??null;
      c.el.hidden=!area;
      if(area){c.el.style.width=width+'px';c.el.style.transform=`translate3d(${area.left.toFixed(2)}px,${area.top.toFixed(2)}px,0)`;occupied.push(area);}
    }
    const priority=n=>n.id===selectedId?100:n.id===hover?90:n.prominent?80:near.has(n.id)?70:n.major?50:10;
    for(const n of [...nodes].sort((a,b)=>priority(b)-priority(a))){
      const {x,y}=n.screen,width=n.labelWidth,hot=n.id===selectedId||n.id===hover;
      const place=height=>{
        const narrow=Math.max(90,width-28),possibilities=[box(x-width/2,y+19,width,height),box(x-width/2,y-height-19,width,height),box(x+19,y-height/2,width,height),box(x-width-19,y-height/2,width,height),...(narrow<width?[box(x+19,y-height/2,narrow,height),box(x-narrow-19,y-height/2,narrow,height)]:[])];
        const within=possibilities.filter(inView),clear=within.find(rect=>!occupied.some(o=>overlaps(rect,o))&&!nodeObstacles.some(o=>o.id!==n.id&&overlaps(rect,o,2)));
        return clear||(hot?within[0]:n.prominent?within.find(rect=>!occupied.some(o=>overlaps(rect,o))):null);
      };
      // Without room for its source line, a star keeps its name rather than losing both.
      let chosen=place(n.labelHeight),short=false;
      if(!chosen&&n.subtitle&&n.titleHeight){chosen=place(n.titleHeight);short=!!chosen;}
      n.el.dataset.short=String(short);const visible=!!chosen&&!n.el.hidden;
      n.labelBox=visible?chosen:null;n.el.dataset.quiet=String(!visible);n.label.style.opacity=visible?'1':'0';n.label.style.pointerEvents=visible?'auto':'none';
      if(chosen){const fit=chosen.right-chosen.left;Object.assign(n.label.style,{left:(chosen.left-x+22)+'px',top:(chosen.top-y+22)+'px',bottom:'auto',transform:'none',width:fit+'px',maxWidth:fit+'px',minWidth:'0'});if(visible)occupied.push(chosen);}
    }
    const candidates=edges.filter(e=>!e.muted&&(e.id===selectedEdgeId||e.id===hoverEdge||((hover||selectedId)&&focusEdges.has(e.id)))).sort((a,b)=>(b.id===hoverEdge?100:b.id===selectedEdgeId?90:b.highlight?70:0)-(a.id===hoverEdge?100:a.id===selectedEdgeId?90:a.highlight?70:0));
    const visibleEdges=new Set();let shown=0;
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
      if(hot)for(const n of nodes)if(n.labelBox&&overlaps(chosen,n.labelBox)){n.labelBox=null;n.el.dataset.quiet='true';n.label.style.opacity='0';n.label.style.pointerEvents='none';}
      visibleEdges.add(e.id);e.el.dataset.active=String(hot);e.el.dataset.kind=e.kind;e.el.dataset.compact=String(compact);e.el.style.setProperty('--edge-color',e.color);e.el.style.color=e.color;e.el.style.width=(compact?e.compactWidth:e.labelWidth)+'px';e.el.style.transform=`translate3d(${chosen.left.toFixed(2)}px,${chosen.top.toFixed(2)}px,0)`;if(nativeTitles)e.el.title=e.el.getAttribute('aria-label');occupied.push(chosen);shown++;
    }
    for(const e of edges)e.el.hidden=!visibleEdges.has(e.id);
    return occupied;
  }

  function draw(now,still=false){
    raf=0;if(disposed||(document.hidden&&!still))return;
    const frameInterval=softwareRenderer?1000/28:1000/36;if(!still&&lastFrame&&now-lastFrame<frameInterval){raf=requestAnimationFrame(draw);return;}lastFrame=now;
    const started=performance.now(),elapsed=last?(now-last)/1000:1/60,dt=Math.min(elapsed,.05),ease=(reduced.matches||still)?1:1-Math.exp(-dt*5);last=now;
    if(playing&&!still)skyTime+=dt*.60;
    if(velocity&&!drag&&!still&&!nodes.some(n=>n.turningView)){
      const nextYaw=clamp(tyaw+velocity.yaw*dt,...YAW),nextPitch=clamp(tpitch+velocity.pitch*dt,...PITCH),decay=Math.exp(-dt*3.6);
      velocity.yaw=nextYaw===tyaw+velocity.yaw*dt?velocity.yaw*decay:0;velocity.pitch=nextPitch===tpitch+velocity.pitch*dt?velocity.pitch*decay:0;
      tyaw=nextYaw;tpitch=nextPitch;if(Math.abs(velocity.yaw)+Math.abs(velocity.pitch)<.015)velocity=null;
    }
    // The first view of a space turns slowly into its rest, so that its depth is seen moving once.
    if(introPending&&nodes.length&&!still){introPending=false;yaw=clamp(restYaw+(Math.sign(restYaw)||1)*.6,...YAW);pitch=clamp(restPitch-(Math.sign(restPitch)||1)*.2,...PITCH);introUntil=now+2200;}
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
    ctx.beginFrame?.();ctx.setTransform(dpr,0,0,dpr,0,0);ctx.globalAlpha=1;ctx.globalCompositeOperation='source-over';ctx.drawImage(nebula,0,0,w,h);
    const count=Math.round(dust.length*design.density);paintStars(0,count);paintFog(fogBack,.2,.85);paintStars(1,count);paintFog(fogFront,2.4,.46);
    nodes.forEach(n=>n.screen=project(n.p));
    if(revealId&&!drag)revealPending();
    const byId=new Map(nodes.map(n=>[n.id,n]));
    const selectedEdge=edges.find(e=>e.id===selectedEdgeId),hoveredEdge=edges.find(e=>e.id===hoverEdge),focusId=hover??selectedId,near=new Set(focusId?[focusId]:[]),focusEdges=new Set();
    for(const edge of [selectedEdge,hoveredEdge])if(edge){near.add(edge.from);near.add(edge.to);focusEdges.add(edge.id);}
    for(const edge of edges)if(!edge.muted&&(edge.from===focusId||edge.to===focusId)){near.add(edge.from);near.add(edge.to);focusEdges.add(edge.id);}
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
    if(measured&&backdrop.length){
      ctx.strokeStyle='#8fa8c4';ctx.globalAlpha=.3;ctx.lineWidth=.9;ctx.beginPath();
      for(const line of backdrop){let previous=null;for(const [x,y] of line){const point=project([x,y,0]);if(previous){const cut=clipSegment(previous,point);if(cut){ctx.moveTo(cut[0].x,cut[0].y);ctx.lineTo(cut[1].x,cut[1].y);}}previous=point;}}
      ctx.stroke();ctx.globalAlpha=1;
    }
    for(const edge of edges){
      let a=byId.get(edge.from)?.screen,b=byId.get(edge.to)?.screen,whole=true;if(!a||!b)continue;
      if(measured){const cut=clipSegment(a,b);if(!cut)continue;[a,b,whole]=cut;}
      if(edge.muted){ctx.strokeStyle=edge.color;ctx.globalAlpha=.05;ctx.lineWidth=.6;strokeSegment(a,b,edge.dash);continue;}
      const hot=edge.id===hoverEdge||edge.id===selectedEdgeId||edge.highlight,neighbor=focusEdges.has(edge.id),rest=Number.isFinite(edge.baseAlpha)?edge.baseAlpha:.24,weight=Number.isFinite(edge.weight)?edge.weight:1;
      ctx.strokeStyle=edge.color;ctx.globalAlpha=hot?.85:neighbor?.57:focused?rest*.23:rest;ctx.lineWidth=(hot?1.55:neighbor?1.1:.65)*weight;
      strokeSegment(a,b,edge.dash??(edge.kind==='questions'||edge.kind==='contrasts'?'dashed':'solid'));
      if((hot||neighbor)&&whole)arrow(a,b);
      edgeHits.push({id:edge.id,a,b});
    }
    // A route's way lights up while its card is pointed at: a golden line through its stars in the order it walks them.
    litGlow+=((lit?1:0)-litGlow)*(1-Math.exp(-dt*(lit?9:4)));if(litGlow<.01)litGlow=0;
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
      const p=n.screen,hot=n.id===selectedId||n.id===hover||(holdToMove>0&&n.dragging),color=colorOf(n),big=n.major,dim=(focused&&root.dataset.journey!=='true'&&!near.has(n.id)||n.dimmed)&&!hot,radius=(hot?5.2:big?4.3:2.8)*Math.max(.70,Math.min(1.5,p.s));
      const near01=depthCue&&!measured?clamp((p.d-.86)/.3,0,1):1,fade=hot?1:.5+.5*near01;
      // A measured layout draws only the stars inside its free area; every star still keeps its type, so its label is measured right.
      if(n.el.dataset.major!==String(!!big)){n.el.dataset.major=String(!!big);measureLabels=true;}
      if(measured&&!inView(box(p.x-5,p.y-5,10,10))){n.el.hidden=true;return;}
      ctx.globalAlpha=(dim?.28:hot?1:.9)*fade;const glow=(hot?125:big?86:46)*Math.max(.65,Math.min(1.4,p.s));
      ctx.drawImage(sprite(color),p.x-glow/2,p.y-glow/2,glow,glow);
      // A source is a disc; an authored proposal a ring; a personal record a broken ring.
      if(n.form==='source'){ctx.fillStyle=color;ctx.beginPath();ctx.arc(p.x,p.y,radius,0,Math.PI*2);ctx.fill();ctx.fillStyle='#fff9ea';ctx.fillRect(p.x-1,p.y-1,2,2);}
      else ring(p.x,p.y,radius+.9,color,n.form==='personal',hot);
      const ray=hot?17:big?10:5;ctx.globalAlpha=dim?.12:hot?.9:.38;ctx.strokeStyle=color;ctx.lineWidth=.65;ctx.beginPath();ctx.moveTo(p.x-ray,p.y);ctx.lineTo(p.x+ray,p.y);ctx.moveTo(p.x,p.y-ray);ctx.lineTo(p.x,p.y+ray);ctx.stroke();
      if(n.id===selectedId||n.id===linkFrom){ctx.globalAlpha=.65;ctx.beginPath();for(const [sx,sy] of [[-1,-1],[-1,1],[1,-1],[1,1]]){ctx.moveTo(p.x+sx*22,p.y+sy*16);ctx.lineTo(p.x+sx*22,p.y+sy*22);ctx.lineTo(p.x+sx*16,p.y+sy*22);}ctx.stroke();}
      n.el.style.transform=`translate3d(${(p.x-22).toFixed(2)}px,${(p.y-22).toFixed(2)}px,0)`;
      n.el.dataset.active=String(n.id===selectedId);n.el.dataset.prominent=String(n.prominent);n.el.dataset.dim=String(dim);n.el.style.opacity=dim?'.35':fade<1?(.62+.38*near01).toFixed(2):'1';n.el.setAttribute('aria-pressed',String(n.id===selectedId));n.el.style.setProperty('--star-color',color);
      n.el.hidden=!inView(box(p.x-5,p.y-5,10,10));
    });
    const taken=labelsForScene(near,focusEdges);
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
    if(!still&&(playing||unsettled||layoutMoving||drag||velocity||reveal>0||litGlow>0))kick();else last=0;
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
    const alpha=ctx.globalAlpha;ctx.fillStyle='#050b16';ctx.globalAlpha=Math.min(1,alpha+.2);ctx.beginPath();ctx.arc(x,y,Math.max(1.2,r-1.1),0,Math.PI*2);ctx.fill();ctx.globalAlpha=alpha;
    ctx.strokeStyle=color;ctx.lineWidth=hot?1.6:1.25;const steps=broken?10:14;
    for(let i=0;i<steps;i++){if(broken&&i%2)continue;const a=i/steps*Math.PI*2,b=(i+1)/steps*Math.PI*2;ctx.beginPath();ctx.moveTo(x+Math.cos(a)*r,y+Math.sin(a)*r);ctx.lineTo(x+Math.cos(b)*r,y+Math.sin(b)*r);ctx.stroke();}
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
  const zoomTo=value=>{tzoom=measured?clamp(value,1,measured.axes==='xy'?8:4):clamp(value,.65,2.2);kick();};
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
    const nextYaw=clamp(state.yaw+dx*.0025,...YAW),nextPitch=clamp(state.pitch+dy*.0025,...PITCH);
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
  canvas.addEventListener('pointermove',e=>{
    if(pinching(e))return;
    if(!drag){const p=pointer(e),id=hitEdge(p.x,p.y);if(hoverEdge!==id){hoverEdge=id;canvas.style.cursor=id?'pointer':'';onEdgeHover?.(id);kick();}return;}
    if(e.pointerId!==drag.id)return;const dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.hypot(dx,dy)<5&&!drag.moved)return;drag.moved=true;hoverEdge=null;dragBy(drag,dx,dy);
  });
  const release=e=>{
    if(!drag||e.pointerId!==drag.id)return;const click=e.type==='pointerup'&&!drag.moved&&!drag.forcePan;drag=null;canvas.classList.remove('dragging');letGo();
    if(click){const p=pointer(e),id=hitEdge(p.x,p.y);if(id)onEdgeSelect(id);}kick();
  };canvas.addEventListener('pointerup',release);canvas.addEventListener('pointercancel',release);canvas.addEventListener('lostpointercapture',release);
  canvas.addEventListener('pointerleave',()=>{if(!drag){if(hoverEdge)onEdgeHover?.(null);hoverEdge=null;canvas.style.cursor='';kick();}});
  const visibility=()=>{last=0;if(document.hidden){cancelAnimationFrame(raf);raf=0}else kick()};document.addEventListener('visibilitychange',visibility);
  canvas.addEventListener('sophia-renderer-restored',kick);
  const ro=new ResizeObserver(resize);ro.observe(root);paintNebula();resize();
  // Labels are measured in their type; once the page's fonts are ready they are measured again.
  document.fonts?.ready?.then(()=>{if(!disposed){measureLabels=true;kick();}});
  return {
    update:refresh,
    capturePose(){return validateSkyPose({v:1,yaw:tyaw,pitch:tpitch,zoom:tzoom,pan:[tpan.x,tpan.y],center:[...targetCenter],fit:targetFit});},
    restorePose(value){const pose=validateSkyPose(value);yaw=tyaw=pose.yaw;pitch=tpitch=pose.pitch;zoom=tzoom=pose.zoom;
      pan={x:pose.pan[0],y:pose.pan[1]};tpan={...pan};center=[...pose.center];targetCenter=[...pose.center];fitScale=targetFit=pose.fit;kick();},
    select(id){selectedId=id;updateThreads();kick()},
    selectEdge(id){selectedEdgeId=id;kick()},
    link(id){linkFrom=id;kick()},
    zoomBy(factor){if(Number.isFinite(factor)&&factor>0)zoomTo(tzoom*factor);},
    frame({fill=false,x=null,y=null,plane=false}={}){
      if(fill)resize();tstretchY=1;measured=null;
      if(nodes.length){
        const points=nodes.map(n=>n.target),min=[0,1,2].map(i=>Math.min(...points.map(p=>p[i]))),max=[0,1,2].map(i=>Math.max(...points.map(p=>p[i])));
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
      }
      // A measured layout is seen straight on, so the view's tilt does not bend its measure; a space rests in its own pose.
      velocity=null;tyaw=measured?0:restYaw;tpitch=measured?0:restPitch;tzoom=1;tpan={x:0,y:0};
      const introNow=intro&&!measured&&!introDone&&!reduced.matches;
      // The first framing is where the view begins, unless the space is to turn into it; later framings ease there.
      if(!framedOnce){framedOnce=true;if(rest&&!introNow){yaw=tyaw;pitch=tpitch;}}
      if(introNow){introDone=true;introPending=true;}
      resize()},
    /** Turn the view by a step, as a drag would; a measured layout does not turn. */
    turnBy(dyaw=0,dpitch=0){if(measured)return;velocity=null;introUntil=0;turnShownUntil=performance.now()+900;tyaw=clamp(tyaw+dyaw,...YAW);tpitch=clamp(tpitch+dpitch,...PITCH);kick();},
    /** Bring the view back to its resting pose. */
    turnToRest(){if(measured)return;velocity=null;introUntil=0;turnShownUntil=performance.now()+900;tyaw=restYaw;tpitch=restPitch;kick();},
    pose(){return {yaw,pitch,targetYaw:tyaw,targetPitch:tpitch,restYaw,restPitch,measured:!!measured};},
    focus(id){const n=nodes.find(n=>n.id===id);if(!n)return;targetCenter=[...n.target];tzoom=1.3;tpan={x:0,y:0};kick();},
    reveal(id){revealId=id??null;kick();},
    /** Light a way through the given stars, in order (null puts it out). */
    light(ids){lit=Array.isArray(ids)&&ids.length>1?[...ids]:null;kick();},
    refresh:resize,
    motion(value){playing=value;kick()},
    dispose(){disposed=true;cancelAnimationFrame(raf);ro.disconnect();document.removeEventListener('visibilitychange',visibility);for(const target of [canvas,layer,edgeLayer,clusterLayer])target.removeEventListener('wheel',wheel);canvas.removeEventListener('sophia-renderer-restored',kick);nodes.forEach(n=>n.el.remove());edges.forEach(e=>e.el.remove());clusters.forEach(c=>c.el.remove());planes.forEach(plane=>plane.el.remove());threads.forEach(thread=>thread.el.remove());ctx.dispose?.()}
  };
}
