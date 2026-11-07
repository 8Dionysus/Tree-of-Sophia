// The tree drawn as a figure of the book: the layout of the lens "The tree" seen from in front and a little above,
// the way the space shows it at rest. The frontispiece of the contents draws it whole; an article draws it small,
// with its own star and the stars it is linked with lit. Nothing here measures or places anything: the figure
// draws the places the tree layout gives.

const SVG='http://www.w3.org/2000/svg';
const ELEVATION=.32;
const project=([x,y,z])=>[x,y*Math.cos(ELEVATION)+z*Math.sin(ELEVATION)];
const svg=(tag,attributes={})=>{const node=document.createElementNS(SVG,tag);for(const [key,value] of Object.entries(attributes))if(value!==undefined&&value!==null)node.setAttribute(key,String(value));return node;};
const round=value=>Math.round(value*10)/10;

/**
 * `layout` is a tree layout ({positions, roles, lines}); `colorOf(line)` gives a line's colour.
 * With `strata`, the names of the tree's strata stand at their heights beside it, each named by `strata(title)`.
 * Returns the figure's svg, the projected point of an id, and `light(ids, {current, links})` to light its stars.
 */
export function drawTree(layout,{width=320,label='',colorOf=line=>line.color??'#8ea2ba',via=null,className='rb-tree',strata=null}={}){
  const points=new Map();for(const [id,position] of layout.positions)points.set(id,project(position));
  const all=[...points.values(),...layout.lines.flatMap(line=>line.points.map(project))];
  const xs=all.map(point=>point[0]),ys=all.map(point=>point[1]);
  const levels=strata?(layout.names??[]).filter(name=>name.kind==='stratum'):[];
  // The names of the strata take a margin of their own at the left of the figure.
  const margin=levels.length?.27*width:0;
  const pad=24,minX=Math.min(...xs)-pad,maxX=Math.max(...xs)+pad,minY=Math.min(...ys)-pad,maxY=Math.max(...ys)+pad;
  const scale=(width-margin)/(maxX-minX),height=round((maxY-minY)*scale);
  const at=([x,y])=>[round(margin+(x-minX)*scale),round((y-minY)*scale)];
  const root=svg('svg',{class:className,viewBox:`0 0 ${width} ${height}`,role:'img','aria-label':label});
  const ground=svg('g',{class:'rb-tree-lines'}),stars=svg('g',{class:'rb-tree-stars'}),lit=svg('g',{class:'rb-tree-lit'});
  for(const line of layout.lines){
    const d=line.points.map((point,index)=>{const [x,y]=at(project(point));return `${index?'L':'M'}${x} ${y}`;}).join('');
    ground.append(svg('path',{d,stroke:colorOf(line),'data-tone':line.tone}));
  }
  const dots=new Map();
  for(const [id,point] of points){const [cx,cy]=at(point),dot=svg('circle',{cx,cy,r:1.2,'data-role':layout.roles?.get(id)??''});dot.dataset.id=id;stars.append(dot);dots.set(id,dot);}
  const names=svg('g',{class:'rb-tree-strata'});
  // Strata that stand close keep a line apart, each at its own height or just under the one above it.
  let previous=-Infinity;
  const heights=levels.map(level=>({level,y:at(project(level.center))[1]})).sort((a,b)=>a.y-b.y).map(entry=>{const y=Math.max(entry.y,previous+.045*width);previous=y;return {...entry,y:round(y)};});
  for(const {level,y} of heights){
    const text=svg('text',{x:0,y,'dominant-baseline':'middle'});
    text.textContent=strata(level.title);text.dataset.stratum=String(level.id).replace(/^tree:stratum:/,'');names.append(text);
  }
  root.append(ground,names,stars,lit);
  const pointOf=id=>points.has(id)?at(points.get(id)):null;
  // A link between strata runs through the trunk, as on the map.
  const path=(a,b)=>{
    const from=layout.positions.get(a),to=layout.positions.get(b);if(!from||!to)return null;
    const middle=via?.(from,to)??[];
    return [from,...middle,to].map((point,index)=>{const [x,y]=at(project(point));return `${index?'L':'M'}${x} ${y}`;}).join('');
  };
  function light(ids=[],{current=null,links=[]}={}){
    lit.replaceChildren();
    for(const dot of dots.values())dot.classList.remove('is-near','is-current');
    root.classList.toggle('is-lit',!!ids.length||!!current);
    for(const [a,b,color] of links){const d=path(a,b);if(d)lit.append(svg('path',{d,stroke:color??'currentColor',class:'rb-tree-link'}));}
    for(const id of ids)dots.get(id)?.classList.add('is-near');
    if(current&&points.has(current)){
      const [cx,cy]=at(points.get(current));
      lit.append(svg('circle',{cx,cy,r:9,class:'rb-tree-halo'}),svg('circle',{cx,cy,r:3.4,class:'rb-tree-current'}));
      dots.get(current)?.classList.add('is-current');
    }
  }
  /** Mark the stratum a part of the contents stands in. */
  function stratum(id){for(const text of names.children)text.classList.toggle('is-current',!!id&&text.dataset.stratum===id);}
  return {svg:root,width,height,pointOf,light,stratum,has:id=>points.has(id)};
}
