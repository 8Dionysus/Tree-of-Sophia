// The near galaxies of the cosmos between pages (cosmos.js), computed pixel by pixel away from the page's own work:
// a bulge of old yellow light, an exponential disc, logarithmic arms whose light gathers in clumps, blue knots and
// pink nebulae where stars are born, dust lanes along the arms, and light compressed as the eye compresses it, so a
// bright core glows and never burns out to a blot.
'use strict';
function random(seed){var state=seed>>>0;return function(){state=(state+0x6D2B79F5)>>>0;var t=state;t=Math.imul(t^(t>>>15),t|1);t^=t+Math.imul(t^(t>>>7),t|61);return((t^(t>>>14))>>>0)/4294967296;};}
function noiseOf(seed){
  var next=random(seed),S=64,grid=new Float32Array(S*S);for(var i=0;i<S*S;i++)grid[i]=next();
  function at(x,y){var xi=Math.floor(x),yi=Math.floor(y),xf=x-xi,yf=y-yi;xi&=63;yi&=63;var x1=(xi+1)&63,y1=(yi+1)&63;
    var a=grid[yi*S+xi],b=grid[yi*S+x1],c=grid[y1*S+xi],d=grid[y1*S+x1],u=xf*xf*(3-2*xf),w=yf*yf*(3-2*yf);return a+(b-a)*u+(c-a)*w+(a-b-c+d)*u*w;}
  return function(x,y){return at(x,y)*.5+at(x*2.03+17,y*2.03+5)*.3+at(x*4.1+3,y*4.1+11)*.2;};
}
function spiral(px,o){
  var data=new Uint8ClampedArray(px*px*4),noise=noiseOf(o.seed),c=px/2,R=px*.48,wind=1/Math.tan(o.pitch*Math.PI/180);
  var rb=o.bulge,rd=o.disk,m=o.arms,ex=o.exposure,warm=o.bulgeColor,cool=o.armColor,kc=o.knotColor,hc=o.hiiColor;
  for(var y=0;y<px;y++)for(var x=0;x<px;x++){
    var dx=(x-c)/R,dy=(y-c)/R,r=Math.sqrt(dx*dx+dy*dy);if(r>=1)continue;
    var theta=Math.atan2(dy,dx),bulge=1.5*Math.exp(-Math.pow(r/rb,.7)*2.2),disc=Math.exp(-r/rd);
    var wave=m*(theta-wind*Math.log(r+.03)),arm=Math.pow((Math.cos(wave)+1)/2,2.2);
    // The arms are not ribbons: their light gathers in clumps, finer the farther out.
    var n=noise(dx*5+7,dy*5+3)*.65+noise(dx*13+1,dy*13+9)*.35,clump=Math.pow(n,3)*5.2,lane=Math.pow((Math.cos(wave+.8)+1)/2,9)*Math.min(1,r/(rb*1.4));
    var fade=r<.5?1:Math.max(0,1-(r-.5)/.5),edge=fade*fade*(3-2*fade);
    var light=disc*(.05+2.4*arm*clump)*(1-.65*lane)*edge,knot=Math.pow(Math.max(0,n-.6)/.4,3)*arm*disc*5*edge,hii=n>.72&&arm>.5?knot*.55:0;
    // Old yellow light near the core, young blue light out along the arms.
    var t=Math.min(1,Math.max(0,(r-.12)/.45)),s=t*t*(3-2*t),ar=warm[0]+(cool[0]-warm[0])*s,ag=warm[1]+(cool[1]-warm[1])*s,ab=warm[2]+(cool[2]-warm[2])*s;
    var red=bulge*warm[0]+light*ar+knot*kc[0]+hii*hc[0],green=bulge*warm[1]+light*ag+knot*kc[1]+hii*hc[1],blue=bulge*warm[2]+light*ab+knot*kc[2]+hii*hc[2];
    red=1-Math.exp(-red*ex);green=1-Math.exp(-green*ex);blue=1-Math.exp(-blue*ex);
    var a=Math.max(red,green,blue);if(a<.004)continue;
    var i=(y*px+x)*4;data[i]=red/a*255;data[i+1]=green/a*255;data[i+2]=blue/a*255;data[i+3]=a*255;
  }
  return data;
}
self.onmessage=function(event){
  var job=event.data,data=spiral(job.px,job.o);
  self.postMessage({id:job.id,px:job.px,data:data.buffer},[data.buffer]);
};
