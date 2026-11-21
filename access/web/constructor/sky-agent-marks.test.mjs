import {describe,expect,it} from 'vitest';
import {AGENT_COLOR,agentRolesLine,agentTextImages,drawAgentMarks} from './sky-agent-marks.mjs';

// A painter that knows only what the GPU painter (src/observatory/gpu-canvas.js) draws: straight strokes,
// full discs and images. Any other call fails the test, as it would stop the GPU sky's frame.
function gpuPainter(){
  const calls=[],state={path:[]};
  const values={globalAlpha:1,fillStyle:'#000000',strokeStyle:'#000000',lineWidth:1,lineCap:'butt'};
  const known={
    beginPath(){state.path=[];state.current=null;},
    moveTo(x,y){state.current=[x,y];},
    lineTo(x,y){if(state.current)state.path.push({type:'line',a:state.current,b:[x,y]});state.current=[x,y];},
    arc(x,y,r,start,end){if(Math.abs(end-start-Math.PI*2)>.0001)throw new Error('Only full scene discs are supported');state.path.push({type:'disc',center:[x,y],radius:r});},
    fill(){if(state.path.some(p=>p.type!=='disc'))throw new Error('Only scene discs may be filled');calls.push({op:'fill',path:state.path,style:values.fillStyle});},
    stroke(){if(state.path.some(p=>p.type!=='line'))throw new Error('Only scene line segments may be stroked');calls.push({op:'stroke',path:state.path,width:values.lineWidth,style:values.strokeStyle,alpha:values.globalAlpha});},
    drawImage(image,x,y,w,h){calls.push({op:'image',image,x,y,w,h,alpha:values.globalAlpha});},
  };
  const painter=new Proxy({},{
    get(_,key){if(key in known)return known[key];if(key in values)return values[key];throw new Error(`the GPU painter has no ${String(key)}`);},
    set(_,key,value){values[key]=value;return true;},
  });
  return {painter,calls};
}

describe('agents on the sky drawn by the GPU painter',()=>{
  it('draws one agent as a diamond: one straight stroke as wide as it is long',()=>{
    const {painter,calls}=gpuPainter();
    drawAgentMarks(painter,{x:100,y:50,radius:4,count:1,alpha:1});
    expect(calls).toHaveLength(1);
    const [{op,path,width,style,alpha}]=calls,[segment]=path;
    expect(op).toBe('stroke');expect(style).toBe(AGENT_COLOR);expect(alpha).toBeCloseTo(.9);
    expect(path).toHaveLength(1);
    const length=Math.hypot(segment.b[0]-segment.a[0],segment.b[1]-segment.a[1]);
    expect(width).toBeCloseTo(length);
    // Its corners lie 3.2 from its centre beside the star, up and to the right.
    expect([(segment.a[0]+segment.b[0])/2,(segment.a[1]+segment.b[1])/2]).toEqual([110,40]);
    expect(Math.hypot(length/2,width/2)).toBeCloseTo(3.2);
  });

  it('draws several agents as a broken ring that grows with their number',()=>{
    const ring=count=>{const {painter,calls}=gpuPainter();drawAgentMarks(painter,{x:0,y:0,radius:4,count});return calls[0];};
    const two=ring(2),eight=ring(8);
    for(const call of [two,eight]){
      expect(call.op).toBe('stroke');
      // Every other side of the polygon: no segment starts where the one before it ends.
      expect(call.path.every((p,i)=>i===0||Math.hypot(p.a[0]-call.path[i-1].b[0],p.a[1]-call.path[i-1].b[1])>1)).toBe(true);
    }
    const radiusOf=call=>Math.hypot(...call.path[0].a);
    expect(radiusOf(two)).toBeCloseTo(4+5+3.2);
    expect(radiusOf(eight)).toBeGreaterThan(radiusOf(two));
    expect(eight.path.length).toBeGreaterThan(two.path.length);
  });

  it('names the roles near enough, as an image, and never otherwise',()=>{
    const images=[];
    const textImage=(text,dpr)=>{const image={width:60*dpr,height:16*dpr,text};images.push(image);return image;};
    const far=gpuPainter();
    drawAgentMarks(far.painter,{x:0,y:0,radius:4,count:3,roles:['maker','reader'],near:false,textImage});
    expect(far.calls.map(c=>c.op)).toEqual(['stroke']);
    const near=gpuPainter();
    drawAgentMarks(near.painter,{x:0,y:0,radius:4,count:3,roles:['maker','reader','scout','keeper'],near:true,dpr:2,textImage});
    expect(near.calls.map(c=>c.op)).toEqual(['stroke','image']);
    expect(images[0].text).toBe('maker · reader · scout …');
    expect(near.calls[1]).toMatchObject({x:16,y:5,w:60,h:16});
  });

  it('draws nothing where no agent works',()=>{
    const {painter,calls}=gpuPainter();
    drawAgentMarks(painter,{x:0,y:0,radius:4,count:0});
    expect(calls).toEqual([]);
  });
});

describe('role lines',()=>{
  it('keeps the three most active and marks more',()=>{
    expect(agentRolesLine(['a','b'])).toBe('a · b');
    expect(agentRolesLine(['a','b','c','d'])).toBe('a · b · c …');
    expect(agentRolesLine([])).toBe('');
  });

  it('paints each line once per pixel ratio',()=>{
    const made=[];
    const makeCanvas=()=>{
      const ops=[];
      const context={set font(v){ops.push(['font',v]);},measureText:text=>({width:text.length*6}),setTransform:(...m)=>ops.push(['transform',...m]),set textBaseline(v){},set fillStyle(v){ops.push(['fill',v]);},fillText:(...a)=>ops.push(['text',...a])};
      const canvas={width:0,height:0,ops,getContext:()=>context};
      made.push(canvas);return canvas;
    };
    const images=agentTextImages(makeCanvas);
    const first=images('maker · reader',2);
    expect(images('maker · reader',2)).toBe(first);
    expect(images('maker · reader',1)).not.toBe(first);
    expect(made).toHaveLength(2);
    expect(first.width).toBe(Math.ceil(('maker · reader'.length*6+2)*2));expect(first.height).toBe(32);
    expect(first.ops).toContainEqual(['text','maker · reader',1,8]);
    expect(first.ops).toContainEqual(['fill',AGENT_COLOR]);
  });
});
