// The cosmos between pages: a deep field. While a bonsai's page gathers itself, the reader looks into the depth of the
// universe: hundreds of far galaxies reddened by their distance, a crowd of nearer ones of every kind, the stars of our
// own sky with their four rays, and a few near galaxies, each its own: the Whirlpool with its companion, a barred
// spiral, the flocculent Sunflower, the Sombrero with its ring of dust, Hoag's ring. The near ones and the crowd were
// drawn once by hand (public/cosmos/*.webp, by tools/cosmos_galaxies.py) and come up out of the depth one after another as
// their images are ready; the far field is stamped here before the first paint. The view drifts forward the whole
// time, each depth at its own pace, and the drift is the clock's: a sky called up on one page (tosCosmos.show()) goes
// on in the next where it stood. When the page is ready (tosCosmos.done()) and has a moment to spare, the view flies
// through the depths into the tree. Everything moves by transforms and opacity alone, so the sky goes on while the
// page's scripts load. The day (Noon) sees the same field as an astronomer's plate: the negative, in ink on paper.
// On a bonsai's page this script stands in the head, after theme.js and the sky's own style, so the sky is in the
// page's very first frame and the page that arrives never shows itself bare for a moment; the home page, which only
// calls the sky up, keeps it in its body.
(function(){
  'use strict';
  var script=document.currentScript,root=document.documentElement;
  if(!script||window.tosCosmos)return;
  var startOn=script.getAttribute('data-cosmos')!=='off',stand=script.getAttribute('data-stand')||'';
  var reduced=false;try{reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;}catch(error){}
  var DPR=window.devicePixelRatio||1,plates=(script.src||'').replace(/cosmos\.js(?:[?#].*)?$/,'')+'cosmos/';

  function random(seed){var state=seed>>>0;return function(){state=(state+0x6D2B79F5)>>>0;var t=state;t=Math.imul(t^(t>>>15),t|1);t^=t+Math.imul(t^(t>>>7),t|61);return((t^(t>>>14))>>>0)/4294967296;};}
  function make(tag,name,parent){var node=document.createElement(tag);if(name)node.className=name;if(parent)parent.appendChild(node);return node;}
  function rgba(c,a){return 'rgba('+Math.round(c[0])+','+Math.round(c[1])+','+Math.round(c[2])+','+Math.max(0,Math.min(1,a)).toFixed(3)+')';}
  function mix(a,b,t){return [a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t];}
  function pick(next,table){var r=next(),sum=0;for(var i=0;i<table.length;i++){sum+=table[i][1];if(r<=sum)return table[i][0];}return table[table.length-1][0];}
  function canvasOf(width,height){var canvas=document.createElement('canvas');canvas.width=Math.max(1,Math.round(width));canvas.height=Math.max(1,Math.round(height));return canvas;}
  function viewport(){return {w:window.innerWidth||1280,h:window.innerHeight||800};}
  // A canvas the size of the sky is drawn finer on a fine screen, within a budget the GPU keeps easily.
  function resolution(){var v=viewport();return Math.min(DPR,1.5,Math.sqrt(7e6/(v.w*v.h)));}
  // A loop stands where it stood on the last page: its phase is the clock's. A loop that goes there and back takes two
  // of its periods to come round.
  function phase(node,period,alternate){var now=Date.now()/1000,cycle=alternate?period*2:period;node.style.animationDuration=period+'s';node.style.animationDelay=(-(now%cycle)).toFixed(2)+'s';}
  function decoded(image){
    if(image.decode)return image.decode().then(function(){return true;},function(){return false;});
    return new Promise(function(done){if(image.complete&&image.naturalWidth)done(true);else{image.onload=function(){done(true);};image.onerror=function(){done(false);};}});
  }
  function idle(work,wait){return window.requestIdleCallback?{idle:requestIdleCallback(work,{timeout:wait})}:{timer:setTimeout(work,Math.min(wait,200))};}
  function unidle(handle){if(!handle)return;if(handle.idle&&window.cancelIdleCallback)cancelIdleCallback(handle.idle);if(handle.timer)clearTimeout(handle.timer);}

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

  // ── the far field is stamped from a few galaxies drawn once
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

  // ── the sky itself: six depths, from the farthest field to the stars of our own sky. Each depth drifts forward at
  // its own pace (its depth), and is carried through on its own plane when the view flies into the tree.
  var host=make('div','tos-cosmos');host.setAttribute('aria-hidden','true');host.dataset.state=startOn?'on':'off';
  var field=make('div','cosmos-field',host);
  make('div','cosmos-ground',field);
  var deep=make('div','cosmos-deep',field);
  function depth(name){return make('div','cosmos-depth',make('div','cosmos-plane cosmos-'+name,deep));}
  var far=depth('far'),middle=depth('middle'),drift=depth('drift'),crowd=depth('crowd'),near=depth('near'),stars=depth('stars'),DEPTHS=[far,middle,drift,crowd,near,stars];
  make('div','cosmos-veil',field);
  var heart=make('div','cosmos-heart',host);
  make('div','cosmos-hush',heart);
  var halo=make('div','cosmos-halo',heart),orbit=make('div','cosmos-orbit',heart);make('i','',orbit);
  var mark=make('div','cosmos-mark',heart),title=make('div','cosmos-title',heart);

  var TITLES={zarathustra:{ru:'Заратустра',en:'Zarathustra',es:'Zaratustra',de:'Zarathustra','zh-Hans':'查拉图斯特拉'},
    antiquity:{ru:'Античность',en:'Antiquity',es:'Antigüedad',de:'Antike','zh-Hans':'古代'},
    aesthetics:{ru:'Эстетика',en:'Aesthetics',es:'Estética',de:'Ästhetik','zh-Hans':'美学'}};
  // The bonsai that have their own tree beside the sky (trees/<id>.svg): the heart of the sky shows it in place of
  // the Tree's mark (cosmos.css).
  var TREES=['zarathustra','antiquity','aesthetics'];
  function localeNow(){
    var value=null;try{value=new URLSearchParams(location.search).get('lang')||localStorage.getItem('tos.public-research.locale.v1');}catch(error){}
    value=value||navigator.language||'en';
    if(/^ru/i.test(value))return 'ru';if(/^es/i.test(value))return 'es';if(/^de/i.test(value))return 'de';if(/^zh/i.test(value))return 'zh-Hans';return 'en';
  }
  function titleFor(id){var names=TITLES[id];return names?names[localeNow()]||names.en:'';}

  // The far field and the middle one: hundreds of galaxies, stamped at once, before the first paint.
  function paintField(layer,seed,count,sizes,skew,palette,points,scale){
    layer.textContent='';
    var v=viewport(),canvas=make('canvas','',layer);canvas.width=Math.round(v.w*scale);canvas.height=Math.round(v.h*scale);
    var g=canvas.getContext('2d');if(!g)return;
    var s=spritesNow(),next=random(seed);g.globalCompositeOperation='lighter';
    for(var i=0;i<points;i++){var r=.7+next()*1.2;stamp(g,next()<.6?s.point.white:s.point.pale,next()*v.w,next()*v.h,r,r,0,.2+next()*.6,scale);}
    for(var j=0;j<count;j++){
      var x=next()*v.w,y=next()*v.h,size=sizes[0]+Math.pow(next(),skew)*(sizes[1]-sizes[0]);
      if(size>sizes[0]*2.5&&clear(x,y,v.w,v.h))continue;
      small(g,next,x,y,size,palette,scale);
    }
    g.setTransform(1,0,0,1,0,0);g.globalAlpha=1;
  }
  // The small stars between us and the field fly toward the reader. A few sheets of them, each drawn once, come out of the
  // depth one after another and spread from the heart of the sky as they near, the faster the nearer, until they pass by;
  // the flight is the clock's, so it goes on from one page into the next.
  var SHEETS=5,FLIGHT=34;
  function paintDrift(){
    drift.textContent='';
    var v=viewport(),scale=Math.min(1,Math.sqrt(1e6/(v.w*v.h))),s=spritesNow(),count=Math.round(175*v.w*v.h/1e6),hole=Math.min(v.w,v.h)*.07,now=Date.now()/1000;
    var kinds=[s.point.white,s.point.white,s.point.white,s.point.pale,s.point.pale,s.point.gold];
    for(var k=0;k<SHEETS;k++){
      var sheet=make('i','cosmos-sheet',drift),canvas=make('canvas','',sheet);canvas.width=Math.round(v.w*scale);canvas.height=Math.round(v.h*scale);
      sheet.style.animationDuration=FLIGHT+'s';sheet.style.animationDelay=(-((now+k*FLIGHT/SHEETS)%FLIGHT)).toFixed(2)+'s';
      var g=canvas.getContext('2d');if(!g)continue;g.globalCompositeOperation='lighter';
      var next=random(700+k*31);
      for(var i=0;i<count;i++){
        var x=next()*v.w,y=next()*v.h,bright=next()<.08,r=bright?1.6+next()*1.2:.55+Math.pow(next(),2)*1.35,alpha=bright?.95:.4+next()*.55,kind=kinds[Math.floor(next()*kinds.length)];
        // The Tree's mark keeps a clear sky of its own: no flying star sets out from it.
        if(Math.abs(x-v.w*.5)<hole&&Math.abs(y-v.h*.46)<hole)continue;
        stamp(g,kind,x,y,r,r,0,alpha,scale);
      }
      g.setTransform(1,0,0,1,0,0);g.globalAlpha=1;
    }
  }
  function paintStars(){
    stars.textContent='';
    var next=random(4242),images=[starSprite([200,220,255]),starSprite([255,236,206])];if(!images[0])return;
    for(var i=0;i<11;i++){
      var x=next()*100,y=next()*100;if(Math.abs(x-50)<15&&Math.abs(y-46)<17)x=(x+33)%100;
      var star=make('i','cosmos-star',stars),size=14+Math.pow(next(),1.6)*40;
      star.style.left=x.toFixed(2)+'%';star.style.top=y.toFixed(2)+'%';star.style.width=star.style.height=size.toFixed(0)+'px';
      star.style.margin=(-size/2).toFixed(0)+'px 0 0 '+(-size/2).toFixed(0)+'px';star.style.backgroundImage='url('+images[next()<.65?0:1]+')';
      phase(star,3.5+next()*4.5,true);
    }
  }

  // ── the near galaxies, each its own, placed and turned by hand. A disc turns slowly in its own plane (its tilt and
  // the foreshortening of its plane stay); the Sombrero, seen edge on, only drifts.
  var HEROES=[
    {name:'whirlpool',x:.765,y:.61,tall:[.7,.71],size:.5,tilt:-16,flat:.86,turn:420,drift:56},
    {name:'barred',x:.165,y:.27,tall:[.26,.24],size:.36,tilt:34,flat:.56,turn:340,back:true,drift:47},
    {name:'sunflower',x:.885,y:.155,tall:[.84,.1],size:.23,tilt:-48,flat:.66,turn:300,drift:43},
    {name:'sombrero',x:.355,y:.875,tall:[.36,.9],size:.4,ratio:.5,tilt:-7,drift:52},
    {name:'hoag',x:.09,y:.67,tall:[.13,.6],size:.16,flat:.92,turn:380,back:true,drift:39}
  ];
  // They come up out of the depth one after another, each once its image is ready, so none appears at a jump.
  function paintHeroes(){
    near.textContent='';
    // A tall screen (a phone held upright) has them placed for its own shape, clear of the title.
    var v=viewport(),vmin=Math.min(v.w,v.h),tall=v.h>v.w*1.1,order=Promise.resolve(),slot=0;
    HEROES.forEach(function(spec){
      var width=Math.round(vmin*spec.size),height=Math.round(width*(spec.ratio||1)),at=tall?spec.tall:[spec.x,spec.y];
      var hero=make('div','cosmos-hero',near);
      hero.style.left=(at[0]*100).toFixed(1)+'%';hero.style.top=(at[1]*100).toFixed(1)+'%';hero.style.width=width+'px';hero.style.height=height+'px';
      hero.style.margin=(-height/2).toFixed(0)+'px 0 0 '+(-width/2).toFixed(0)+'px';phase(hero,spec.drift,true);
      var rise=make('div','cosmos-rise',hero),tilt=make('div','cosmos-tilt',rise),spin=make('div','cosmos-spin',tilt);
      tilt.style.transform='rotate('+(spec.tilt||0)+'deg)'+(spec.flat?' scaleY('+spec.flat+')':'');
      if(spec.turn){phase(spin,spec.turn);if(spec.back)spin.classList.add('is-back');}else spin.classList.add('is-fixed');
      var image=make('img','',spin);image.alt='';image.decoding='async';image.draggable=false;image.src=plates+spec.name+'.webp';
      var ready=decoded(image);
      order=order.then(function(){return ready;}).then(function(ok){
        if(!hero.isConnected)return;
        if(!ok){hero.remove();return;}
        var now=performance.now();slot=Math.max(now,slot+280);
        rise.style.animationDelay=((slot-now)/1000).toFixed(2)+'s';rise.classList.add('is-risen');
      });
    });
  }
  // The crowd: the twenty-five small galaxies of the atlas, each laid down once or twice at its own size, angle and
  // foreshortening, between the far field and the near galaxies.
  var ATLAS=5;
  function paintCrowd(){
    crowd.textContent='';
    var atlas=new Image();atlas.decoding='async';atlas.src=plates+'atlas.webp';
    decoded(atlas).then(function(ok){
      if(!ok||!crowd.isConnected||host.dataset.state==='off')return;
      var v=viewport(),scale=resolution(),canvas=make('canvas','',crowd);canvas.width=Math.round(v.w*scale);canvas.height=Math.round(v.h*scale);
      var g=canvas.getContext('2d');if(!g)return;
      if('imageSmoothingQuality' in g)g.imageSmoothingQuality='high';
      // They keep their size to the screen's, as the near galaxies do, within bounds.
      var next=random(303),cell=atlas.naturalWidth/ATLAS,count=Math.round(Math.max(18,Math.min(46,30*v.w*v.h/1.4e6))),deck=[];
      var zoom=Math.max(.55,Math.min(1.2,Math.min(v.w,v.h)/950));
      for(var k=0;k<ATLAS*ATLAS;k++)deck.push(k);
      for(var n=deck.length-1;n>0;n--){var m=Math.floor(next()*(n+1)),swap=deck[n];deck[n]=deck[m];deck[m]=swap;}
      for(var i=0,tries=0;i<count&&tries<count*4;tries++){
        var x=next()*v.w,y=next()*v.h,size=(24+Math.pow(next(),2.4)*72)*zoom,angle=next()*6.2832,flat=.5+next()*.5,alpha=.62+next()*.38;
        if(clear(x,y,v.w,v.h))continue;
        var index=deck[i%deck.length],cos=Math.cos(angle),sin=Math.sin(angle);
        g.globalAlpha=alpha;g.setTransform(cos*scale,sin*scale,-sin*flat*scale,cos*flat*scale,x*scale,y*scale);
        g.drawImage(atlas,(index%ATLAS)*cell,Math.floor(index/ATLAS)*cell,cell,cell,-size/2,-size/2,size,size);
        i++;
      }
      g.setTransform(1,0,0,1,0,0);g.globalAlpha=1;
      canvas.classList.add('is-risen');
    });
  }

  function release(){
    [far,middle,drift,crowd].forEach(function(layer){layer.querySelectorAll('canvas').forEach(function(canvas){canvas.width=canvas.height=0;});});
    DEPTHS.forEach(function(layer){layer.textContent='';});
  }
  // The flight forward goes on from the moment the sky came up (`since`), on this page or on the one before it.
  var since=Date.now();
  function fly(){
    var elapsed=Math.max(0,Date.now()-since)/1000,moving=DEPTHS.concat(title);
    moving.forEach(function(node){node.style.animation='none';});void host.offsetWidth;
    moving.forEach(function(node){node.style.animation='';});
    DEPTHS.forEach(function(layer){layer.style.animationDelay=(-elapsed).toFixed(2)+'s';});
    title.style.animationDelay=(.3-elapsed).toFixed(2)+'s';
  }
  // `whole`: the near galaxies and the crowd come up too. A page that calls the sky up on its way out shows the field
  // alone, since a page loading behind the sky would lose them for a moment; the page that arrives raises them.
  function light(id,whole){
    var v=viewport(),area=v.w*v.h/1e6,scale=resolution();
    paintField(far,101,Math.round(620*area),[1.3,7],2.3,FAR,Math.round(220*area),scale);
    paintField(middle,202,Math.round(130*area),[3,15],2.1,NEAR,Math.round(60*area),scale);
    paintDrift();paintStars();crowd.textContent='';near.textContent='';
    title.textContent=titleFor(id||stand);
    if(TREES.indexOf(id||stand)>=0)host.dataset.tree=id||stand;else delete host.dataset.tree;
    fly();phase(orbit,3.6);phase(halo,5.5,true);phase(mark,6,true);
    if(whole){paintHeroes();paintCrowd();}
  }
  if(reduced)host.classList.add('is-still');
  // A sky that a page called up before leaving goes on here from where it stood.
  var CARRY='tos.cosmos.carry';
  try{var at=Number(sessionStorage.getItem(CARRY));sessionStorage.removeItem(CARRY);if(startOn&&at>0&&Date.now()-at<10000)since=at;}catch(error){}
  // In the head the body is not there yet: the sky stands beside it, as the root's own child.
  if(document.body)document.body.insertBefore(host,script.nextSibling);else root.appendChild(host);
  if(startOn)light(stand,true);

  var leaving=0,waiting=null;
  function stay(){clearTimeout(leaving);unidle(waiting);waiting=null;}
  var api={
    /** Call the sky up before leaving for a bonsai; resolves once it covers the page. */
    show:function(options){
      options=options||{};stay();
      var was=host.dataset.state;
      if(was!=='on'){since=Date.now();host.dataset.state='rising';light(options.stand,false);void host.offsetWidth;host.dataset.state='on';}
      if(options.title)title.textContent=options.title;
      try{sessionStorage.setItem(CARRY,String(since));}catch(error){}
      return new Promise(function(resolve){setTimeout(resolve,was==='on'?0:reduced?120:520);});
    },
    /** The page is ready: at its first quiet moment the view flies through the depths and opens onto the tree. */
    done:function(){
      if(host.dataset.state!=='on'||waiting)return;
      waiting=idle(function(){
        waiting=null;if(host.dataset.state!=='on')return;
        host.dataset.state='leaving';
        leaving=setTimeout(function(){host.dataset.state='off';release();},reduced?500:1700);
      },800);
    },
    /** Put the sky away at once (a page brought back from the history). */
    reset:function(){stay();host.dataset.state='off';release();}
  };
  window.tosCosmos=api;
  // A page restored from the history is already itself: the sky that led away from it is not left standing over it.
  window.addEventListener('pageshow',function(event){if(event.persisted)api.reset();});
  // A sky that waited too long gives way all the same: the page shows its own state, an error included.
  if(startOn)setTimeout(function(){api.done();},45000);
  // The page that only calls the sky up (the home page) fetches the near galaxies while the reader looks round, so the
  // page the reader goes to finds them at hand.
  if(!startOn){
    var kept=[];
    var fetchPlates=function(){HEROES.map(function(spec){return spec.name;}).concat('atlas').forEach(function(name){var image=new Image();image.decoding='async';if('fetchPriority' in image)image.fetchPriority='low';image.src=plates+name+'.webp';kept.push(image);});};
    var whenQuiet=function(){idle(fetchPlates,4000);};
    if(document.readyState==='complete')whenQuiet();else window.addEventListener('load',whenQuiet,{once:true});
  }
})();
