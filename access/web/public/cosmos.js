// The cosmos between pages: a deep field. While a bonsai's page gathers itself, the reader looks into the depth of the
// universe, hundreds of galaxies of every kind, the far ones reddened by their distance, a few near ones large enough
// to show their arms, their bulges, their dust and the blue knots where stars are born, and in front of them the
// stars of our own sky with their four rays. The view drifts slowly forward into the depth; when the page is ready it
// flies through and opens onto the tree (tosCosmos.done()); a page leaving for a bonsai calls the sky up first
// (tosCosmos.show()). A classic script in the body, like theme.js in the head, so the sky is in the page's first
// paint. The field is stamped from a few galaxies drawn once; the near spirals are computed in a worker
// (cosmos-worker.js), away from the page's own work; and everything moves by transforms and opacity alone, so the
// sky goes on while the page's scripts load. The day (Noon) sees the same field as an astronomer's plate: the
// negative, in ink on warm paper.
(function(){
  'use strict';
  var script=document.currentScript,root=document.documentElement;
  if(!script||!document.body||window.tosCosmos)return;
  var startOn=script.getAttribute('data-cosmos')!=='off',stand=script.getAttribute('data-stand')||'';
  var reduced=false;try{reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;}catch(error){}
  var DPR=window.devicePixelRatio||1,workerUrl=(script.src||'').replace(/cosmos\.js(?:[?#].*)?$/,'cosmos-worker.js');

  function random(seed){var state=seed>>>0;return function(){state=(state+0x6D2B79F5)>>>0;var t=state;t=Math.imul(t^(t>>>15),t|1);t^=t+Math.imul(t^(t>>>7),t|61);return((t^(t>>>14))>>>0)/4294967296;};}
  function make(tag,name,parent){var node=document.createElement(tag);if(name)node.className=name;if(parent)parent.appendChild(node);return node;}
  function rgba(c,a){return 'rgba('+Math.round(c[0])+','+Math.round(c[1])+','+Math.round(c[2])+','+Math.max(0,Math.min(1,a)).toFixed(3)+')';}
  function mix(a,b,t){return [a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t];}
  function pick(next,table){var r=next(),sum=0;for(var i=0;i<table.length;i++){sum+=table[i][1];if(r<=sum)return table[i][0];}return table[table.length-1][0];}
  function canvasOf(width,height){var canvas=document.createElement('canvas');canvas.width=Math.max(1,Math.round(width));canvas.height=Math.max(1,Math.round(height));return canvas;}
  // A loop of `period` seconds stands where it stood on the last page: its phase is the clock's.
  function phase(node,period){var now=Date.now()/1000;node.style.animationDuration=period+'s';node.style.animationDelay=(-(now%period)).toFixed(2)+'s';}

  // ── the colours of a deep field: old stars burn orange and gold, young ones white and blue; the farthest are reddened
  var C={red:[255,104,58],orange:[255,140,52],amber:[255,178,86],gold:[255,214,150],white:[255,244,230],pale:[196,216,255],blue:[118,168,255],pink:[255,150,180]};
  var FAR=[['red',.16],['orange',.3],['amber',.2],['gold',.1],['white',.1],['pale',.09],['blue',.05]];
  var NEAR=[['orange',.2],['amber',.14],['gold',.14],['white',.2],['pale',.2],['blue',.12]];

  // ── a smooth galaxy, elliptical or a disc seen from afar: light falling off from a bright centre (a Sérsic-like profile)
  var PROFILE=[[0,1],[.06,.9],[.14,.66],[.26,.4],[.42,.19],[.62,.07],[.82,.02],[1,0]];
  function smooth(g,x,y,radius,flat,angle,color,core,alpha){
    g.save();g.translate(x,y);g.rotate(angle);g.scale(1,flat);
    var light=g.createRadialGradient(0,0,0,0,0,radius);
    for(var i=0;i<PROFILE.length;i++){var t=PROFILE[i][0];light.addColorStop(t,rgba(mix(core,color,Math.min(1,t*4)),PROFILE[i][1]*alpha));}
    g.fillStyle=light;g.beginPath();g.arc(0,0,radius,0,6.2832);g.fill();g.restore();
  }
  function glow(g,x,y,radius,color){var light=g.createRadialGradient(x,y,0,x,y,radius);light.addColorStop(0,rgba(C.white,1));light.addColorStop(.16,rgba(color,.6));light.addColorStop(1,rgba(color,0));g.fillStyle=light;g.fillRect(x-radius,y-radius,radius*2,radius*2);}

  // ── the field is stamped from a few galaxies drawn once
  var HALF=32,sprites=null;
  function sprite(draw){var canvas=canvasOf(HALF*2,HALF*2),g=canvas.getContext('2d');if(g){g.globalCompositeOperation='lighter';draw(g,HALF);}return canvas;}
  function spritesNow(){
    if(sprites)return sprites;
    var next=random(9);sprites={round:{},disc:[],clumps:[],point:{}};
    Object.keys(C).forEach(function(name){var color=C[name];sprites.round[name]=sprite(function(g,c){smooth(g,c,c,c,1,0,color,mix(color,C.white,.7),1);});
      sprites.point[name]=sprite(function(g,c){glow(g,c,c,c,color);});});
    for(var v=0;v<3;v++)sprites.disc.push(sprite(function(g,c){
      smooth(g,c,c,c,1,0,mix(C.pale,C.gold,v*.18),C.pale,.7);smooth(g,c,c,c*.36,1,0,C.gold,C.white,1);
      for(var k=0;k<3+v;k++){var r=c*(.35+next()*.45),t=next()*6.2832;smooth(g,c+Math.cos(t)*r,c+Math.sin(t)*r,c*.14,1,0,C.blue,C.pale,.85);}
    }));
    for(var w=0;w<3;w++)sprites.clumps.push(sprite(function(g,c){for(var j=0;j<4+w;j++)smooth(g,c+(next()-.5)*c*1.1,c+(next()-.5)*c*.9,c*(.2+next()*.25),.6+next()*.4,next()*3,C.blue,C.pale,.95);}));
    return sprites;
  }
  // One stamp: a sprite laid at (x,y) with radii rx and ry, turned by angle, in a canvas of the given resolution.
  function stamp(g,image,x,y,rx,ry,angle,alpha,scale){
    var cos=Math.cos(angle),sin=Math.sin(angle),kx=scale*rx/HALF,ky=scale*ry/HALF;
    g.globalAlpha=alpha;g.setTransform(cos*kx,sin*kx,-sin*ky,cos*ky,x*scale,y*scale);g.drawImage(image,-HALF,-HALF);
  }
  // A small galaxy of any kind, as the field is full of them.
  function small(g,next,x,y,size,palette,scale){
    var s=spritesNow(),kind=next(),color=pick(next,palette),angle=next()*Math.PI;
    if(kind<.38){var f=.45+next()*.55;stamp(g,s.round[color],x,y,size*1.6,size*1.6*f,angle,.35,scale);stamp(g,s.round[color],x,y,size,size*f,angle,1,scale);return;}
    if(kind<.58){stamp(g,s.round[color],x,y,size*1.3,size*1.3*(.1+next()*.14),angle,1,scale);stamp(g,s.round.gold,x,y,size*.42,size*.21,angle,.85,scale);return;}
    if(kind<.86){stamp(g,s.disc[Math.floor(next()*3)],x,y,size*1.3,size*1.3*(.3+next()*.6),angle,1,scale);return;}
    // An irregular one: a few blue clumps that have not settled into a shape.
    stamp(g,s.clumps[Math.floor(next()*3)],x,y,size*1.2,size,angle,1,scale);
  }
  // The heart of the sky is kept clear for the Tree's mark.
  function clear(x,y,w,h){return Math.abs(x/w-.5)<.13&&Math.abs(y/h-.46)<.15;}

  // ── the near galaxies that are cheap to draw here: a disc edge on, a merging pair, a starburst
  function edgeOn(width,height,seed){
    var canvas=canvasOf(width,height),g=canvas.getContext('2d');if(!g)return canvas;
    var next=random(seed),cx=width/2,cy=height/2;g.globalCompositeOperation='lighter';
    smooth(g,cx,cy,width*.48,.11,0,C.pale,C.white,.95);smooth(g,cx,cy,width*.2,.45,0,C.gold,C.white,.9);
    for(var k=0;k<14;k++)glow(g,cx+(next()-.5)*width*.8,cy+(next()-.5)*height*.08,1.4+next()*2,C.pale);
    // A dark lane of dust along its middle.
    g.globalCompositeOperation='destination-out';smooth(g,cx,cy+height*.012,width*.42,.022,0,[0,0,0],[0,0,0],.75);
    return canvas;
  }
  function pair(size,seed){
    var canvas=canvasOf(size,size),g=canvas.getContext('2d');if(!g)return canvas;
    var next=random(seed),c=size/2;g.globalCompositeOperation='lighter';
    smooth(g,c,c,size*.5,.85,.4,C.orange,C.amber,.55);
    smooth(g,c-size*.12,c-size*.05,size*.2,.8,.2,C.amber,C.white,.95);smooth(g,c+size*.11,c+size*.06,size*.15,.75,1.1,C.orange,C.white,.9);
    for(var k=0;k<6;k++)glow(g,c+(next()-.5)*size*.7,c+(next()-.5)*size*.6,1.4+next()*1.8,C.amber);
    return canvas;
  }
  function burst(size,seed){
    var canvas=canvasOf(size,size),g=canvas.getContext('2d');if(!g)return canvas;
    var next=random(seed),c=size/2;g.globalCompositeOperation='lighter';
    smooth(g,c,c,size*.5,1,0,C.blue,C.pale,.5);smooth(g,c,c,size*.16,1,0,C.pale,C.white,1);
    for(var k=0;k<10;k++)glow(g,c+(next()-.5)*size*.5,c+(next()-.5)*size*.5,1.6+next()*2,C.blue);
    return canvas;
  }
  // Resolved stars along a near spiral's arms: the speckle that makes it sharp.
  function speckle(g,px,o){
    var s=spritesNow(),next=random(o.seed+9),c=px/2,R=px*.48,wind=1/Math.tan(o.pitch*Math.PI/180);g.globalCompositeOperation='lighter';
    for(var i=0;i<px*.9;i++){
      var rr=.12+next()*.75,tt=next()*6.2832,on=Math.pow((Math.cos(o.arms*(tt-wind*Math.log(rr+.03)))+1)/2,3);if(on<.45)continue;
      var r=1+next()*2;stamp(g,next()<.8?s.point.pale:s.point.pink,c+Math.cos(tt)*rr*R,c+Math.sin(tt)*rr*R,r,r,0,(.25+.5*next())*on,1);
    }
    g.setTransform(1,0,0,1,0,0);g.globalAlpha=1;
  }
  // A star of our own sky, with the four rays of the telescope (and of the Tree's own stars).
  function starSprite(color){
    var size=96,canvas=canvasOf(size,size),g=canvas.getContext('2d');if(!g)return '';
    var c=size/2;g.globalCompositeOperation='lighter';
    var light=g.createRadialGradient(c,c,0,c,c,c*.42);light.addColorStop(0,'rgba(255,255,255,1)');light.addColorStop(.1,rgba(color,.95));light.addColorStop(.32,rgba(color,.2));light.addColorStop(1,rgba(color,0));
    g.fillStyle=light;g.fillRect(0,0,size,size);
    for(var turn=0;turn<2;turn++){
      g.save();g.translate(c,c);g.rotate(turn*Math.PI/2);
      var ray=g.createLinearGradient(-c,0,c,0);ray.addColorStop(0,rgba(color,0));ray.addColorStop(.38,rgba(color,.35));ray.addColorStop(.5,'rgba(255,255,255,.95)');ray.addColorStop(.62,rgba(color,.35));ray.addColorStop(1,rgba(color,0));
      g.fillStyle=ray;g.fillRect(-c,-.7,size,1.4);g.restore();
    }
    try{return canvas.toDataURL();}catch(error){return '';}
  }

  // ── the worker that computes the near spirals
  var worker=null,jobs=0,pending={};
  function galaxyWorker(){
    if(worker!==null)return worker;
    try{worker=new Worker(workerUrl);worker.onmessage=function(event){var done=pending[event.data.id];delete pending[event.data.id];if(done)done(event.data);};worker.onerror=function(){worker=false;};}
    catch(error){worker=false;}
    return worker;
  }

  // ── the sky itself
  var host=make('div','tos-cosmos');host.setAttribute('aria-hidden','true');host.dataset.state=startOn?'on':'off';
  var field=make('div','cosmos-field',host);
  make('div','cosmos-ground',field);
  var deep=make('div','cosmos-deep',field);
  var far=make('div','cosmos-depth cosmos-far',deep),middle=make('div','cosmos-depth cosmos-middle',deep),near=make('div','cosmos-depth cosmos-near',deep),stars=make('div','cosmos-depth cosmos-stars',deep);
  make('div','cosmos-veil',field);
  var heart=make('div','cosmos-heart',host);
  make('div','cosmos-hush',heart);make('div','cosmos-halo',heart);
  var orbit=make('div','cosmos-orbit',heart);make('i','',orbit);
  make('div','cosmos-mark',heart);
  var title=make('div','cosmos-title',heart);

  var TITLES={zarathustra:{ru:'Заратустра',en:'Zarathustra',es:'Zaratustra',de:'Zarathustra','zh-Hans':'查拉图斯特拉'},
    antiquity:{ru:'Античность',en:'Antiquity',es:'Antigüedad',de:'Antike','zh-Hans':'古代'}};
  function localeNow(){
    var value=null;try{value=new URLSearchParams(location.search).get('lang')||localStorage.getItem('tos.public-research.locale.v1');}catch(error){}
    value=value||navigator.language||'en';
    if(/^ru/i.test(value))return 'ru';if(/^es/i.test(value))return 'es';if(/^de/i.test(value))return 'de';if(/^zh/i.test(value))return 'zh-Hans';return 'en';
  }
  function titleFor(id){var names=TITLES[id];return names?names[localeNow()]||names.en:'';}

  // The far field and the middle one: hundreds of galaxies, stamped at once, before the first paint.
  function paintField(layer,seed,count,sizes,skew,palette,points,scale){
    layer.textContent='';
    var w=window.innerWidth||1280,h=window.innerHeight||800,canvas=make('canvas','',layer);canvas.width=Math.round(w*scale);canvas.height=Math.round(h*scale);
    var g=canvas.getContext('2d');if(!g)return;
    var s=spritesNow(),next=random(seed);g.globalCompositeOperation='lighter';
    for(var i=0;i<points;i++){var r=.7+next()*1.2;stamp(g,next()<.6?s.point.white:s.point.pale,next()*w,next()*h,r,r,0,.2+next()*.6,scale);}
    for(var j=0;j<count;j++){
      var x=next()*w,y=next()*h,size=sizes[0]+Math.pow(next(),skew)*(sizes[1]-sizes[0]);
      if(size>sizes[0]*2.5&&clear(x,y,w,h))continue;
      small(g,next,x,y,size,palette,scale);
    }
    g.setTransform(1,0,0,1,0,0);g.globalAlpha=1;
  }
  var HEROES=[
    {kind:'spiral',x:.74,y:.6,size:.44,tilt:-18,flat:.74,spin:210,arms:2,pitch:17,bulge:.24,disk:.42,exposure:3,seed:11,
      bulgeColor:[1,.8,.52],armColor:[.55,.72,1],knotColor:[.42,.62,1],hiiColor:[1,.45,.62]},
    {kind:'spiral',x:.2,y:.3,size:.26,tilt:28,flat:.46,spin:170,reverse:true,arms:2,pitch:22,bulge:.26,disk:.38,exposure:3.2,seed:23,
      bulgeColor:[1,.84,.6],armColor:[.62,.76,1],knotColor:[.5,.66,1],hiiColor:[1,.5,.66]},
    {kind:'spiral',x:.9,y:.17,size:.14,tilt:-50,flat:.6,spin:150,arms:3,pitch:25,bulge:.26,disk:.38,exposure:3.3,seed:37,
      bulgeColor:[1,.82,.58],armColor:[.6,.74,1],knotColor:[.5,.66,1],hiiColor:[1,.5,.66]},
    {kind:'pair',x:.61,y:.22,size:.17,seed:41},
    {kind:'burst',x:.23,y:.76,size:.12,seed:53},
    {kind:'edge',x:.42,y:.86,size:.22,tilt:-24,seed:61},
    {kind:'edge',x:.08,y:.55,size:.12,tilt:62,seed:67}
  ];
  function lit(wrap){requestAnimationFrame(function(){wrap.classList.add('is-lit');});}
  function paintHero(spec){
    var vmin=Math.min(window.innerWidth||1280,window.innerHeight||800),size=Math.round(vmin*spec.size),edge=spec.kind==='edge';
    var wrap=make('div','cosmos-hero',near);wrap.style.left=(spec.x*100).toFixed(1)+'%';wrap.style.top=(spec.y*100).toFixed(1)+'%';
    wrap.style.width=size+'px';wrap.style.height=(edge?Math.round(size*.3):size)+'px';wrap.style.margin=(-(edge?size*.15:size/2)).toFixed(0)+'px 0 0 '+(-size/2).toFixed(0)+'px';
    var tilt=make('div','cosmos-tilt',wrap),spin=make('div','cosmos-spin',tilt);phase(wrap,40+spec.seed);
    if(spec.kind!=='spiral'){
      var res=Math.min(DPR,1.5);spin.classList.add('is-fixed');
      if(edge)tilt.style.transform='rotate('+spec.tilt+'deg)';
      spin.appendChild(edge?edgeOn(size*res,size*.3*res,spec.seed):spec.kind==='pair'?pair(size*res,spec.seed):burst(size*res,spec.seed));lit(wrap);return;
    }
    tilt.style.transform='rotate('+spec.tilt+'deg) scaleY('+spec.flat+')';if(spec.reverse)spin.classList.add('is-reverse');phase(spin,spec.spin);
    var compute=galaxyWorker();if(!compute){wrap.remove();return;}
    var id=++jobs,px=Math.round(size*Math.min(DPR,1.5));
    pending[id]=function(result){
      if(!wrap.isConnected)return;
      var canvas=canvasOf(result.px,result.px),g=canvas.getContext('2d');if(!g)return;
      g.putImageData(new ImageData(new Uint8ClampedArray(result.data),result.px,result.px),0,0);speckle(g,result.px,spec);
      spin.appendChild(canvas);lit(wrap);
    };
    compute.postMessage({id:id,px:px,o:spec});
  }
  function paintStars(){
    stars.textContent='';
    var next=random(4242),images=[starSprite([200,220,255]),starSprite([255,236,206])];if(!images[0])return;
    for(var i=0;i<11;i++){
      var x=next()*100,y=next()*100;if(Math.abs(x-50)<15&&Math.abs(y-46)<17)x=(x+33)%100;
      var star=make('i','cosmos-star',stars),size=14+Math.pow(next(),1.6)*40;
      star.style.left=x.toFixed(2)+'%';star.style.top=y.toFixed(2)+'%';star.style.width=star.style.height=size.toFixed(0)+'px';
      star.style.margin=(-size/2).toFixed(0)+'px 0 0 '+(-size/2).toFixed(0)+'px';star.style.backgroundImage='url('+images[next()<.65?0:1]+')';
      phase(star,4+next()*5);
    }
  }
  function release(){
    [far,middle,near].forEach(function(layer){layer.querySelectorAll('canvas').forEach(function(canvas){canvas.width=canvas.height=0;});layer.textContent='';});
    stars.textContent='';pending={};if(worker){worker.terminate();worker=null;}
  }
  function light(id){
    host.dataset.sky=root.getAttribute('data-site-theme')==='noon'?'noon':'midnight';
    // Each time the sky comes up the flight begins again from where the view stands.
    [far,middle,near,stars].forEach(function(layer){layer.style.animation='none';void layer.offsetWidth;layer.style.animation='';});
    var area=(window.innerWidth||1280)*(window.innerHeight||800)/1e6;
    paintField(far,101,Math.round(620*area),[1.3,7],2.3,FAR,Math.round(220*area),Math.min(DPR,1.5));
    paintField(middle,202,Math.round(150*area),[3.5,24],2.1,NEAR,Math.round(60*area),Math.min(DPR,2));
    paintStars();near.textContent='';HEROES.forEach(paintHero);
    title.textContent=titleFor(id||stand);
    phase(orbit,3.6);
  }
  if(reduced)host.classList.add('is-still');
  // A sky that a page called up before leaving goes on here without coming up again.
  var CARRY='tos.cosmos.carry',carried=false;
  try{var at=Number(sessionStorage.getItem(CARRY));sessionStorage.removeItem(CARRY);carried=startOn&&at>0&&Date.now()-at<10000;}catch(error){}
  if(carried)host.classList.add('is-carried');
  document.body.insertBefore(host,script.nextSibling);
  if(startOn)light(stand);

  var leaving=0;
  var api={
    /** Call the sky up before leaving for a bonsai; resolves once it covers the page. */
    show:function(options){
      options=options||{};clearTimeout(leaving);
      var was=host.dataset.state;
      if(was!=='on'){host.dataset.state='rising';light(options.stand);void host.offsetWidth;host.dataset.state='on';}
      if(options.title)title.textContent=options.title;
      try{sessionStorage.setItem(CARRY,String(Date.now()));}catch(error){}
      return new Promise(function(resolve){setTimeout(resolve,was==='on'?0:reduced?120:520);});
    },
    /** The page is ready: the view flies through the field and opens onto the tree. */
    done:function(){
      if(host.dataset.state!=='on')return;
      host.dataset.state='leaving';
      leaving=setTimeout(function(){host.dataset.state='off';release();},reduced?500:1700);
    },
    /** Put the sky away at once (a page brought back from the history). */
    reset:function(){clearTimeout(leaving);host.dataset.state='off';release();}
  };
  window.tosCosmos=api;
  // A page restored from the history is already itself: the sky that led away from it is not left standing over it.
  window.addEventListener('pageshow',function(event){if(event.persisted)api.reset();});
  // A sky that waited too long gives way all the same: the page shows its own state, an error included.
  if(startOn)setTimeout(function(){api.done();},45000);
})();
