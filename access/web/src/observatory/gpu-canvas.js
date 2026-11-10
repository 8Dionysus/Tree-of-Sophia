// A deliberately small painter for the accepted observatory, not a general
// Canvas implementation. Camera, gestures, texture authorship and DOM stay owned
// by the reference scene. Draw order and encoded-sRGB compositing are explicit.
// It speaks WebGL2 directly: one program, one buffer of quads, one atlas texture.
const STRIDE = 11;
// Each quad has four corners and two triangles, (0,2,1) and (0,3,2): the screen's Y is inverted by the vertex shader,
// so this winding faces the viewer.
const TRIANGLES = [0,2,1,0,3,2];
const ATLAS = 2048;
const VERTEX = `
precision highp float;
in vec2 aPosition;
in vec2 aLocal;
in vec4 aColor;
in vec3 aShape;
uniform vec2 uResolution;
out vec2 vLocal;
out vec4 vColor;
out vec3 vShape;
void main() {
  gl_Position = vec4(aPosition.x/uResolution.x*2.-1., 1.-aPosition.y/uResolution.y*2., 0., 1.);
  vLocal=aLocal; vColor=aColor; vShape=aShape;
}`;
const FRAGMENT = `
precision highp float;
uniform sampler2D uAtlas;
in vec2 vLocal;
in vec4 vColor;
in vec3 vShape;
out vec4 outColor;
float halfPlaneCoverage(float d,vec2 derivative) {
  vec2 n=abs(derivative);
  float a=max(n.x,n.y),b=min(n.x,n.y);
  if(b<.0001)return clamp(d/max(a,.0001)+.5,0.,1.);
  float z=clamp(d+(a+b)*.5,0.,a+b);
  if(z<b)return z*z/(2.*a*b);
  if(z<a)return (z-b*.5)/a;
  float tail=a+b-z;
  return 1.-tail*tail/(2.*a*b);
}
float stripCoverage(float coordinate,float halfExtent,vec2 derivative) {
  return halfPlaneCoverage(halfExtent-coordinate,derivative)-halfPlaneCoverage(-halfExtent-coordinate,derivative);
}
void main() {
  if(vShape.x<.5) {
    // Atlas bytes and vertex colors deliberately remain encoded sRGB to match
    // Canvas2D blending. Texture samples are premultiplied before interpolation.
    outColor=texture(uAtlas,vLocal)*vColor;
  } else {
    float coverage;
    if(vShape.x<1.5) {
      // Integrate a thin line across the pixel square. A linear fwidth ramp
      // dims diagonals relative to the accepted Canvas painter.
      coverage=stripCoverage(vLocal.x,vShape.y,vec2(dFdx(vLocal.x),dFdy(vLocal.x)))
        *stripCoverage(vLocal.y,vShape.z,vec2(dFdx(vLocal.y),dFdy(vLocal.y)));
    } else if(vShape.x<2.5) {
      float dist=length(vLocal)-vShape.y;
      coverage=clamp(.5-dist,0.,1.);
    } else if(vShape.x<3.5) {
      // The focus marker has mitered L corners. Draw their union once, retaining
      // the outer corner and avoiding alpha accumulation between the two legs.
      float len=vShape.y,hw=vShape.z,center=(len-hw)*.5,extent=(len+hw)*.5;
      vec2 dx=vec2(dFdx(vLocal.x),dFdy(vLocal.x));
      vec2 dy=vec2(dFdx(vLocal.y),dFdy(vLocal.y));
      float sx=stripCoverage(vLocal.x,hw,dx),sy=stripCoverage(vLocal.y,hw,dy);
      coverage=stripCoverage(vLocal.x-center,extent,dx)*sy
        +sx*stripCoverage(vLocal.y-center,extent,dy)-sx*sy;
    } else if(vShape.x<4.5) {
      vec2 dx=vec2(dFdx(vLocal.x),dFdy(vLocal.x));
      vec2 dy=vec2(dFdx(vLocal.y),dFdy(vLocal.y));
      float sx=stripCoverage(vLocal.x,vShape.z,dx),sy=stripCoverage(vLocal.y,vShape.z,dy);
      coverage=stripCoverage(vLocal.x,vShape.y,dx)*sy
        +sx*stripCoverage(vLocal.y,vShape.y,dy)-sx*sy;
    } else if(vShape.x<5.5) {
      // A ring of radius y and half width z: the coverage across its band, integrated over the pixel as a strip's is.
      float across=length(vLocal)-vShape.y;
      vec2 n=normalize(vLocal+vec2(1e-6));
      vec2 d=vec2(dot(vec2(dFdx(vLocal.x),dFdx(vLocal.y)),n),dot(vec2(dFdy(vLocal.x),dFdy(vLocal.y)),n));
      coverage=stripCoverage(across,vShape.z,d);
    } else if(vShape.x<7.5) {
      // A run of dashes along x, one every z from 0, each the fraction x-6 of z long, y across: the nearest dash, as a
      // quad of its own would draw it.
      float period=vShape.z,dash=(vShape.x-6.)*period*.5,k=floor((vLocal.x-dash)/period+.5);
      coverage=stripCoverage(vLocal.x-k*period-dash,dash,vec2(dFdx(vLocal.x),dFdy(vLocal.x)))
        *stripCoverage(vLocal.y,vShape.y,vec2(dFdx(vLocal.y),dFdy(vLocal.y)));
    } else {
      // A row of round dots along x, one every z from 0, of radius y: the nearest dot, as a disc of its own would draw it.
      float k=floor(vLocal.x/vShape.z+.5);
      coverage=clamp(.5-(length(vec2(vLocal.x-k*vShape.z,vLocal.y))-vShape.y),0.,1.);
    }
    outColor=vColor*coverage;
  }
}`;

