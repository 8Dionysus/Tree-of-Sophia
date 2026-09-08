import {uiComputed,ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {createScrollIntent,controlGain} from './control-model.mjs';
import {relationPreview,describePreview} from './graph-preview.mjs';
import {validatePose} from './view-state.mjs';
import {sameSceneView} from './scene-history.mjs';
import {refreshIcons} from './icons';
import { projectLens,nodeLabels } from './knowledge-client.mjs';
import { attachKnowledgeUI } from './knowledge-ui.mjs';
import { SophiaGpuCanvas } from './gpu-canvas.js';

export function mountScene(root, {client,onChange=()=>{}, initialFocus,initialLens,autoStart=true}={}) {
  const q = s => root.querySelector(s);root.dataset.rendererBuild='c8d4575cf448b8707cdf55ae779d2743482a02f7348ae5e66f4972983bfe18cb';

  let canvas=q('.sc-sky'),ctx;
  try {ctx=new SophiaGpuCanvas(canvas);root.dataset.renderer='three-webgl2';}
  catch(error) {
    const fallback=canvas.cloneNode(true);canvas.replaceWith(fallback);canvas=fallback;
    ctx=canvas.getContext('2d',{alpha:false});root.dataset.renderer='canvas-fallback';
    console.warn('GPU renderer unavailable; accepted Canvas painter retained.',error);
  }

  if(!ctx) return;
  const reduced=matchMedia('(prefers-reduced-motion: reduce)');
  const design={atmosphere:'midnight',density:0.85,pixel:0.5,depth:1.25,liveliness:1,playing:!reduced.matches};
  const FOCAL=1250;
  let nodes=[],edges=[];
  const colors=['#89accc','#dfc28f','#a49fdb'];
  const lookup=new Map(),nodeLayer=q('.sc-nodes');
  function depthPosition(n){const z=n.volumeZ*design.depth;const ratio=(FOCAL-z)/(FOCAL-n.sourcePosition[2]);return[n.sourcePosition[0]*ratio,n.sourcePosition[1]*ratio,z]}
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
  let w=1,h=740,dpr=1,base=1,selected=-1,hover=-1,lens='constellations',yaw=0,pitch=-.055,tyaw=0,tpitch=-.055,zoom=1,tzoom=1,pan={x:0,y:0},tpan={x:0,y:0},time=0,skyTime=0,last=0,raf=0,visible=true,drag=null,settling=0,frameCount=0,drawSum=0,frameSum=0;
  let calm=0,cardTab='about',layoutDirty=true,overlayReturnPanel=false,windowDrag=null,pinch=null;
  let wheelLastAt=-Infinity,wheelDirection=0,wheelKind='',wheelResponsiveUntil=0,wheelEvents=0;
  let motionPreference='system';
  const scrollIntent=createScrollIntent();
  let scrollAction='auto',dragAction='rotate',sensitivity='normal',nativeGesture=null,nativeGestureUntil=-Infinity;
  const panGain=.55,panResponse=22;
  let windowPosition={x:0,y:0,manual:false},obstacles=[],panelBounds=null;
  const cardSections=new Map();
  let navigationHistory=null;
  const history=[],pointers=new Map(),clamp=(v,min,max)=>Math.max(min,Math.min(max,v));
  const eye={x:0,y:0,tx:0,ty:0};
  let cameraCos=1,cameraSin=0,cameraPitchCos=1,cameraPitchSin=0,skyCos=1,skySin=0,skyPitchCos=1,skyPitchSin=0;
  const panel=q('.sc-inspector');
  // Narrow, page-local bridge. Backend payloads cannot assign a camera pose.
  let scenePacket=null,relationRecords=[],hoverRelation=-1,keyboardRelation=0,visibleRelations=[],selectedRelation=null,suppressHistory=false,knowledgeUI=null;
  function captureGraph(){
    return {packet:scenePacket,vertices:nodes.map(n=>({id:n.id,slot:n.slot,p:n.p.slice(),sourcePosition:n.sourcePosition.slice(),volumeZ:n.volumeZ,pos:n.target.slice(),target:n.target.slice()})),selectedRelation};
  }
  function installGraph(packet,vertices=nodes,{restore=false}={}){
    const oldId=nodes[selected]?.id,oldById=new Map(nodes.map(n=>[n.id,n]));
    const presented=projectLens(packet,vertices);
    nodes=presented.map(n=>{
      const old=oldById.get(n.id);let b=old?.el,label=old?.label;
      if(!b){
        b=document.createElement('button');b.type='button';b.className='sc-node';b.dataset.id=n.id;
        label=document.createElement('span');label.className='sc-node-label';uiChildren(b, "append", label);
        const index=()=>lookup.get(n.id);
        b.addEventListener('click',e=>{if(e.detail<2&&index()!==undefined)select(index())});
        b.addEventListener('dblclick',()=>{if(index()!==undefined){remember();focus(index())}});
        b.addEventListener('pointerenter',()=>{hover=index()??-1;kick()});b.addEventListener('pointerleave',()=>{hover=-1;kick()});
        b.addEventListener('focus',()=>{hover=index()??-1;kick()});b.addEventListener('blur',()=>{hover=-1;kick()});
        b.addEventListener('keydown',e=>{if(index()!==undefined)moveNodeFocus(e,index())});
      }
      b.dataset.main=String(n.main);b.lang=n.labelLanguage||'';uiAttribute(b, 'aria-label', n.fullName+' — '+n.kind);uiText(label, n.name);
      return {...n,el:b,label,screen:project(...n.pos)};
    });
    lookup.clear();nodes.forEach((n,i)=>lookup.set(n.id,i));uiChildren(nodeLayer, "replaceChildren", ...nodes.map(n=>n.el));
    scenePacket=packet;relationRecords=packet.relations;edges=relationRecords.map(r=>[lookup.get(r.from_id),lookup.get(r.to_id)]);
    if(lens!=='constellations'){
      const retained=restore?new Set(vertices.map(n=>n.id)):null;
      const active=lookup.get(packet.focus?.node_id)??0,near=nearTo(active),far=nodes.map((n,i)=>i).filter(i=>i!==active&&!near.includes(i));
      nodes.forEach((n,i)=>{
        if(retained?.has(n.id))return;
        if(lens==='plane')n.target=[n.sourcePosition[0],n.sourcePosition[1],0];
        else if(i===active)n.target=[0,0,0];
        else{const ring=near.includes(i)?near:far,angle=ring.indexOf(i)/ring.length*Math.PI*2-.8,r=ring===near?175:380;n.target=[Math.cos(angle)*r,Math.sin(angle)*r*.68,Math.sin(angle*2)*140*design.depth];}
      });
    }
    selected=lookup.get(oldId)??-1;hover=-1;hoverRelation=-1;keyboardRelation=0;relationKeyLabel();
    if(!relationRecords.some(r=>r.id===selectedRelation))selectedRelation=null;
    root.dataset.graphRevision=packet.source_revision;root.dataset.graphFocus=packet.focus?.node_id||'';
    root.dataset.nodeCount=String(nodes.length);root.dataset.relationCount=String(edges.length);
    root.dataset.dataState='ready';
    measureLabels();updateSelection();updateContext();layoutDirty=true;settling=1;kick();
  }
  function restoreGraph(snapshot){
    if(!snapshot?.packet)return;
    knowledgeUI?.cancelPending();installGraph(snapshot.packet,snapshot.vertices,{restore:true});selectedRelation=snapshot.selectedRelation;
  }
  // Mouse hit testing stays on the sky, so a drag may begin on any edge.
  const relationKey=document.createElement('button');relationKey.type='button';relationKey.className='sc-relation-key';relationKey.hidden=true;uiChildren(root, "append", relationKey);
  function relationAt(e){
    const r=root.getBoundingClientRect(),x=(e.clientX-r.left)*w/r.width,y=(e.clientY-r.top)*h/r.height;
    let best=-1,distance=e.pointerType==='touch'?12:8;
    for(let i=0;i<edges.length;i++){
      const[a,b]=edges[i],p=nodes[a].screen,q=nodes[b].screen;if(!p.s||!q.s)continue;
      const dx=q.x-p.x,dy=q.y-p.y,len=dx*dx+dy*dy;
      const t=len?clamp(((x-p.x)*dx+(y-p.y)*dy)/len,0,1):0;
      if(t<.04||t>.96)continue;
      const d=Math.hypot(x-p.x-t*dx,y-p.y-t*dy);if(d<distance){distance=d;best=i;}
    }
    return best;
  }
  function relationHover(index,event){
    if(index===hoverRelation)return;hoverRelation=index;canvas.style.cursor=index<0?'':'pointer';
    if(index<0){delete canvas.dataset.tooltip;root.dispatchEvent(new CustomEvent('sophia-context-target'));}
    else if(event){
      describePreview(canvas,relationPreview(scenePacket,relationRecords[index]));
      canvas.dataset.tooltipPoint=JSON.stringify({left:event.clientX-1,top:event.clientY-1,right:event.clientX+1,bottom:event.clientY+1,width:2,height:2});
      root.dispatchEvent(new CustomEvent('sophia-context-target',{detail:{anchor:canvas}}));
    }
    root.dataset.hoverRelation=relationRecords[index]?.id||'';kick();
  }
  function pickRelation(e){const index=relationAt(e);if(index<0)return false;showRelation(relationRecords[index].id);return true;}
  function relationKeyLabel(){
    const record=relationRecords[keyboardRelation];if(!record)return;
    const preview=relationPreview(scenePacket,record);describePreview(relationKey,preview);uiAttribute(relationKey, 'aria-label', uiComputed(()=>[preview.title,preview.body,ui("Стрелки — другие связи; Enter — открыть")].join('. ')));
  }
  relationKey.addEventListener('focus',()=>{relationKeyLabel();relationHover(keyboardRelation);});
  relationKey.addEventListener('blur',()=>relationHover(-1));
  relationKey.addEventListener('click',()=>{if(relationRecords[keyboardRelation])showRelation(relationRecords[keyboardRelation].id);});
  relationKey.addEventListener('keydown',event=>{
    const step={ArrowLeft:-1,ArrowUp:-1,ArrowRight:1,ArrowDown:1}[event.key];if(!step)return;event.preventDefault();
    if(!visibleRelations.length)return;const current=visibleRelations.indexOf(keyboardRelation);keyboardRelation=visibleRelations[(current+step+visibleRelations.length)%visibleRelations.length];relationKeyLabel();relationHover(keyboardRelation);kick();
    root.dispatchEvent(new CustomEvent('sophia-context-target',{detail:{anchor:relationKey}}));
  });
  canvas.addEventListener('pointermove',event=>{if(!drag&&!event.buttons&&event.pointerType!=='touch')relationHover(relationAt(event),event);});
  canvas.addEventListener('pointerleave',event=>{if(document.activeElement!==relationKey&&!event.relatedTarget?.closest?.('.sc-context-hint'))relationHover(-1);});
  root.addEventListener('pointerover',event=>{if(hoverRelation>=0&&event.target!==canvas&&event.target!==relationKey&&!event.target.closest?.('.sc-context-hint'))relationHover(-1);});
  canvas.addEventListener('pointerdown',()=>relationHover(-1));
  function showRelation(id){
    const relation=relationRecords.find(r=>r.id===id);if(!relation)return;
    knowledgeUI?.willSelect();
    if(selectedRelation!==id||panel.hidden)remember();closeSearch(false,false);closeLenses(false,false);selectedRelation=id;
    selected=lookup.get(relation.from_id)??-1;panel.hidden=false;fillCard();setCardTab(cardSections.get(id)||'about');
    updateSelection();updateContext();placePanel(true);layoutDirty=true;kick();
    announce(ui("Отношение: {0}", [q('.sc-node-title').textContent]));q('#so-about-tab').focus();
  }
  const scenePort={
    setControls({scrollAction:scroll='auto',dragAction:drag='rotate',sensitivity:speed='normal',motion}){if(scrollAction!==scroll||dragAction!==drag||sensitivity!==speed){endWheelResponse();scrollIntent.reset();}scrollAction=scroll;dragAction=drag;sensitivity=speed;motionPreference=motion;design.playing=motion==='running'||motion==='system'&&!reduced.matches;syncInputMode();syncMotion();},
    setHistory(controller){navigationHistory=controller;history.length=0;},
    get packet(){return scenePacket;},
    get selection(){return {nodeId:nodes[selected]?.id||null,relationId:selectedRelation};},
    node:id=>nodes.find(n=>n.id===id)?.raw,
    relation:id=>relationRecords.find(r=>r.id===id),
    neighbors:id=>relationRecords.filter(r=>r.from_id===id||r.to_id===id),
    captureView:()=>captureView(),
    capturePlace:()=>validatePose({lens,yaw:tyaw,pitch:tpitch,zoom:tzoom,pan:tpan,selectedId:nodes[selected]?.id||null,relationId:selectedRelation,panelOpen:!panel.hidden,cardTab,vertices:captureGraph().vertices}),
    restorePlace(value){
      const state=validatePose(value);endWheelResponse();
      lens=state.lens;installGraph(scenePacket,state.vertices,{restore:true});selected=lookup.get(state.selectedId)??-1;selectedRelation=relationRecords.some(r=>r.id===state.relationId)?state.relationId:null;
      tyaw=state.yaw;tpitch=state.pitch;tzoom=state.zoom;tpan={...state.pan};updateLensUI();
      if(selected>=0||selectedRelation){fillCard();setCardTab(state.cardTab,{preserveCamera:true});panel.hidden=!state.panelOpen;placePanel(false);}else panel.hidden=true;
      updateSelection();updateContext();settling=1;kick();
    },
    refreshTypography(){measureLabels();layoutDirty=true;kick();},
    restoreView(state){if(!state?.graph?.packet)return;remember();restoreView(state);},
    setGraph(packet,{selectFocus=false,initial=false}={}){
      if(!initial)remember();
      knowledgeUI?.cancelInspector();
      installGraph(packet);
      if(selectFocus&&packet.focus){suppressHistory=true;try{select(lookup.get(packet.focus.node_id))}finally{suppressHistory=false;}}
      else if(selected>=0||selectedRelation)fillCard();
      else {panel.hidden=true;updateContext();}
    },
    selectNode(id){const i=lookup.get(id);if(i===undefined)return false;select(i);return true;},
    selectRelation(id,{rememberView=true}={}){const previous=suppressHistory;suppressHistory=!rememberView||previous;try{showRelation(id)}finally{suppressHistory=previous;}},
    cardChanged(){placePanel(false);layoutDirty=true;kick();},
    announce,
  };

  function resize(){const oldWidth=w,oldHeight=h;w=root.clientWidth;h=root.clientHeight;dpr=Math.min(devicePixelRatio||1,1.5);canvas.width=Math.round(w*dpr);canvas.height=Math.round(h*dpr);base=w<=540?w/720:Math.min(w/1190,1.06);dust.forEach(s=>{const midZ=s.minZ+s.range*.5;const spread=(FOCAL-midZ)/FOCAL;s.x=s.sx*w*.5/base*spread;s.y=s.sy*h*.5/base*spread});measureLabels();placePanel(oldWidth!==w);if((oldWidth!==w||oldHeight!==h)&&selected>=0&&!panel.hidden)focus(selected,false);layoutDirty=true;kick();}
  function project(x,y,z,background=false){
    const c=background?skyCos:cameraCos,s=background?skySin:cameraSin,cp=background?skyPitchCos:cameraPitchCos,sp=background?skyPitchSin:cameraPitchSin;
    const rx=x*c-z*s,rz=x*s+z*c,ry=y*cp-rz*sp,zz=y*sp+rz*cp;
    if(zz>FOCAL-110)return{x:-9999,y:-9999,s:0,depth:0,z:zz};
    const depth=FOCAL/(FOCAL-zz),factor=base*(background?1:zoom)*depth;
    return{x:w*.5+rx*factor+(background?pan.x*.18+eye.x*depth*23:pan.x),y:h*.54+ry*factor+(background?pan.y*.18+eye.y*depth*17:pan.y),s:factor,depth,z:zz};
  }
  function announce(text){uiText(q('.sc-announcement'), text);}
  function styleOnce(el,key,value){if(el.style[key]!==String(value))el.style[key]=value;}
  function nearTo(i){return edges.filter(e=>e.includes(i)).map(e=>e[0]===i?e[1]:e[0]);}
  function measureLabels(){nodes.forEach(n=>{const s=getComputedStyle(n.label);ctx.font=s.font;const spacing=parseFloat(s.letterSpacing)||0;n.labelWidth=Math.ceil(ctx.measureText(n.name).width+spacing*n.name.length)+2;n.labelHeight=Math.ceil(parseFloat(s.fontSize)*1.5);});}
  // Relabel existing DOM/vertices only. A navigation-language change must not
  // reinstall the graph, create history, or assign any camera or motion state.
  document.addEventListener('sophia-ui-language',()=>{
    if(!root.isConnected)return;
    for(const node of nodes){Object.assign(node,nodeLabels(node.raw));node.el.lang=node.labelLanguage||'';uiText(node.label,node.name);uiAttribute(node.el,'aria-label',node.fullName+' — '+node.kind);}
    measureLabels();relationKeyLabel();
    if(hoverRelation>=0)describePreview(canvas,relationPreview(scenePacket,relationRecords[hoverRelation]));
    updateContext(false);layoutDirty=true;kick();
  });
  function localBox(el){const r=el.getBoundingClientRect(),origin=root.getBoundingClientRect();return{x:r.left-origin.left,y:r.top-origin.top,w:r.width,h:r.height};}
  function overlap(a,b,gap=0){return a.x<b.x+b.w+gap&&a.x+a.w+gap>b.x&&a.y<b.y+b.h+gap&&a.y+a.h+gap>b.y;}
  function updateLayout(){obstacles=[];for(const el of root.querySelectorAll('.sc-header,.sc-context,.sc-footer,.sc-label-west,.sc-label-east,.sc-panel')){if(!el.hidden){const b=localBox(el);if(b.w&&b.h)obstacles.push(b)}}panelBounds=panel.hidden?null:localBox(panel);layoutDirty=false;}
  function placePanel(reposition=false){
    if(panel.hidden||panel.dataset.floating==='true')return;
    if(w<=540){for(const key of ['left','right','top','bottom','transform'])panel.style[key]='';}
    else{
      const pw=panel.offsetWidth,ph=panel.offsetHeight;
      if(reposition&&!windowPosition.manual&&selected>=0){const p=nodes[selected].screen;windowPosition.x=p.x+pw+60<w?p.x+46:p.x-pw-46;windowPosition.y=p.y-100;}
      windowPosition.x=clamp(windowPosition.x,16,w-pw-16);windowPosition.y=clamp(windowPosition.y,190,Math.max(190,h-ph-98));
      panel.style.left='0';panel.style.top='0';panel.style.right='auto';panel.style.bottom='auto';panel.style.transform='translate3d('+Math.round(windowPosition.x)+'px,'+Math.round(windowPosition.y)+'px,0)';
    }
    layoutDirty=true;kick();
  }
  function updateContext(notify=true){
    if(!navigationHistory)q('.sc-back').hidden=history.length===0;
    uiText(q('.sc-context-sub'), selectedRelation?ui("Выбрана связь"):selected>=0?ui("Фокус: {0}", [nodes[selected].name]):scenePacket?ui("{0} звёзд · {1} связей", [nodes.length, edges.length]):ui("Загружаем область…"));
    if(!navigationHistory)root.dataset.history=String(history.length);layoutDirty=true;if(notify)onChange(scenePort);
  }
  function captureView(){return {graph:captureGraph(),selected,panelOpen:!panel.hidden||overlayReturnPanel,lens,targets:nodes.map(n=>n.target.slice()),yaw:tyaw,pitch:tpitch,zoom:tzoom,pan:{...tpan},windowPosition:{...windowPosition},cardTab};}
  function remember(){if(suppressHistory)return;if(navigationHistory){navigationHistory.beforeChange();return;}const state=captureView();if(!sameSceneView(state,history.at(-1))){history.push(state);if(history.length>24)history.shift()}updateContext();}
  function back(){
    if(navigationHistory){navigationHistory.back();return;}
    const state=history.pop();if(!state)return;
    restoreView(state);
  }
  function restoreView(state){
    endWheelResponse();
    closeSearch(false,false);closeLenses(false,false);restoreGraph(state.graph);selected=state.selected;hover=-1;lens=state.lens;nodes.forEach((n,i)=>n.target=state.targets[i].slice());tyaw=state.yaw;tpitch=state.pitch;tzoom=state.zoom;tpan={...state.pan};windowPosition={...state.windowPosition};
    updateLensUI();if(selected>=0||selectedRelation){fillCard();setCardTab(state.cardTab,{preserveCamera:true});panel.hidden=!state.panelOpen;placePanel(false)}else panel.hidden=true;
    updateSelection();updateContext();announce(selected>=0?ui("Возврат: {0}", [nodes[selected].name]):ui("Возврат к предыдущему виду"));if(!history.length)q('.sc-overview').focus();settling=1;kick();
  }
  function focus(i,approach=true,{gentle=false}={}){
    endWheelResponse();
    const p=nodes[i].target,c=Math.cos(tyaw),s=Math.sin(tyaw),cp=Math.cos(tpitch),sp=Math.sin(tpitch),rx=p[0]*c-p[2]*s,rz=p[0]*s+p[2]*c,ry=p[1]*cp-rz*sp,zz=p[1]*sp+rz*cp;
    let x=w*.5,y=h*.5;
    if(!panel.hidden){const b=localBox(panel);if(w<=540){const context=localBox(q('.sc-context'));y=(context.y+context.h+35+b.y-38)*.5}else{x=b.x>w*.44?Math.max(w*.30,b.x*.55):Math.min(w*.76,(b.x+b.w+w)*.5);y=clamp(b.y+b.h*.44,250,h-150)}}
    tzoom=gentle?clamp(tzoom*1.08,.65,2.2):approach?Math.max(w<=540?1.25:1.45,tzoom):w<=540?1.12:Math.max(1.1,tzoom);tzoom=Math.min(2.2,tzoom);
    const f=base*tzoom*FOCAL/(FOCAL-zz);tpan.x=x-w*.5-rx*f;tpan.y=y-h*.54-ry*f;settling=1;kick();
  }
  function updateSelection(){root.dataset.selected=selected>=0?nodes[selected].id:'';nodes.forEach((n,i)=>uiAttribute(n.el, 'aria-pressed', String(i===selected)));}
  function fillCard(){
    const raw=selectedRelation?scenePort.relation(selectedRelation):nodes[selected]?.raw;
    if(raw)knowledgeUI?.showCard(selectedRelation?'relation':'node',raw);
  }
  function select(i,{fly=false,keepWindow=false}={}){
    knowledgeUI?.willSelect();const changed=selected!==i||Boolean(selectedRelation);if(changed||panel.hidden)remember();selectedRelation=null;const hadPanel=!panel.hidden;
    closeSearch(false,false);closeLenses(false,false);selected=i;fillCard();setCardTab(cardSections.get(nodes[i].id)||'about');panel.hidden=false;knowledgeUI?.restoreReading();updateSelection();updateContext();
    placePanel(!keepWindow&&(changed||!hadPanel));if(fly)focus(i,true);else if(changed||!hadPanel)focus(i,false,{gentle:true});else if(w<=540)focus(i,false);announce(nodes[i].kind+': '+nodes[i].name);layoutDirty=true;kick();
  }
  function closeInspector(returnFocus=true,clear=false){knowledgeUI?.captureReading();knowledgeUI?.cancelInspector();panel.hidden=true;overlayReturnPanel=false;if(returnFocus&&selected>=0&&!nodes[selected].el.hidden)nodes[selected].el.focus();if(clear){selectedRelation=null;selected=-1;hover=-1;updateSelection();updateContext()}layoutDirty=true;kick();}
  function closeSearch(focusBack=true,restore=true){knowledgeUI?.cancelSearch();const wasOpen=!q('.sc-search').hidden;q('.sc-search').hidden=true;uiAttribute(q('.sc-search-open'), 'aria-expanded', 'false');if(wasOpen){if(restore&&overlayReturnPanel&&selected>=0){panel.hidden=false;placePanel(false)}overlayReturnPanel=false}if(focusBack)q('.sc-search-open').focus();layoutDirty=true;kick();}
  function closeLenses(focusBack=true,restore=true){const wasOpen=!q('.sc-lenses').hidden;q('.sc-lenses').hidden=true;uiAttribute(q('.sc-lenses-open'), 'aria-expanded', 'false');if(wasOpen){if(restore&&overlayReturnPanel&&selected>=0){panel.hidden=false;placePanel(false)}overlayReturnPanel=false}if(focusBack)q('.sc-lenses-open').focus();layoutDirty=true;kick();}
  function openOverlay(kind){const el=q('.sc-'+kind);if(!el.hidden){kind==='search'?closeSearch():closeLenses();return}const restore=!panel.hidden||overlayReturnPanel;closeSearch(false,false);closeLenses(false,false);overlayReturnPanel=restore;panel.hidden=true;el.hidden=false;uiAttribute(q('.sc-'+kind+'-open'), 'aria-expanded', 'true');if(kind==='search'){search();q('#sc-query').focus()}else q('.sc-lens[aria-pressed="true"]').focus();layoutDirty=true;kick();}
  function search(){knowledgeUI?.search(q('#sc-query').value);}
  function setCardTab(value,{preserveCamera=false}={}){knowledgeUI?.captureReading();cardTab=value;const cardId=selectedRelation||nodes[selected]?.id;if(cardId){cardSections.delete(cardId);cardSections.set(cardId,value);if(cardSections.size>64)cardSections.delete(cardSections.keys().next().value);}root.querySelectorAll('.sc-card-tab').forEach(b=>{const active=b.id==='so-'+value+'-tab';uiAttribute(b, 'aria-selected', String(active));b.tabIndex=active?0:-1;});q('#so-about').hidden=value!=='about';q('#so-relations').hidden=value!=='relations';knowledgeUI?.restoreReading();placePanel(false);if(!preserveCamera&&w<=540&&selected>=0&&!panel.hidden)focus(selected,false);layoutDirty=true;kick();}
  function updateLensUI(){root.dataset.lens=lens;uiText(q('.sc-context h2'), lens==='orbits'?ui("Орбиты мысли"):lens==='plane'?ui("Карта связей"):ui("Созвездия мысли"));q('.sc-label-west').hidden=true;q('.sc-label-east').hidden=true;root.querySelectorAll('.sc-lens').forEach(b=>uiAttribute(b, 'aria-pressed', String(b.dataset.lens===lens)));layoutDirty=true;}
  function setLens(value){
    if(value===lens){closeLenses();return}endWheelResponse();remember();lens=value;const active=selected>=0?selected:0,near=nearTo(active),far=nodes.map((n,i)=>i).filter(i=>i!==active&&!near.includes(i));
    nodes.forEach((n,i)=>{if(value==='constellations')n.target=n.p.slice();else if(value==='plane')n.target=[n.sourcePosition[0],n.sourcePosition[1],0];else if(i===active)n.target=[0,0,0];else{const ring=near.includes(i)?near:far,angle=ring.indexOf(i)/ring.length*Math.PI*2-.8,r=ring===near?175:380;n.target=[Math.cos(angle)*r,Math.sin(angle)*r*.68,Math.sin(angle*2)*140*design.depth]}});
    updateLensUI();tyaw=0;tpitch=value==='plane'?0:-.055;tpan={x:0,y:0};tzoom=1;closeLenses();if(selected>=0&&!panel.hidden)focus(selected,false);announce(ui("Линза: {0}", [q('.sc-context h2').textContent]));settling=1;kick();
  }
  function overview(){endWheelResponse();remember();closeSearch(false,false);closeLenses(false,false);tyaw=0;tpitch=lens==='plane'?0:-.055;tpan={x:0,y:0};tzoom=1;settling=1;closeInspector(false,true);windowPosition.manual=false;announce(ui("Общий вид"));kick();}
  function endWheelResponse(){wheelResponsiveUntil=0;wheelLastAt=-Infinity;wheelDirection=0;wheelKind='';nativeGesture=null;nativeGestureUntil=-Infinity;}
  function syncInputMode(){
    root.dataset.scrollAction=scrollAction;root.dataset.dragAction=dragAction;
    uiText(q('.sc-gesture'), dragAction==='pan'?ui("Перетаскивание — сдвиг · Shift — вращение"):ui("Перетаскивание — вращение · Shift — сдвиг"));
    layoutDirty=true;
  }
  function zoomAt(next,x=w*.5,y=h*.54){next=clamp(next,.65,2.2);const ratio=next/tzoom;tpan.x=x-w*.5-(x-w*.5-tpan.x)*ratio;tpan.y=y-h*.54-(y-h*.54-tpan.y)*ratio;tzoom=next;settling=1;kick();}
  function syncMotion(){uiAttribute(q('.sc-motion'), "data-tooltip", design.playing?ui("Остановить фоновое движение звёзд. Навигация останется доступна."):ui("Возобновить фоновое движение звёзд."));root.dataset.motion=design.playing?'running':'paused';uiAttribute(q('.sc-motion'), 'aria-pressed', String(!design.playing));uiAttribute(q('.sc-motion'), 'aria-label', design.playing?ui("Приостановить движение"):ui("Включить движение"));uiHTML(q('.sc-motion'), design.playing?'<i data-lucide="pause" aria-hidden="true"></i>':'<i data-lucide="play" aria-hidden="true"></i>');refreshIcons();kick();}
  function moveNodeFocus(e,i){const direction={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[e.key];if(!direction)return;e.preventDefault();const p=nodes[i].screen;let best=-1,score=Infinity;nodes.forEach((n,j)=>{if(j===i||n.el.hidden)return;const dx=n.screen.x-p.x,dy=n.screen.y-p.y,dot=dx*direction[0]+dy*direction[1];if(dot<=0)return;const distance=dx*dx+dy*dy,s=distance/Math.max(1,dot)+Math.abs(dx*direction[1]-dy*direction[0])*1.5;if(s<score){score=s;best=j}});if(best>=0)nodes[best].el.focus();}
  q('.sc-back').addEventListener('click',back);
  q('.sc-close').addEventListener('click',()=>closeInspector());q('.sc-focus').addEventListener('click',()=>{if(selected>=0){remember();focus(selected)}});
  q('.sc-search-open').addEventListener('click',()=>openOverlay('search'));q('.sc-search-close').addEventListener('click',()=>closeSearch());q('#sc-query').addEventListener('input',search);
  q('.sc-lenses-open').addEventListener('click',()=>openOverlay('lenses'));q('.sc-lenses-close').addEventListener('click',()=>closeLenses());root.querySelectorAll('.sc-lens').forEach(b=>b.addEventListener('click',()=>setLens(b.dataset.lens)));
  q('.sc-overview').addEventListener('click',overview);q('.sc-plus').addEventListener('click',()=>{endWheelResponse();remember();zoomAt(tzoom*1.18)});q('.sc-minus').addEventListener('click',()=>{endWheelResponse();remember();zoomAt(tzoom/1.18)});q('.sc-motion').addEventListener('click',()=>{design.playing=!design.playing;motionPreference=design.playing?'running':'paused';syncMotion();root.dispatchEvent(new CustomEvent('sophia-controls-change',{detail:{motion:motionPreference}}))});
  root.querySelectorAll('.sc-card-tab').forEach((b,i)=>{b.addEventListener('click',()=>setCardTab(i?'relations':'about'));b.addEventListener('keydown',e=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(e.key)){e.preventDefault();const value=e.key==='Home'?'about':e.key==='End'?'relations':cardTab==='about'?'relations':'about';setCardTab(value);q('#so-'+value+'-tab').focus()}})});
  q('.sc-search').addEventListener('keydown',e=>{const results=[...root.querySelectorAll('.sc-result')],index=results.indexOf(e.target);if(e.target===q('#sc-query')&&e.key==='Enter'&&results[0]){e.preventDefault();results[0].click()}else if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();const next=index+(e.key==='ArrowDown'?1:-1);if(next<0)q('#sc-query').focus();else results[Math.min(next,results.length-1)]?.focus()}});
  root.addEventListener('keydown',e=>{if(e.key==='Escape'){if(!q('.sc-search').hidden)closeSearch();else if(!q('.sc-lenses').hidden)closeLenses();else if(!panel.hidden)closeInspector();e.stopPropagation()}if(e.key==='/'&&!e.target.closest('input,textarea,select,[contenteditable]')){e.preventDefault();openOverlay('search')}});
  canvas.addEventListener('pointerdown',e=>{if(e.button!==0)return;pointers.set(e.pointerId,{x:e.clientX,y:e.clientY});canvas.setPointerCapture(e.pointerId);if(pointers.size===1){endWheelResponse();drag={id:e.pointerId,x:e.clientX,y:e.clientY,yaw:tyaw,pitch:tpitch,pan:{...tpan},shift:dragAction==='pan'?!e.shiftKey:e.shiftKey,moved:false}}else if(pointers.size===2){const[a,b]=[...pointers.values()],r=root.getBoundingClientRect();tzoom=zoom;tpan={...pan};pinch={distance:Math.hypot(b.x-a.x,b.y-a.y),x:((a.x+b.x)*.5-r.left)*w/r.width,y:((a.y+b.y)*.5-r.top)*h/r.height};if(drag)drag.moved=true}});
  canvas.addEventListener('pointermove',e=>{if(!pointers.has(e.pointerId))return;pointers.set(e.pointerId,{x:e.clientX,y:e.clientY});if(pinch&&pointers.size>=2){const[a,b]=[...pointers.values()],r=root.getBoundingClientRect(),distance=Math.hypot(b.x-a.x,b.y-a.y),x=((a.x+b.x)*.5-r.left)*w/r.width,y=((a.y+b.y)*.5-r.top)*h/r.height;tpan.x+=x-pinch.x;tpan.y+=y-pinch.y;zoomAt(tzoom*distance/Math.max(1,pinch.distance),x,y);pinch={distance,x,y};wheelKind='pinch';wheelResponsiveUntil=performance.now()+180;recordNavigation()}else if(drag&&e.pointerId===drag.id){const dx=e.clientX-drag.x,dy=e.clientY-drag.y;drag.moved=drag.moved||Math.abs(dx)+Math.abs(dy)>4;if(drag.shift){const r=root.getBoundingClientRect();tpan.x=drag.pan.x+dx*w/r.width*panGain*controlGain(sensitivity);tpan.y=drag.pan.y+dy*h/r.height*panGain*controlGain(sensitivity);wheelKind='pan';wheelResponsiveUntil=performance.now()+180;recordNavigation()}else{tyaw=drag.yaw+dx*.0022*controlGain(sensitivity);tpitch=clamp(drag.pitch+dy*.0018*controlGain(sensitivity),-.7,.7)}}settling=1;kick()});
  function release(e){if(!pointers.has(e.pointerId))return;const moved=drag?.moved||pinch||e.type!=='pointerup';pointers.delete(e.pointerId);pinch=null;if(pointers.size){const[id,p]=[...pointers.entries()][0];drag={id,x:p.x,y:p.y,yaw:tyaw,pitch:tpitch,pan:{...tpan},shift:dragAction==='pan',moved:true}}else{drag=null;if(!moved&&!pickRelation(e)){if(selected>=0)remember();closeSearch(false,false);closeLenses(false,false);closeInspector(false,true)}}if(canvas.hasPointerCapture(e.pointerId))canvas.releasePointerCapture(e.pointerId);}
  canvas.addEventListener('pointerup',release);canvas.addEventListener('pointercancel',release);canvas.addEventListener('lostpointercapture',release);
  function navigableTarget(e){
    const target=e.target instanceof Element?e.target:e.target.parentElement;
    return Boolean(target&&!target.closest('.sc-panel, .sc-navigation, button:not(.sc-node), input, textarea, select, a, [contenteditable="true"]'));
  }
  function navigationPoint(e,fallback={x:w*.5,y:h*.54}){
    const r=root.getBoundingClientRect();
    return {x:Number.isFinite(e.clientX)?(e.clientX-r.left)*w/r.width:fallback.x,y:Number.isFinite(e.clientY)?(e.clientY-r.top)*h/r.height:fallback.y};
  }
  function beginNavigation(kind,now){
    if(now-wheelLastAt>220||kind!==wheelKind){tzoom=zoom;tpan={...pan};}
    wheelLastAt=now;wheelKind=kind;wheelResponsiveUntil=now+180;
  }
  function recordNavigation(){root.dataset.wheelKind=wheelKind;root.dataset.zoomTarget=tzoom.toFixed(6);root.dataset.panTarget=[tpan.x.toFixed(3),tpan.y.toFixed(3)].join(',');}
  function handleWheel(e){
    if(!navigableTarget(e))return;
    relationHover(-1);
    // Claim the first packet, including a horizontal or subpixel start, before
    // the browser can assign the wheel transaction to a surrounding scroller.
    if(e.cancelable)e.preventDefault();
    e.stopPropagation();
    root.dataset.wheelEvents=String(++wheelEvents);
    const mode=e.deltaMode,dx=e.deltaX,dy=e.deltaY,now=performance.now();
    if(!Number.isFinite(dx)||!Number.isFinite(dy))return;
    if(nativeGesture||(e.ctrlKey&&now<nativeGestureUntil))return;
    if(dx===0&&dy===0)return;
    const kind=scrollIntent(e,scrollAction,now);
    const unit=mode===1?32:mode===2?h:1;
    const direction=Math.sign(-dy);
    if(kind==='wheel'&&wheelKind===kind&&direction!==wheelDirection){tzoom=zoom;tpan={...pan};}
    beginNavigation(kind,now);wheelDirection=direction;
    if(kind==='pan'){
      const r=root.getBoundingClientRect();tpan.x-=dx*unit*w/r.width*panGain*controlGain(sensitivity);tpan.y-=dy*unit*h/r.height*panGain*controlGain(sensitivity);
      settling=1;kick();
    }else{
      // Chromium encodes a pinch as -100 * log(scale), not a mouse notch.
      // Keep the accepted pinch gain; preserve increments across frame boundaries.
      const step=kind==='pinch'?clamp(-dy*unit*.006,-Math.log(4),Math.log(4)):clamp(-dy*unit*.0016,-.18,.18);
      const p=navigationPoint(e);zoomAt(tzoom*Math.exp(step*controlGain(sensitivity)),p.x,p.y);
    }
    recordNavigation();
  }
  root.addEventListener('wheel',handleWheel,{passive:false,capture:true});
  function startNativeGesture(e){
    if(!navigableTarget(e)||!Number.isFinite(e.scale)||e.scale<=0)return;
    if(e.cancelable)e.preventDefault();e.stopPropagation();
    tzoom=zoom;tpan={...pan};const p=navigationPoint(e);
    nativeGesture={scale:e.scale,...p};beginNavigation('pinch',performance.now());
  }
  function changeNativeGesture(e){
    if(!nativeGesture||!Number.isFinite(e.scale)||e.scale<=0)return;
    if(e.cancelable)e.preventDefault();e.stopPropagation();
    const p=navigationPoint(e,nativeGesture);
    tpan.x+=p.x-nativeGesture.x;tpan.y+=p.y-nativeGesture.y;
    zoomAt(tzoom*Math.pow(e.scale/nativeGesture.scale,.6),p.x,p.y);
    nativeGesture={scale:e.scale,...p};wheelLastAt=performance.now();wheelKind='pinch';wheelResponsiveUntil=wheelLastAt+180;recordNavigation();
  }
  function endNativeGesture(e){if(!nativeGesture)return;if(e.cancelable)e.preventDefault();e.stopPropagation();nativeGesture=null;nativeGestureUntil=performance.now()+80;}
  root.addEventListener('gesturestart',startNativeGesture,{passive:false,capture:true});
  root.addEventListener('gesturechange',changeNativeGesture,{passive:false,capture:true});
  root.addEventListener('gestureend',endNativeGesture,{passive:false,capture:true});
  window.addEventListener('blur',()=>{nativeGesture=null;nativeGestureUntil=-Infinity;pointers.clear();drag=null;pinch=null;endWheelResponse()});
  root.addEventListener('pointermove',e=>{if(e.pointerType!=='mouse'||!design.playing)return;const r=root.getBoundingClientRect();eye.tx=clamp(((e.clientX-r.left)/w-.5)*2,-1,1);eye.ty=clamp(((e.clientY-r.top)/h-.5)*2,-1,1);kick()});
  root.addEventListener('pointerleave',()=>{eye.tx=0;eye.ty=0;kick()});
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
  function draw(now){raf=0;if(!root.isConnected||!visible||document.hidden){last=0;return}const start=performance.now();ctx.beginFrame?.();const elapsed=last?(now-last)/1000:1/60;const dt=Math.min(elapsed,.05);last=now;const ease=reduced.matches?1:1-Math.exp(-dt*14),zoomEase=reduced.matches?1:now<wheelResponsiveUntil?1-Math.exp(-dt*(wheelKind==='pan'?panResponse:30)):ease,calmTarget=panel.hidden?0:1;calm+=(calmTarget-calm)*(reduced.matches?1:1-Math.exp(-dt*3));if(design.playing){time+=dt;skyTime+=dt*design.liveliness*(1-calm*.65);const easing=1-Math.exp(-dt*4);eye.x+=(eye.tx-eye.x)*easing;eye.y+=(eye.ty-eye.y)*easing}yaw+=(tyaw-yaw)*ease;pitch+=(tpitch-pitch)*ease;zoom+=(tzoom-zoom)*zoomEase;pan.x+=(tpan.x-pan.x)*zoomEase;pan.y+=(tpan.y-pan.y)*zoomEase;let delta=Math.abs(tyaw-yaw)+Math.abs(tpitch-pitch)+Math.abs(tzoom-zoom)+Math.abs(pan.x-tpan.x)*.01+Math.abs(pan.y-tpan.y)*.01+Math.abs(calm-calmTarget);if(layoutDirty)updateLayout();
    cameraCos=Math.cos(yaw);cameraSin=Math.sin(yaw);cameraPitchCos=Math.cos(pitch);cameraPitchSin=Math.sin(pitch);
    const skyYaw=yaw+Math.sin(skyTime*.021)*.011,skyPitch=pitch+Math.cos(skyTime*.017)*.008;
    skyCos=Math.cos(skyYaw);skySin=Math.sin(skyYaw);skyPitchCos=Math.cos(skyPitch);skyPitchSin=Math.sin(skyPitch);
    ctx.setTransform(dpr,0,0,dpr,0,0);ctx.globalAlpha=1;ctx.globalCompositeOperation='source-over';ctx.drawImage(nebula,0,0,w,h);
    const count=Math.round(dust.length*design.density);paintStars(0,count);paintFog(fogBack,.2,.85);paintStars(1,count);paintFog(fogFront,2.4,.46);
    nodes.forEach(n=>{for(let a=0;a<3;a++){n.pos[a]+=(n.target[a]-n.pos[a])*ease;delta+=Math.abs(n.target[a]-n.pos[a])*.001}n.screen=project(...n.pos)});
    const active=hover>=0?hover:selected;const neighbors=new Set(active<0?[]:edges.filter(e=>e.includes(active)).flat());
    ctx.lineWidth=.7;edges.forEach(([a,b],edgeIndex)=>{
      const x=nodes[a].screen,y=nodes[b].screen;if(!x.s||!y.s)return;
      const pointed=edgeIndex===hoverRelation,hot=pointed||(selectedRelation?relationRecords[edgeIndex].id===selectedRelation:active>=0&&(a===active||b===active)),group=nodes[a].group===nodes[b].group;
      const color=hot?'#e8c88d':group?colors[nodes[a].group]:'#8ea8c3';
      const strength=hot?.76:active>=0?.07:group?.39:.17;
      const ax=Math.max(.18,Math.min(1,x.depth*.75)),ay=Math.max(.18,Math.min(1,y.depth*.75));
      const gradient=ctx.createLinearGradient(x.x,x.y,y.x,y.y);
      gradient.addColorStop(0,color+Math.round(strength*ax*255).toString(16).padStart(2,'0'));
      gradient.addColorStop(1,color+Math.round(strength*ay*255).toString(16).padStart(2,'0'));
      ctx.strokeStyle=gradient;ctx.lineWidth=(pointed?1.8:hot?1:.7)*Math.max(.6,Math.min(1.35,(x.depth+y.depth)/2));ctx.beginPath();ctx.moveTo(x.x,x.y);ctx.lineTo(y.x,y.y);ctx.stroke();
    });ctx.globalAlpha=1;
    const edgePoint=index=>{const[a,b]=edges[index],p=nodes[a].screen,q=nodes[b].screen;return {x:(p.x+q.x)/2,y:(p.y+q.y)/2,shown:p.s&&q.s};};
    visibleRelations=edges.map((_,index)=>index).filter(index=>{const p=edgePoint(index);return p.shown&&p.x>=20&&p.x<=w-20&&p.y>=94&&p.y<=h-90&&!obstacles.some(box=>overlap({x:p.x-14,y:p.y-14,w:28,h:28},box));});
    relationKey.hidden=!visibleRelations.length;
    if(visibleRelations.length){
      if(!visibleRelations.includes(keyboardRelation)){keyboardRelation=visibleRelations[0];relationKeyLabel();if(document.activeElement===relationKey)hoverRelation=keyboardRelation;}
      const p=edgePoint(keyboardRelation);styleOnce(relationKey,'transform','translate3d('+Math.round(p.x-14)+'px,'+Math.round(p.y-14)+'px,0)');
    }
    const occupied=[],takenHits=[];let labelCount=0;const priority=nodes.map((n,i)=>({n,i})).sort((a,b)=>(b.i===active?20:neighbors.has(b.i)?8:b.n.main?3:0)-(a.i===active?20:neighbors.has(a.i)?8:a.n.main?3:0));
    for(const{n,i}of priority){const p=n.screen,hot=i===active,near=neighbors.has(i),main=n.main;const haze=Math.max(.52,Math.min(1,p.depth*.86)),alpha=(active<0||hot||near?1:.36)*haze;const color=colors[n.group];const radius=(hot?5:i===0?4.7:main?3.1:1.9)*Math.max(.55,Math.min(p.s,1.45));ctx.globalAlpha=alpha;const glow=(hot?115:i===0?145:main?90:42)*Math.max(.55,p.s);ctx.drawImage(sprite(color),p.x-glow/2,p.y-glow/2,glow,glow);ctx.fillStyle=color;ctx.beginPath();ctx.arc(p.x,p.y,radius,0,Math.PI*2);ctx.fill();ctx.fillStyle='#fff9ea';const core=Math.max(1,radius*.45);ctx.fillRect(Math.round(p.x-core/2),Math.round(p.y-core/2),core,core);
      if(main||hot){const len=(hot?18:i===0?16:10)*Math.max(.7,p.s);ctx.globalAlpha=alpha*(.45+design.pixel*.45);ctx.strokeStyle=color;ctx.lineWidth=.65;ctx.beginPath();ctx.moveTo(p.x-len,p.y);ctx.lineTo(p.x+len,p.y);ctx.moveTo(p.x,p.y-len);ctx.lineTo(p.x,p.y+len);ctx.stroke();}
      if(hot){ctx.globalAlpha=.85;ctx.strokeStyle='#d8bc84';ctx.lineWidth=1;const r=20,l=5;ctx.beginPath();for(const[sx,sy]of[[-1,-1],[-1,1],[1,-1],[1,1]]){ctx.moveTo(p.x+sx*r,p.y+sy*(r-l));ctx.lineTo(p.x+sx*r,p.y+sy*r);ctx.lineTo(p.x+sx*(r-l),p.y+sy*r)}ctx.stroke();}
      ctx.globalAlpha=1;const hitSize=w<=540?44:38,hit={x:p.x-hitSize/2,y:p.y-hitSize/2,w:hitSize,h:hitSize};styleOnce(n.el,'transform','translate3d('+Math.round(hit.x)+'px,'+Math.round(hit.y)+'px,0)');styleOnce(n.el,'opacity',active<0||hot||near?'1':'.46');
      const onScreen=p.s>0&&hit.x>10&&hit.x+hit.w<w-10&&hit.y>92&&hit.y+hit.h<h-90;
      const obscured=obstacles.some(b=>overlap(hit,b,2)),crowded=takenHits.some(b=>overlap(hit,b,0));
      n.el.hidden=!onScreen||obscured||crowded;if(!n.el.hidden)takenHits.push(hit);
      const showDetails=hot||near||(active<0&&(main||zoom>(w<=540?1.5:1.28)))||(main&&zoom>1.8);
      let chosen=null;const lw=n.labelWidth,lh=n.labelHeight;
      if(!n.el.hidden&&showDetails){
        const below={x:p.x-lw/2,y:p.y+22,w:lw,h:lh},above={x:p.x-lw/2,y:p.y-23-lh,w:lw,h:lh},right={x:p.x+28,y:p.y-lh/2,w:lw,h:lh},left={x:p.x-28-lw,y:p.y-lh/2,w:lw,h:lh};
        const choices=n.above?[above,below,right,left]:[below,above,right,left];
        chosen=choices.find(b=>b.x>12&&b.x+b.w<w-12&&b.y>90&&b.y+b.h<h-86&&!obstacles.some(o=>overlap(b,o,7))&&!occupied.some(o=>overlap(b,o,7))&&!nodes.some((other,j)=>j!==i&&other.screen.s>0&&overlap(b,{x:other.screen.x-8,y:other.screen.y-8,w:16,h:16},3)));
      }
      styleOnce(n.label,'visibility',chosen?'visible':'hidden');
      if(chosen){styleOnce(n.label,'left',Math.round(chosen.x-hit.x)+'px');styleOnce(n.label,'top',Math.round(chosen.y-hit.y)+'px');styleOnce(n.label,'transform','none');occupied.push(chosen);labelCount++;}
    }
    paintStars(2,count);ctx.globalAlpha=1;ctx.endFrame?.();frameCount++;drawSum+=performance.now()-start;frameSum+=elapsed*1000;
    if(frameCount%90===0){root.dataset.drawMs=(drawSum/90).toFixed(2);root.dataset.frameMs=(frameSum/90).toFixed(2);root.dataset.stars=String(count);root.dataset.frames=String(frameCount);drawSum=0;frameSum=0;}
    root.dataset.skyClock=skyTime.toFixed(3);root.dataset.rotation=yaw.toFixed(3);root.dataset.zoom=zoom.toFixed(3);root.dataset.labels=String(labelCount);root.dataset.calm=calm.toFixed(2);root.dataset.camera=[yaw.toFixed(3),pitch.toFixed(3),zoom.toFixed(3),pan.x.toFixed(1),pan.y.toFixed(1)].join(',');
    settling=delta>(now<wheelResponsiveUntil?.00001:.005)?1:0;if(design.playing||settling||drag)raf=requestAnimationFrame(draw);else last=0;
  }
  canvas.addEventListener('sophia-renderer-restored',()=>{settling=1;kick()});
  function kick(){if(!raf&&visible&&!document.hidden)raf=requestAnimationFrame(draw)}
  document.addEventListener('visibilitychange',()=>{if(document.hidden){cancelAnimationFrame(raf);raf=0;last=0}else kick()});
  const observer=new IntersectionObserver(es=>{visible=es[0].isIntersecting;if(visible)kick();else{cancelAnimationFrame(raf);raf=0;last=0}});observer.observe(root);new ResizeObserver(resize).observe(root);
  reduced.addEventListener('change',e=>{if(motionPreference==='system'){design.playing=!e.matches;syncMotion()}});
  knowledgeUI=attachKnowledgeUI(root,scenePort,{client,initialFocus,initialLens});
  paintNebula();resize();syncMotion();syncInputMode();root.dataset.lens=lens;updateContext();if(autoStart)knowledgeUI.start();document.fonts?.ready.then(()=>{measureLabels();kick()});
  refreshIcons();
  return {port:scenePort, ui:knowledgeUI, openSearch:()=>openOverlay('search'), overview, closeInspector, invalidate:()=>{layoutDirty=true;kick()}};
}
