import * as THREE from 'three';

// A deliberately small painter port for the accepted observatory, not a general
// Canvas implementation. Camera, gestures, texture authorship and DOM stay owned
// by the reference scene. Draw order and encoded-sRGB compositing are explicit.
const STRIDE = 11;
const CORNERS = [0,2,1,0,3,2];
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
    } else {
      vec2 dx=vec2(dFdx(vLocal.x),dFdy(vLocal.x));
      vec2 dy=vec2(dFdx(vLocal.y),dFdy(vLocal.y));
      float sx=stripCoverage(vLocal.x,vShape.z,dx),sy=stripCoverage(vLocal.y,vShape.z,dy);
      coverage=stripCoverage(vLocal.x,vShape.y,dx)*sy
        +sx*stripCoverage(vLocal.y,vShape.y,dy)-sx*sy;
    }
    outColor=vColor*coverage;
  }
}`;

const colorCache = new Map();
function color(value) {
  if (colorCache.has(value)) return colorCache.get(value);
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
    this.renderer=new THREE.WebGLRenderer({canvas,alpha:false,antialias:false,depth:false,stencil:false,premultipliedAlpha:true});
    this.renderer.setClearColor(0x000000,1);
    this.renderer.outputColorSpace=THREE.LinearSRGBColorSpace;
    this.renderer.toneMapping=THREE.NoToneMapping;
    this.renderer.sortObjects=false;
    this.measure=document.createElement('canvas').getContext('2d');
    this.atlas=document.createElement('canvas');this.atlas.width=this.atlas.height=2048;
    this.atlasContext=this.atlas.getContext('2d');
    this.atlasTexture=new THREE.CanvasTexture(this.atlas);
    this.atlasTexture.colorSpace=THREE.NoColorSpace;
    this.atlasTexture.premultiplyAlpha=true;
    this.atlasTexture.generateMipmaps=false;
    this.atlasTexture.minFilter=this.atlasTexture.magFilter=THREE.LinearFilter;
    this.images=new Map();this.shelfX=2;this.shelfY=2;this.shelfHeight=0;
    this.resolution=new THREE.Vector2(1,1);
    this.materials=['source-over','screen'].map(mode=>new THREE.RawShaderMaterial({
      name:'Sophia '+mode,vertexShader:VERTEX,fragmentShader:FRAGMENT,glslVersion:THREE.GLSL3,
      uniforms:{uResolution:{value:this.resolution},uAtlas:{value:this.atlasTexture}},
      transparent:true,depthTest:false,depthWrite:false,toneMapped:false,
      blending:THREE.CustomBlending,blendEquation:THREE.AddEquation,
      blendSrc:THREE.OneFactor,blendDst:mode==='screen'?THREE.OneMinusSrcColorFactor:THREE.OneMinusSrcAlphaFactor,
      blendSrcAlpha:THREE.OneFactor,blendDstAlpha:THREE.OneMinusSrcAlphaFactor,
    }));
    this.scene=new THREE.Scene();this.camera=new THREE.Camera();
    this.capacity=0;this.mesh=null;this.allocate(16384);
    this.matrix=[1,0,0,1,0,0];this.stack=[];this.path=[];
    // Immediate painter: one reusable quad is sufficient between submissions.
    // Avoid allocating point/UV arrays for every background star each frame.
    this.quadPoints=Array.from({length:4},()=>[0,0]);
    this.quadLocals=Array.from({length:4},()=>[0,0]);
    this.quadShape=[0,0,0];
    this.globalAlpha=1;this.globalCompositeOperation='source-over';
    this.fillStyle='#000000';this.strokeStyle='#000000';this.lineWidth=1;
    this.lost=false;this.disposed=false;
    canvas.addEventListener('webglcontextlost',e=>{e.preventDefault();this.lost=true;canvas.dataset.context='lost';});
    canvas.addEventListener('webglcontextrestored',()=>{
      this.lost=false;this.atlasTexture.needsUpdate=true;canvas.dataset.context='restored';
      canvas.dispatchEvent(new CustomEvent('sophia-renderer-restored'));
    });
    addEventListener('pagehide',e=>{if(!e.persisted)this.dispose();});
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
  allocate(capacity) {
    const old=this.data;
    this.geometry?.dispose();
    this.capacity=capacity;this.data=new Float32Array(capacity*STRIDE);
    if(old)this.data.set(old);
    this.buffer=new THREE.InterleavedBuffer(this.data,STRIDE).setUsage(THREE.DynamicDrawUsage);
    this.geometry=new THREE.BufferGeometry();
    for(const [name,size,offset] of [['aPosition',2,0],['aLocal',2,2],['aColor',4,4],['aShape',3,8]]) {
      this.geometry.setAttribute(name,new THREE.InterleavedBufferAttribute(this.buffer,size,offset));
    }
    if(this.mesh)this.mesh.geometry=this.geometry;
    else {this.mesh=new THREE.Mesh(this.geometry,this.materials);this.mesh.frustumCulled=false;this.scene.add(this.mesh);}
  }
  beginFrame() {
    this.used=0;this.groups=[];
    if(this.resolution.x!==this.canvas.width||this.resolution.y!==this.canvas.height) {
      this.resolution.set(this.canvas.width,this.canvas.height);
      this.renderer.setSize(this.canvas.width,this.canvas.height,false);
    }
  }
  endFrame() {
    if(this.lost||this.disposed)return;
    this.geometry.clearGroups();
    this.groups.forEach(g=>this.geometry.addGroup(g.start,g.count,g.material));
    this.geometry.setDrawRange(0,this.used);
    this.buffer.clearUpdateRanges();this.buffer.addUpdateRange(0,this.used*STRIDE);this.buffer.needsUpdate=true;
    this.renderer.render(this.scene,this.camera);
    this.canvas.dataset.drawCalls=String(this.renderer.info.render.calls);
    this.canvas.dataset.gpuVertices=String(this.used);
    this.canvas.dataset.gpuTextures=String(this.renderer.info.memory.textures);
  }
  quad(points,locals,shape,style,alpha=this.globalAlpha) {
    if(this.used+6>this.capacity)this.allocate(this.capacity*2);
    const material=this.globalCompositeOperation==='screen'?1:0;
    let group=this.groups.at(-1);
    if(!group||group.material!==material){group={start:this.used,count:0,material};this.groups.push(group);}
    group.count+=6;
    // Screen Y is inverted by the vertex shader, so reverse the winding.
    const solid=style instanceof SceneGradient?null:color(style);
    for(const corner of CORNERS) {
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
  atlasEntry(image) {
    if(this.images.has(image))return this.images.get(image);
    const width=image.width,height=image.height;
    if(this.shelfX+width+2>2048){this.shelfX=2;this.shelfY+=this.shelfHeight+4;this.shelfHeight=0;}
    if(this.shelfY+height+2>2048)throw new Error('Scene texture atlas budget exceeded');
    const entry={x:this.shelfX,y:this.shelfY,width,height};
    this.shelfX+=width+4;this.shelfHeight=Math.max(this.shelfHeight,height);
    this.images.set(image,entry);this.invalidateImage(image);return entry;
  }
  invalidateImage(image) {
    const e=this.images.get(image);if(!e)return;
    const g=this.atlasContext,{x,y,width:w,height:h}=e;
    g.clearRect(x-1,y-1,w+2,h+2);g.drawImage(image,x,y);
    // Border extrusion retains clamp-to-edge behavior inside the shared atlas.
    g.drawImage(image,0,0,w,1,x,y-1,w,1);g.drawImage(image,0,h-1,w,1,x,y+h,w,1);
    g.drawImage(image,0,0,1,h,x-1,y,1,h);g.drawImage(image,w-1,0,1,h,x+w,y,1,h);
    for(const [sx,sy,dx,dy] of [[0,0,x-1,y-1],[w-1,0,x+w,y-1],[0,h-1,x-1,y+h],[w-1,h-1,x+w,y+h]])g.drawImage(image,sx,sy,1,1,dx,dy,1,1);
    this.atlasTexture.needsUpdate=true;
  }
  drawImage(image,x,y,w,h) {
    const e=this.atlasEntry(image),u=e.x/2048,v=1-e.y/2048,uw=e.width/2048,vh=e.height/2048;
    const m=this.matrix,points=this.quadPoints,locals=this.quadLocals,shape=this.quadShape;
    for(let i=0;i<4;i++) {
      const right=i===1||i===2,bottom=i>=2,px=right?x+w:x,py=bottom?y+h:y;
      points[i][0]=m[0]*px+m[2]*py+m[4];points[i][1]=m[1]*px+m[3]*py+m[5];
      locals[i][0]=right?u+uw:u;locals[i][1]=bottom?v-vh:v;
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
  dispose() {
    if(this.disposed)return;this.disposed=true;
    this.geometry.dispose();this.materials.forEach(m=>m.dispose());
    this.atlasTexture.dispose();this.images.clear();this.renderer.dispose();
  }
}