// A star in one record: its glow (an atlas image), its disc and white core, and four rays in steps that narrow and fade
// towards their tips. The vertex shader lays from the record the quads the painter lays for each of them in turn
// (drawImage, a disc's fill, a segment's stroke), and the fragment shader is the painter's own.
const STAR_STRIDE = 24;
const STAR_QUADS = 15;
const STAR_VERTEX = `
precision highp float;
in vec4 aCenter;
in vec4 aGlow;
in vec4 aGlowUv;
in vec4 aColor;
in vec4 aRays;
in vec4 aMore;
uniform vec2 uResolution;
uniform vec4 uSteps[3];
out vec2 vLocal;
out vec4 vColor;
out vec3 vShape;
const int CORNERS[6]=int[6](0,2,1,0,3,2);
const vec2 DIRECTIONS[4]=vec2[4](vec2(1.,0.),vec2(-1.,0.),vec2(0.,1.),vec2(0.,-1.));
void main() {
  int quad=gl_VertexID/6,corner=CORNERS[gl_VertexID-quad*6];
  vec2 side=vec2(corner==1||corner==2?1.:-1.,corner>=2?1.:-1.);
  float scale=aCenter.z;
  vec2 position=vec2(-1e5);
  vLocal=vec2(0.);vColor=vec4(0.);vShape=vec3(0.);
  if(quad==0) {
    if(aGlow.z>0.) {
      vec2 at=step(0.,side);
      position=aCenter.xy+(aGlow.xy+at*aGlow.z)*scale;
      vLocal=aGlowUv.xy+at*aGlowUv.zw;vColor=vec4(aGlow.w);
    }
  } else if(quad<3) {
    float radius=(quad==1?aCenter.w:aRays.x)*scale,extent=radius+1.,light=quad==1?aColor.a*aMore.x:aMore.x;
    vLocal=side*extent;position=aCenter.xy+vLocal;
    vColor=vec4((quad==1?aColor.rgb:vec3(1.,249./255.,234./255.))*light,light);vShape=vec3(2.,radius,radius);
  } else {
    int k=(quad-3)/4;vec4 s=uSteps[k];vec2 u=DIRECTIONS[quad-3-k*4];
    float from=max(s.x,aRays.z);
    if(aRays.w>0.&&from<s.y) {
      float reach=aRays.y*scale,halfLength=reach*(s.y-from)*.5,halfWidth=s.w*aMore.y*scale*.5,light=aColor.a*aRays.w*s.z;
      vec2 center=aCenter.xy+u*reach*(s.y+from)*.5;
      vLocal=side*vec2(halfLength+1.,halfWidth+1.);
      position=center+vec2(u.x*vLocal.x-u.y*vLocal.y,u.y*vLocal.x+u.x*vLocal.y);
      vColor=vec4(aColor.rgb*light,light);vShape=vec3(1.,halfLength,halfWidth);
    }
  }
  gl_Position = vec4(position.x/uResolution.x*2.-1., 1.-position.y/uResolution.y*2., 0., 1.);
}`;

// The day's paper: the scene as the night drew it, turned over by the page's colour filter one step at a time, each
// step clamped as the page's filter clamps it, then laid on the page's paper as a multiply blend lays it.
const PAPER_VERTEX = `
void main() {
  gl_Position = vec4(gl_VertexID==1?3.:-1., gl_VertexID==2?3.:-1., 0., 1.);
}`;
const PAPER_FRAGMENT = `
precision highp float;
uniform sampler2D uScene;
uniform mat3 uMatrix[6];
uniform vec3 uOffset[6];
uniform int uSteps;
uniform vec4 uStops[8];
uniform int uStopCount;
uniform vec2 uCenter;
uniform vec2 uRadii;
uniform float uHeight;
out vec4 outColor;
void main() {
  vec3 c=texelFetch(uScene,ivec2(gl_FragCoord.xy),0).rgb;
  for(int i=0;i<6;i++){if(i>=uSteps)break;c=clamp(uMatrix[i]*c+uOffset[i],0.,1.);}
  float t=length((vec2(gl_FragCoord.x,uHeight-gl_FragCoord.y)-uCenter)/uRadii);
  vec3 paper=uStops[0].rgb;
  for(int i=1;i<8;i++){
    if(i>=uStopCount)break;
    vec4 a=uStops[i-1],b=uStops[i];
    if(t>=b.w)paper=b.rgb;else if(t>a.w)paper=mix(a.rgb,b.rgb,(t-a.w)/(b.w-a.w));
  }
  outColor=vec4(c*(floor(paper*255.+.5)/255.),1.);
}`;

// A round ring of the same width as a many-sided one carries a little more ink than it: its band is whole where the
// polygon's sides leave their joints. It is drawn this much finer, so a ring weighs what the polygon weighed.
const RING_WEIGHT=.92;
const colorCache = new Map();
// Quads in a row mostly share their colour: the last one is at hand without a lookup.
let lastColor='',lastRgba=null;
function color(value) {
  if (value===lastColor) return lastRgba;
  const known=colorCache.get(value);
  if (known) {lastColor=value;lastRgba=known;return known;}
  const h=value.replace('#','');
  if (![3,4,6,8].includes(h.length) || !/^[a-f0-9]+$/i.test(h)) throw new Error('Unsupported scene color: '+value);
  const full=h.length<5?[...h].map(c=>c+c).join(''):h;
  const rgba=[0,2,4].map(i=>parseInt(full.slice(i,i+2),16)/255);
  rgba.push(full.length===8?parseInt(full.slice(6,8),16)/255:1);
  colorCache.set(value,rgba);
  return rgba;
}
class SceneGradient {
  constructor(a,b) { this.a=a;this.b=b;this.stops=[]; }
  addColorStop(at,value) {this.stops.push([at,color(value)]);this.stops.sort((a,b)=>a[0]-b[0]);}
  at(x,y) {
    const dx=this.b[0]-this.a[0],dy=this.b[1]-this.a[1];
    const t=Math.max(0,Math.min(1,((x-this.a[0])*dx+(y-this.a[1])*dy)/(dx*dx+dy*dy||1)));
    const a=this.stops[0],b=this.stops.at(-1);
    return a[1].map((v,i)=>v+(b[1][i]-v)*t);
  }
}

export class SophiaGpuCanvas {
  constructor(canvas) {
    this.canvas=canvas;
    const gl=canvas.getContext('webgl2',{alpha:false,antialias:false,depth:false,stencil:false,premultipliedAlpha:true,preserveDrawingBuffer:false,powerPreference:'default',failIfMajorPerformanceCaveat:false});
    if(!gl)throw new Error('WebGL2 unavailable');
    this.gl=gl;
    this.measure=document.createElement('canvas').getContext('2d');
    // An image enters the atlas through a small canvas of its own size and a one-pixel border, uploaded to its place on
    // the texture: only what changed travels to the GPU, and no copy of the whole atlas is kept beside it.
    this.staging=document.createElement('canvas');this.stagingContext=this.staging.getContext('2d');
    this.images=new Map();this.shelfX=2;this.shelfY=2;this.shelfHeight=0;
    this.capacity=0;this.allocate(16384);
    this.starCapacity=0;this.starsUsed=0;this.allocateStars(512);this.starSteps=new Float32Array(12);
    this.matrix=[1,0,0,1,0,0];this.stack=[];this.path=[];
    // Immediate painter: one reusable quad is sufficient between submissions.
    // Avoid allocating point/UV arrays for every background star each frame.
    this.quadPoints=Array.from({length:4},()=>[0,0]);
    this.quadLocals=Array.from({length:4},()=>[0,0]);
    this.quadShape=[0,0,0];
    this.globalAlpha=1;this.globalCompositeOperation='source-over';
    this.fillStyle='#000000';this.strokeStyle='#000000';this.lineWidth=1;
    this.lost=false;this.disposed=false;
    this.setup();
    canvas.addEventListener('webglcontextlost',e=>{e.preventDefault();this.lost=true;canvas.dataset.context='lost';});
    canvas.addEventListener('webglcontextrestored',()=>{
      if(this.disposed)return;
      this.lost=false;this.setup();this.clearAtlas();canvas.dataset.context='restored';
      canvas.dispatchEvent(new CustomEvent('sophia-renderer-restored'));
    });
    this.pagehide=e=>{if(!e.persisted)this.dispose();};
    addEventListener('pagehide',this.pagehide);
  }
  // Program, buffers, atlas and fixed state: made once, and again when a lost context is restored.
  setup() {
    const gl=this.gl;
    const shader=(type,source)=>{
      const value=gl.createShader(type);gl.shaderSource(value,'#version 300 es\n'+source);gl.compileShader(value);
      if(!gl.getShaderParameter(value,gl.COMPILE_STATUS)&&!gl.isContextLost())throw new Error('Sophia shader: '+gl.getShaderInfoLog(value));
      return value;
    };
    const program=gl.createProgram(),vertex=shader(gl.VERTEX_SHADER,VERTEX),fragment=shader(gl.FRAGMENT_SHADER,FRAGMENT);
    gl.attachShader(program,vertex);gl.attachShader(program,fragment);
    ['aPosition','aLocal','aColor','aShape'].forEach((name,index)=>gl.bindAttribLocation(program,index,name));
    gl.linkProgram(program);
    if(!gl.getProgramParameter(program,gl.LINK_STATUS)&&!gl.isContextLost())throw new Error('Sophia program: '+gl.getProgramInfoLog(program));
    gl.deleteShader(vertex);gl.deleteShader(fragment);
    gl.useProgram(program);this.program=program;
    this.uResolution=gl.getUniformLocation(program,'uResolution');
    gl.uniform1i(gl.getUniformLocation(program,'uAtlas'),0);
    this.vao=gl.createVertexArray();gl.bindVertexArray(this.vao);
    this.vertexBuffer=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,this.vertexBuffer);
    for(const [index,size,offset] of [[0,2,0],[1,2,2],[2,4,4],[3,3,8]]){gl.enableVertexAttribArray(index);gl.vertexAttribPointer(index,size,gl.FLOAT,false,STRIDE*4,offset*4);}
    this.indexBuffer=gl.createBuffer();gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,this.indexBuffer);
    this.bufferCapacity=0;this.indexQuads=0;
    this.texture=gl.createTexture();gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,this.texture);
    gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);
    gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,ATLAS,ATLAS,0,gl.RGBA,gl.UNSIGNED_BYTE,null);
    // Atlas texels stay encoded sRGB and premultiplied, as the Canvas painter composes them.
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL,false);gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL,true);
    gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL,gl.NONE);gl.pixelStorei(gl.UNPACK_ALIGNMENT,4);
    gl.disable(gl.DEPTH_TEST);gl.enable(gl.CULL_FACE);gl.cullFace(gl.BACK);gl.frontFace(gl.CCW);
    gl.enable(gl.BLEND);gl.blendEquation(gl.FUNC_ADD);gl.clearColor(0,0,0,1);
    this.width=this.height=0;this.paperShader=null;this.paperScene=null;
    // The stars' program reads its records one star at a time from a buffer of its own, beside the quads'.
    const starProgram=gl.createProgram(),starVertex=shader(gl.VERTEX_SHADER,STAR_VERTEX),starFragment=shader(gl.FRAGMENT_SHADER,FRAGMENT);
    gl.attachShader(starProgram,starVertex);gl.attachShader(starProgram,starFragment);
    ['aCenter','aGlow','aGlowUv','aColor','aRays','aMore'].forEach((name,index)=>gl.bindAttribLocation(starProgram,index,name));
    gl.linkProgram(starProgram);
    if(!gl.getProgramParameter(starProgram,gl.LINK_STATUS)&&!gl.isContextLost())throw new Error('Sophia star program: '+gl.getProgramInfoLog(starProgram));
    gl.deleteShader(starVertex);gl.deleteShader(starFragment);
    this.starProgram=starProgram;gl.useProgram(starProgram);
    this.uStarResolution=gl.getUniformLocation(starProgram,'uResolution');this.uStarSteps=gl.getUniformLocation(starProgram,'uSteps');
    gl.uniform1i(gl.getUniformLocation(starProgram,'uAtlas'),0);gl.uniform4fv(this.uStarSteps,this.starSteps);
    this.starVao=gl.createVertexArray();gl.bindVertexArray(this.starVao);
    this.starBuffer=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,this.starBuffer);this.starBufferCapacity=0;
    for(let index=0;index<6;index++){gl.enableVertexAttribArray(index);gl.vertexAttribPointer(index,4,gl.FLOAT,false,STAR_STRIDE*4,index*16);gl.vertexAttribDivisor(index,1);}
    gl.useProgram(program);gl.bindVertexArray(this.vao);gl.bindBuffer(gl.ARRAY_BUFFER,this.vertexBuffer);
  }
  get font() {return this.measure.font;}
  set font(value) {this.measure.font=value;}
  measureText(value) {return this.measure.measureText(value);}
  point(x,y) {const m=this.matrix;return[m[0]*x+m[2]*y+m[4],m[1]*x+m[3]*y+m[5]];}
  setTransform(...m) {this.matrix=m;}
  translate(x,y) {const p=this.point(x,y);this.matrix[4]=p[0];this.matrix[5]=p[1];}
  rotate(a) {
    const [x,y,z,w,tx,ty]=this.matrix,c=Math.cos(a),s=Math.sin(a);
    this.matrix=[x*c+z*s,y*c+w*s,z*c-x*s,w*c-y*s,tx,ty];
  }
  save() {this.stack.push({matrix:this.matrix.slice(),globalAlpha:this.globalAlpha,globalCompositeOperation:this.globalCompositeOperation,fillStyle:this.fillStyle,strokeStyle:this.strokeStyle,lineWidth:this.lineWidth});}
  restore() {const s=this.stack.pop();if(s)Object.assign(this,s);}
  createLinearGradient(x1,y1,x2,y2) {return new SceneGradient(this.point(x1,y1),this.point(x2,y2));}
  // Capacity counts corners, four to a quad.
  allocate(capacity) {
    const old=this.data;
    this.capacity=capacity;this.data=new Float32Array(capacity*STRIDE);
    if(old)this.data.set(old);
  }
  allocateStars(capacity) {
    const old=this.starData;
    this.starCapacity=capacity;this.starData=new Float32Array(capacity*STAR_STRIDE);
    if(old)this.starData.set(old);
  }
  beginFrame() {
    this.used=0;this.starsUsed=0;this.groups=[];
    if(this.atlasFull)this.clearAtlas();
    if(this.lost||this.disposed)return;
    if(this.width!==this.canvas.width||this.height!==this.canvas.height) {
      const gl=this.gl;this.width=this.canvas.width;this.height=this.canvas.height;
      gl.viewport(0,0,this.width,this.height);gl.uniform2f(this.uResolution,this.width,this.height);
      gl.useProgram(this.starProgram);gl.uniform2f(this.uStarResolution,this.width,this.height);gl.useProgram(this.program);
    }
  }
  endFrame() {
    if(this.lost||this.disposed)return;
    const gl=this.gl,quads=this.used/4;
    if(this.starsUsed){
      gl.bindBuffer(gl.ARRAY_BUFFER,this.starBuffer);
      if(this.starBufferCapacity<this.starCapacity){gl.bufferData(gl.ARRAY_BUFFER,this.starData.byteLength,gl.DYNAMIC_DRAW);this.starBufferCapacity=this.starCapacity;}
      gl.bufferSubData(gl.ARRAY_BUFFER,0,this.starData,0,this.starsUsed*STAR_STRIDE);
    }
    gl.bindBuffer(gl.ARRAY_BUFFER,this.vertexBuffer);
    if(this.bufferCapacity<this.capacity){gl.bufferData(gl.ARRAY_BUFFER,this.data.byteLength,gl.DYNAMIC_DRAW);this.bufferCapacity=this.capacity;}
    if(this.used)gl.bufferSubData(gl.ARRAY_BUFFER,0,this.data,0,this.used*STRIDE);
    if(this.indexQuads<quads) {
      const count=this.capacity/4,indices=new Uint32Array(count*6);
      for(let q=0;q<count;q++)for(let i=0;i<6;i++)indices[q*6+i]=q*4+TRIANGLES[i];
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,indices,gl.STATIC_DRAW);this.indexQuads=count;
    }
    const paper=this.paper&&this.paperTarget();
    if(paper){gl.bindFramebuffer(gl.FRAMEBUFFER,paper.framebuffer);gl.useProgram(this.program);gl.enable(gl.BLEND);}
    gl.clear(gl.COLOR_BUFFER_BIT);
    let mode=-1,stars=false;
    for(const g of this.groups) {
      if(g.material!==mode){mode=g.material;gl.blendFuncSeparate(gl.ONE,mode===1?gl.ONE_MINUS_SRC_COLOR:gl.ONE_MINUS_SRC_ALPHA,gl.ONE,gl.ONE_MINUS_SRC_ALPHA);}
      if(!!g.stars!==stars){stars=!!g.stars;gl.useProgram(stars?this.starProgram:this.program);gl.bindVertexArray(stars?this.starVao:this.vao);}
      if(stars){
        // A run of stars reads its records from where it starts in the stars' buffer.
        gl.bindBuffer(gl.ARRAY_BUFFER,this.starBuffer);
        for(let index=0;index<6;index++)gl.vertexAttribPointer(index,4,gl.FLOAT,false,STAR_STRIDE*4,(g.start*STAR_STRIDE+index*4)*4);
        gl.drawArraysInstanced(gl.TRIANGLES,0,STAR_QUADS*6,g.count);
      } else gl.drawElements(gl.TRIANGLES,g.count/4*6,gl.UNSIGNED_INT,g.start/4*6*4);
    }
    if(stars){gl.useProgram(this.program);gl.bindVertexArray(this.vao);}
    gl.bindBuffer(gl.ARRAY_BUFFER,this.vertexBuffer);
    if(paper)this.layOnPaper(paper);
    // The counts are told every thirtieth frame: an attribute written each frame is a page mutation each frame.
    if((this.frames=(this.frames||0)+1)%30===1){
      this.canvas.dataset.drawCalls=String(this.groups.length);
      this.canvas.dataset.gpuVertices=String(this.used);
      this.canvas.dataset.gpuTextures='1';
    }
  }
  quad(points,locals,shape,style,alpha=this.globalAlpha) {
    if(this.used+4>this.capacity)this.allocate(this.capacity*2);
    const material=this.globalCompositeOperation==='screen'?1:0;
    let group=this.groups.at(-1);
    if(!group||group.stars||group.material!==material){group={start:this.used,count:0,material};this.groups.push(group);}
    group.count+=4;
    const solid=style instanceof SceneGradient?null:color(style);
    for(let corner=0;corner<4;corner++) {
      const p=points[corner],uv=locals[corner];
      const c=solid||style.at(...p);
      const a=c[3]*alpha;
      const offset=this.used++*STRIDE;
      this.data[offset]=p[0];this.data[offset+1]=p[1];
      this.data[offset+2]=uv[0];this.data[offset+3]=uv[1];
      this.data[offset+4]=c[0]*a;this.data[offset+5]=c[1]*a;this.data[offset+6]=c[2]*a;this.data[offset+7]=a;
      this.data[offset+8]=shape[0];this.data[offset+9]=shape[1];this.data[offset+10]=shape[2];
    }
  }
  // A run of a frame's quads can be kept and laid again, unchanged, into a later frame: the part of a scene that rests
  // while the sky around it moves. What is kept refers to the atlas as it stood, so a reset atlas or a new size of the
  // canvas lets it go and the scene is drawn whole again.
  mark() {return {used:this.used,stars:this.starsUsed};}
  keep(from,to=this.mark()) {
    const count=to.used-from.used,starCount=to.stars-from.stars;
    if(count<0||starCount<0||to.used>this.used||to.stars>this.starsUsed)return null;
    if(!this.kept||this.kept.length<count*STRIDE)this.kept=new Float32Array(Math.max(count,1024)*STRIDE*1.25|0);
    if(!this.keptStars||this.keptStars.length<starCount*STAR_STRIDE)this.keptStars=new Float32Array(Math.max(starCount,256)*STAR_STRIDE*1.25|0);
    this.kept.set(this.data.subarray(from.used*STRIDE,to.used*STRIDE));
    this.keptStars.set(this.starData.subarray(from.stars*STAR_STRIDE,to.stars*STAR_STRIDE));
    const groups=[];
    for(const g of this.groups){
      const [low,high]=g.stars?[from.stars,to.stars]:[from.used,to.used],start=Math.max(g.start,low),end=Math.min(g.start+g.count,high);
      if(end>start)groups.push({start:start-low,count:end-start,material:g.material,stars:!!g.stars});
    }
    return {count,starCount,groups,epoch:this.atlasEpoch|0,width:this.canvas.width,height:this.canvas.height,data:this.kept,stars:this.keptStars};
  }
  replay(kept) {
    if(!kept||kept.data!==this.kept||kept.stars!==this.keptStars||kept.epoch!==(this.atlasEpoch|0)||kept.width!==this.canvas.width||kept.height!==this.canvas.height)return false;
    while(this.used+kept.count>this.capacity)this.allocate(this.capacity*2);
    while(this.starsUsed+kept.starCount>this.starCapacity)this.allocateStars(this.starCapacity*2);
    this.data.set(this.kept.subarray(0,kept.count*STRIDE),this.used*STRIDE);
    this.starData.set(this.keptStars.subarray(0,kept.starCount*STAR_STRIDE),this.starsUsed*STAR_STRIDE);
    for(const g of kept.groups){
      const start=(g.stars?this.starsUsed:this.used)+g.start,last=this.groups.at(-1);
      if(last&&!!last.stars===g.stars&&last.material===g.material&&last.start+last.count===start)last.count+=g.count;
      else this.groups.push(g.stars?{start,count:g.count,material:g.material,stars:true}:{start,count:g.count,material:g.material});
    }
    this.used+=kept.count;this.starsUsed+=kept.starCount;return true;
  }
  // The steps of a star's rays, outward from its centre: [start, end, light, width] each, in units of the ray's reach.
  setStarRays(steps) {
    steps.slice(0,3).forEach((step,i)=>this.starSteps.set(step,i*4));
    if(this.lost||this.disposed)return;
    const gl=this.gl;gl.useProgram(this.starProgram);gl.uniform4fv(this.uStarSteps,this.starSteps);gl.useProgram(this.program);
  }
  // One star: {x, y, color, alpha, radius, core, image, glowX, glowY, glowSize, glowAlpha, ray, from, rayLight, lineScale},
  // where the glow's place and size are taken from the star's centre and a ray's light is 0 for a star without rays.
  // It lays what drawImage, two discs' fills and the rays' strokes lay for it, in that order, as one record.
  star(s) {
    const entry=s.image?this.atlasEntry(s.image):null;
    if(this.starsUsed+1>this.starCapacity)this.allocateStars(this.starCapacity*2);
    const material=this.globalCompositeOperation==='screen'?1:0,groups=this.groups;
    let group=groups[groups.length-1];
    if(!group||!group.stars||group.material!==material){group={start:this.starsUsed,count:0,material,stars:true};groups.push(group);}
    group.count++;
    const m=this.matrix,d=this.starData,o=this.starsUsed++*STAR_STRIDE,c=color(s.color);
    d[o]=m[0]*s.x+m[2]*s.y+m[4];d[o+1]=m[1]*s.x+m[3]*s.y+m[5];d[o+2]=Math.hypot(m[0],m[1]);d[o+3]=s.radius;
    d[o+4]=s.glowX;d[o+5]=s.glowY;d[o+6]=entry?s.glowSize:0;d[o+7]=s.glowAlpha;
    if(entry){d[o+8]=entry.x/ATLAS;d[o+9]=entry.y/ATLAS;d[o+10]=entry.width/ATLAS;d[o+11]=entry.height/ATLAS;}
    d[o+12]=c[0];d[o+13]=c[1];d[o+14]=c[2];d[o+15]=c[3];
    d[o+16]=s.core;d[o+17]=s.ray;d[o+18]=s.from;d[o+19]=s.rayLight;
    d[o+20]=s.alpha;d[o+21]=s.lineScale;
  }
  // The atlas packs images on shelves as they are first drawn. When it is full, the image waits and the next frame starts
  // the atlas over with only the images it draws, so a scene that changes its images (a theme, a lens) never runs out.
  atlasEntry(image) {
    if(this.images.has(image))return this.images.get(image);
    const width=image.width,height=image.height;
    if(width+4>ATLAS||height+4>ATLAS)return null;
    if(this.shelfX+width+2>ATLAS){this.shelfX=2;this.shelfY+=this.shelfHeight+4;this.shelfHeight=0;}
    if(this.shelfY+height+2>ATLAS){this.atlasFull=true;return null;}
    const entry={x:this.shelfX,y:this.shelfY,width,height};
    this.shelfX+=width+4;this.shelfHeight=Math.max(this.shelfHeight,height);
    this.images.set(image,entry);this.invalidateImage(image);return entry;
  }
  clearAtlas() {
    this.images.clear();
    this.shelfX=2;this.shelfY=2;this.shelfHeight=0;this.atlasFull=false;this.atlasEpoch=(this.atlasEpoch|0)+1;
    if(!this.lost&&!this.disposed){const gl=this.gl;gl.bindTexture(gl.TEXTURE_2D,this.texture);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,ATLAS,ATLAS,0,gl.RGBA,gl.UNSIGNED_BYTE,null);}
    this.canvas.dataset.atlasResets=String((Number(this.canvas.dataset.atlasResets)||0)+1);
  }
  invalidateImage(image) {
    const e=this.images.get(image);if(!e)return;
    const s=this.staging,g=this.stagingContext,{x,y,width:w,height:h}=e;
    if(s.width!==w+2||s.height!==h+2){s.width=w+2;s.height=h+2;}else g.clearRect(0,0,w+2,h+2);
    g.drawImage(image,1,1);
    // Border extrusion retains clamp-to-edge behavior inside the shared atlas.
    g.drawImage(image,0,0,w,1,1,0,w,1);g.drawImage(image,0,h-1,w,1,1,h+1,w,1);
    g.drawImage(image,0,0,1,h,0,1,1,h);g.drawImage(image,w-1,0,1,h,w+1,1,1,h);
    for(const [sx,sy,dx,dy] of [[0,0,0,0],[w-1,0,w+1,0],[0,h-1,0,h+1],[w-1,h-1,w+1,h+1]])g.drawImage(image,sx,sy,1,1,dx,dy,1,1);
    if(this.lost||this.disposed)return;
    const gl=this.gl;gl.bindTexture(gl.TEXTURE_2D,this.texture);
    gl.texSubImage2D(gl.TEXTURE_2D,0,x-1,y-1,gl.RGBA,gl.UNSIGNED_BYTE,s);
  }
  drawImage(image,x,y,w,h) {
    const e=this.atlasEntry(image);if(!e)return;
    // The atlas's rows lie as the images' do, top first.
    const u=e.x/ATLAS,v=e.y/ATLAS,uw=e.width/ATLAS,vh=e.height/ATLAS;
    const m=this.matrix,points=this.quadPoints,locals=this.quadLocals,shape=this.quadShape;
    for(let i=0;i<4;i++) {
      const right=i===1||i===2,bottom=i>=2,px=right?x+w:x,py=bottom?y+h:y;
      points[i][0]=m[0]*px+m[2]*py+m[4];points[i][1]=m[1]*px+m[3]*py+m[5];
      locals[i][0]=right?u+uw:u;locals[i][1]=bottom?v+vh:v;
    }
    shape[0]=shape[1]=shape[2]=0;
    this.quad(points,locals,shape,'#ffffff');
  }
  rectAt(cx,cy,ux,uy,halfWidth,halfHeight,style) {
    const extentX=halfWidth+1,extentY=halfHeight+1,points=this.quadPoints,locals=this.quadLocals,shape=this.quadShape;
    for(let i=0;i<4;i++) {
      const x=i===1||i===2?extentX:-extentX,y=i>=2?extentY:-extentY;
      locals[i][0]=x;locals[i][1]=y;
      points[i][0]=cx+ux*x-uy*y;points[i][1]=cy+uy*x+ux*y;
    }
    shape[0]=1;shape[1]=halfWidth;shape[2]=halfHeight;
    this.quad(points,locals,shape,style);
  }
  fillRect(x,y,w,h) {
    const m=this.matrix,sx=Math.hypot(m[0],m[1]),sy=Math.hypot(m[2],m[3]);
    const px=x+w/2,py=y+h/2;
    this.rectAt(m[0]*px+m[2]*py+m[4],m[1]*px+m[3]*py+m[5],m[0]/sx,m[1]/sx,Math.abs(w*sx/2),Math.abs(h*sy/2),this.fillStyle);
  }
  // A whole ring in one quad, stroked with the current line width and style: what a polygon of many sides draws, round.
  ring(x,y,r) {
    const m=this.matrix,scale=Math.hypot(m[0],m[1]),cx=m[0]*x+m[2]*y+m[4],cy=m[1]*x+m[3]*y+m[5];
    const radius=r*scale,half=this.lineWidth*scale/2*RING_WEIGHT,extent=radius+half+1.5,points=this.quadPoints,locals=this.quadLocals,shape=this.quadShape;
    for(let i=0;i<4;i++){const lx=i===1||i===2?extent:-extent,ly=i>=2?extent:-extent;locals[i][0]=lx;locals[i][1]=ly;points[i][0]=cx+lx;points[i][1]=cy+ly;}
    shape[0]=5;shape[1]=radius;shape[2]=half;
    this.quad(points,locals,shape,this.strokeStyle);
  }
  // A dashed line from a to b with dashes `on` long every `period` from a, drawn between `from` and `to` along it, in the
  // current transform, line width and stroke style: what stroke() draws for those dashes, the whole ones in one quad
  // whose shader repeats them and a dash cut by either end in a quad of its own.
  dashes(ax,ay,bx,by,from,to,on,period) {
    const A=this.point(ax,ay),B=this.point(bx,by),reach=Math.hypot(bx-ax,by-ay),dx=B[0]-A[0],dy=B[1]-A[1],length=Math.hypot(dx,dy);
    if(!(reach>0)||!(length>0)||!(on>0)||!(period>on))return;
    const ux=dx/length,uy=dy/length,scale=length/reach,halfWidth=this.lineWidth*Math.hypot(this.matrix[0],this.matrix[1])/2;
    const piece=(begin,end)=>{const at=(begin+end)/2*scale;this.rectAt(A[0]+ux*at,A[1]+uy*at,ux,uy,(end-begin)*scale/2,halfWidth,this.strokeStyle);};
    let first=-1,last=-1;
    for(let start=0;start<to;start+=period){
      const begin=Math.max(start,from),end=Math.min(start+on,to);if(end<=begin)continue;
      if(begin===start&&end===start+on){if(first<0)first=start;last=start;}else piece(begin,end);
    }
    if(first<0)return;
    if(first===last){piece(first,first+on);return;}
    const origin=first*scale,run=(last-first+on)*scale,points=this.quadPoints,locals=this.quadLocals,shape=this.quadShape;
    for(let i=0;i<4;i++){
      const x=i===1||i===2?run+1:-1,y=i>=2?halfWidth+1:-halfWidth-1;
      locals[i][0]=x;locals[i][1]=y;points[i][0]=A[0]+ux*(origin+x)-uy*y;points[i][1]=A[1]+uy*(origin+x)+ux*y;
    }
    shape[0]=6+on/period;shape[1]=halfWidth;shape[2]=period*scale;
    this.quad(points,locals,shape,this.strokeStyle);
  }
  // A row of `count` round dots of radius r from a to b, their centres evenly spaced from half a space in, drawn where a
  // centre lies between `from` and `to`, in the current transform and fill style: what fill() draws for those discs, a
  // row of them in one quad.
  dots(ax,ay,bx,by,r,count,from,to) {
    const reach=Math.hypot(bx-ax,by-ay),step=reach/count;if(!(reach>0)||!(count>=1))return;
    let first=-1,last=-1;
    for(let i=0;i<count;i++){const at=(i+.5)*step;if(at<from||at>to)continue;if(first<0)first=i;last=i;}
    if(first<0)return;
    if(first===last){this.beginPath();const at=(first+.5)*step;this.arc(ax+(bx-ax)*at/reach,ay+(by-ay)*at/reach,r,0,Math.PI*2);this.fill();this.beginPath();return;}
    const A=this.point(ax,ay),B=this.point(bx,by),dx=B[0]-A[0],dy=B[1]-A[1],length=Math.hypot(dx,dy);if(!(length>0))return;
    const ux=dx/length,uy=dy/length,scale=length/reach,radius=r*Math.hypot(this.matrix[0],this.matrix[1]),spacing=step*scale;
    const origin=(first+.5)*step*scale,run=(last-first)*spacing,points=this.quadPoints,locals=this.quadLocals,shape=this.quadShape;
    for(let i=0;i<4;i++){
      const x=i===1||i===2?run+radius+1:-radius-1,y=i>=2?radius+1:-radius-1;
      locals[i][0]=x;locals[i][1]=y;points[i][0]=A[0]+ux*(origin+x)-uy*y;points[i][1]=A[1]+uy*(origin+x)+ux*y;
    }
    shape[0]=8;shape[1]=radius;shape[2]=spacing;
    this.quad(points,locals,shape,this.fillStyle);
  }
  beginPath() {this.path=[];this.current=null;}
  moveTo(x,y) {this.current=this.point(x,y);}
  lineTo(x,y) {const p=this.point(x,y);if(this.current)this.path.push({type:'line',a:this.current,b:p});this.current=p;}
  arc(x,y,r,start,end) {
    if(Math.abs(end-start-Math.PI*2)>.0001)throw new Error('Only full scene discs are supported');
    this.path.push({type:'disc',center:this.point(x,y),radius:r*Math.hypot(this.matrix[0],this.matrix[1])});
  }
  fill() {
    for(const p of this.path) {
      if(p.type!=='disc')throw new Error('Only scene discs may be filled');
      const r=p.radius+1,local=[[-r,-r],[r,-r],[r,r],[-r,r]];
      this.quad(local.map(([x,y])=>[x+p.center[0],y+p.center[1]]),local,[2,p.radius,p.radius],this.fillStyle);
    }
  }
  stroke() {
    const halfWidth=this.lineWidth*Math.hypot(this.matrix[0],this.matrix[1])/2;
    if(this.path.length===2) {
      const [a,b]=this.path;
      if(a.type==='line'&&b.type==='line') {
        const x=a.b[0]-a.a[0],y=a.b[1]-a.a[1],ex=b.b[0]-b.a[0],ey=b.b[1]-b.a[1],len=Math.hypot(x,y);
        if(len>0&&Math.abs(len-Math.hypot(ex,ey))<.00001&&Math.abs(x*ex+y*ey)<.00001
          &&Math.abs(a.a[0]+a.b[0]-b.a[0]-b.b[0])<.00001&&Math.abs(a.a[1]+a.b[1]-b.a[1]-b.b[1])<.00001) {
          const cx=(a.a[0]+a.b[0])/2,cy=(a.a[1]+a.b[1])/2,ux=x/len,uy=y/len,extent=len/2+1;
          const local=[[-extent,-extent],[extent,-extent],[extent,extent],[-extent,extent]];
          this.quad(local.map(([x,y])=>[cx+ux*x-uy*y,cy+uy*x+ux*y]),local,[4,len/2,halfWidth],this.strokeStyle);
          return;
        }
      }
    }
    for(let i=0;i<this.path.length;i++) {
      const p=this.path[i],next=this.path[i+1];
      if(p.type!=='line')throw new Error('Only scene line segments may be stroked');
      const dx=p.b[0]-p.a[0],dy=p.b[1]-p.a[1],length=Math.hypot(dx,dy);
      if(next?.type==='line'&&p.b[0]===next.a[0]&&p.b[1]===next.a[1]) {
        const ex=next.b[0]-next.a[0],ey=next.b[1]-next.a[1],otherLength=Math.hypot(ex,ey);
        if(length>0&&Math.abs(length-otherLength)<.00001&&Math.abs(dx*ex+dy*ey)<.00001) {
          const u=[-dx/length,-dy/length],v=[ex/length,ey/length],lo=-halfWidth-1,hi=length+1;
          let local=[[lo,lo],[hi,lo],[hi,hi],[lo,hi]];
          if(u[0]*v[1]-u[1]*v[0]<0)local=[local[0],local[3],local[2],local[1]];
          this.quad(local.map(([x,y])=>[p.b[0]+u[0]*x+v[0]*y,p.b[1]+u[1]*x+v[1]*y]),local,[3,length,halfWidth],this.strokeStyle);
          i++;continue;
        }
      }
      if(length>0)this.rectAt((p.a[0]+p.b[0])/2,(p.a[1]+p.b[1])/2,dx/length,dy/length,length/2,halfWidth,this.strokeStyle);
    }
  }
  // The day's paper, or null for the night: {steps:[{matrix:[9 numbers, row by row],offset:[3]}], stops:[[r,g,b,at]],
  // center:[x,y], radii:[x,y]} in canvas pixels, as the sky reads them from the page (sky.mjs). It answers whether the
  // painter lays the scene on the paper itself.
  setPaper(paper) {
    if(!paper){this.paper=null;return true;}
    if(!Array.isArray(paper.steps)||paper.steps.length>6||!Array.isArray(paper.stops)||!paper.stops.length||paper.stops.length>8)return false;
    if(!this.lost&&!this.paperProgram()){this.paper=null;return false;}
    this.paper=paper;return true;
  }
  paperProgram() {
    if(this.paperShader)return this.paperShader;
    const gl=this.gl;
    try{
      const shader=(type,source)=>{const value=gl.createShader(type);gl.shaderSource(value,'#version 300 es\n'+source);gl.compileShader(value);if(!gl.getShaderParameter(value,gl.COMPILE_STATUS)&&!gl.isContextLost())throw new Error(gl.getShaderInfoLog(value));return value;};
      const program=gl.createProgram(),vertex=shader(gl.VERTEX_SHADER,PAPER_VERTEX),fragment=shader(gl.FRAGMENT_SHADER,PAPER_FRAGMENT);
      gl.attachShader(program,vertex);gl.attachShader(program,fragment);gl.linkProgram(program);
      if(!gl.getProgramParameter(program,gl.LINK_STATUS)&&!gl.isContextLost())throw new Error(gl.getProgramInfoLog(program));
      gl.deleteShader(vertex);gl.deleteShader(fragment);
      const at=name=>gl.getUniformLocation(program,name);
      this.paperShader={program,vao:gl.createVertexArray(),scene:at('uScene'),matrix:at('uMatrix'),offset:at('uOffset'),steps:at('uSteps'),stops:at('uStops'),stopCount:at('uStopCount'),center:at('uCenter'),radii:at('uRadii'),height:at('uHeight')};
      gl.useProgram(this.program);
    }catch{this.paperShader=null;}
    return this.paperShader;
  }
  // The texture the scene is drawn to before it is laid on the paper, as large as the canvas.
  paperTarget() {
    const gl=this.gl,width=this.canvas.width,height=this.canvas.height;
    if(!this.paperProgram())return null;
    let target=this.paperScene;
    if(!target||target.width!==width||target.height!==height){
      if(target){gl.deleteFramebuffer(target.framebuffer);gl.deleteTexture(target.texture);}
      const texture=gl.createTexture();gl.activeTexture(gl.TEXTURE1);gl.bindTexture(gl.TEXTURE_2D,texture);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
      gl.texStorage2D(gl.TEXTURE_2D,1,gl.RGBA8,width,height);gl.activeTexture(gl.TEXTURE0);
      const framebuffer=gl.createFramebuffer();gl.bindFramebuffer(gl.FRAMEBUFFER,framebuffer);
      gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0,gl.TEXTURE_2D,texture,0);gl.bindFramebuffer(gl.FRAMEBUFFER,null);
      target=this.paperScene={texture,framebuffer,width,height};
    }
    return target;
  }
  layOnPaper(target) {
    const gl=this.gl,shader=this.paperShader,paper=this.paper;
    gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.disable(gl.BLEND);
    gl.useProgram(shader.program);gl.bindVertexArray(shader.vao);
    gl.activeTexture(gl.TEXTURE1);gl.bindTexture(gl.TEXTURE_2D,target.texture);gl.activeTexture(gl.TEXTURE0);
    // The program keeps its uniforms, so the paper's travel when the paper or the canvas's height changes.
    if(shader.paper!==paper||shader.height!==target.height){
      const matrices=new Float32Array(54),offsets=new Float32Array(18),stops=new Float32Array(32);
      paper.steps.forEach((step,i)=>{const m=step.matrix;matrices.set([m[0],m[3],m[6],m[1],m[4],m[7],m[2],m[5],m[8]],i*9);offsets.set(step.offset,i*3);});
      paper.stops.forEach((stop,i)=>stops.set(stop,i*4));
      gl.uniform1i(shader.scene,1);gl.uniformMatrix3fv(shader.matrix,false,matrices);gl.uniform3fv(shader.offset,offsets);gl.uniform1i(shader.steps,paper.steps.length);
      gl.uniform4fv(shader.stops,stops);gl.uniform1i(shader.stopCount,paper.stops.length);
      gl.uniform2f(shader.center,paper.center[0],paper.center[1]);gl.uniform2f(shader.radii,paper.radii[0],paper.radii[1]);gl.uniform1f(shader.height,target.height);
      shader.paper=paper;shader.height=target.height;
    }
    gl.drawArrays(gl.TRIANGLES,0,3);
    gl.useProgram(this.program);gl.bindVertexArray(this.vao);gl.enable(gl.BLEND);
  }
  dispose() {
    if(this.disposed)return;this.disposed=true;
    removeEventListener('pagehide',this.pagehide);
    const gl=this.gl;this.images.clear();
    if(!gl.isContextLost()){gl.deleteBuffer(this.vertexBuffer);gl.deleteBuffer(this.indexBuffer);gl.deleteVertexArray(this.vao);gl.deleteTexture(this.texture);gl.deleteProgram(this.program);gl.deleteBuffer(this.starBuffer);gl.deleteVertexArray(this.starVao);gl.deleteProgram(this.starProgram);}
  }
}
