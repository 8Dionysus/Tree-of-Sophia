// An aspect (ракурс) turns the space toward what the reader has chosen. A chosen star comes forward with the stars it
// links to standing in rows beside it: what leads into it on the left, what it leads to on the right, each link's verb
// in its own row next to the star it reaches. A side with more links than a glance holds shows its families of links
// instead, one row each with its stars gathered at its end; the family the reader opens stands in rows of its own.
// Where the free area is narrow, both sides stand in one column, what leads in above what leads out, each under its name. A chosen link puts
// its two stars side by side with its verb between them, each star's other links on its own side, and what both share
// below. The rest of the space steps back into depth and keeps its shape there. Like depth, an aspect is a view:
// positions are never written into the graph, and letting the choice go brings the space back as it was.

// Units of the layout: a row holds one link's name; a link that names its kind as well takes a row and a half.
export const ASPECT={row:44,tagged:1.6,header:.8,limit:10,narrowLimit:7,near:310,narrowNear:350,gap:22,forward:240,back:-640,pair:220,shared:80};

const verb=edge=>String(edge.label??'').toLocaleLowerCase();
/** A link's family: the registry family a core link was given, else the kind of the link. */
export function familyOf(edge){
  const family=edge.coreRecord?.relation_family;
  return family?.family?{key:`family:${family.family}`,label:family.family_label??family.family}:{key:`kind:${edge.kind??'relates'}`,label:edge.tag||edge.kind||''};
}

/** The ids of an aspect's focus: the chosen link's two stars, or the chosen star. */
export function aspectFocus(view,{nodeId=null,edgeId=null}={}){
  const has=new Set(view.nodes.map(node=>node.id));
  if(edgeId){const edge=view.edges.find(item=>item.id===edgeId);if(edge&&has.has(edge.from)&&has.has(edge.to)&&edge.from!==edge.to)return {kind:'edge',edge,ids:[edge.from,edge.to]};}
  if(nodeId&&has.has(nodeId))return {kind:'node',ids:[nodeId]};
  return null;
}

/**
 * The view turned toward its focus: the aspect's stars with their places and the side their names take, each named
 * link with the star its verb stands beside, the families of a crowded side as headers the reader can open (`open`
 * names the open family of each side), the other stars stepped back and dimmed, the other links muted, and `aspect`
 * naming the stars brought forward so that the camera can frame them. Without a focus the view is unchanged.
 */
const OTHER={ru:'Другие связи',en:'Other links',es:'Otros vínculos',de:'Weitere Verbindungen','zh-Hans':'其他联系'};
const LEADS={in:{ru:'Ведёт сюда',en:'Leads here',es:'Lleva aquí',de:'Führt hierher','zh-Hans':'指向此处'},out:{ru:'Ведёт отсюда',en:'Leads on',es:'Lleva de aquí','de':'Führt weiter','zh-Hans':'由此引出'}};

export function aspectLayout(view,{nodeId=null,edgeId=null,open={},narrow=false,budget=Infinity,label=value=>typeof value==='string'?value:value?.ru??value?.en??''}={},shape=ASPECT){
  const focus=aspectFocus(view,{nodeId,edgeId});if(!focus)return view;
  const byId=new Map(view.nodes.map(node=>[node.id,node])),place=new Map(),sides=new Map(),named=new Map(),quiet=new Set(),headers=[];
  const front=Math.max(0,...view.nodes.map(node=>node.position?.[2]??0))+shape.forward;
  // A long verb wraps to a second line and takes the room of one; in one column the names and verbs are narrower.
  const height=edge=>shape.row*(edge.tag||String(edge.label??'').length>(narrow?24:34)?shape.tagged:1);
  const rowsOf=blocks=>blocks.reduce((sum,block)=>sum+block.height,0)/shape.row;
  const linksOf=id=>view.edges.filter(edge=>(edge.from===id||edge.to===id)&&edge.from!==edge.to&&byId.has(edge.from)&&byId.has(edge.to));
  const byOther=list=>{const groups=new Map();for(const item of [...list].sort((a,b)=>verb(a.edge).localeCompare(verb(b.edge)))){if(!groups.has(item.other))groups.set(item.other,[]);groups.get(item.other).push(item.edge);}return [...groups];};
  // The blocks of one side, top to bottom: rows of stars, or families of links with the open one in rows.
  // `gather` closes every family but the opened one: an opened family is read on its own.
  const blocksOf=(list,sideKey,limit=shape.limit,gather=false)=>{
    const blocks=[];
    const rowsBlock=entries=>blocks.push({height:entries.reduce((sum,[,links])=>sum+links.reduce((s,edge)=>s+height(edge),0),0),lay:(top,x,side)=>{
      for(const [other,links] of entries){
        if(place.has(other))continue;
        const own=links.map(edge=>height(edge)),tall=own.reduce((a,b)=>a+b,0),middle=top+tall/2;let at=top;
        place.set(other,[x,middle,front]);sides.set(other,side>0?'right':'left');
        links.forEach((edge,index)=>{named.set(edge.id,{id:other,align:side>0?'end':'start',dy:at+own[index]/2-middle});at+=own[index];});
        top+=tall;
      }
    }});
    if(list.length<=limit&&!gather){rowsBlock(byOther(list));return blocks;}
    const families=new Map();for(const item of list){const family=familyOf(item.edge);if(!families.has(family.key))families.set(family.key,{...family,items:[]});families.get(family.key).items.push(item);}
    // Gathered, the families of a single link stand together as the other links.
    if(gather){const single=[...families.values()].filter(family=>family.items.length===1&&family.key!==open?.[sideKey]);if(single.length>1){for(const family of single)families.delete(family.key);families.set('other',{key:'other',label:OTHER,items:single.flatMap(family=>family.items)});}}
    const order=[...families.values()].sort((a,b)=>(a.key==='other')-(b.key==='other')||b.items.length-a.items.length||label(a.label).localeCompare(label(b.label)));
    // The largest families close first, until the rows left open fit a glance; the family the reader opened stays open.
    const shown=open?.[sideKey],closed=new Set();let left=list.length;
    for(const family of order){if(!gather&&left<=limit)break;if(family.key===shown||(!gather&&family.items.length<2))continue;closed.add(family.key);left-=family.items.length-1;}
    const inRows=new Set(order.filter(family=>!closed.has(family.key)).flatMap(family=>family.items.map(item=>item.other)));
    const header=(family,tone)=>blocks.push({height:shape.row*shape.header,lay:(top,x,side)=>{
      const y=top+shape.row*shape.header/2;
      headers.push({id:`aspect:${sideKey}:${family.key}`,center:[x,y,front],title:`${label(family.label)} · ${family.items.length}`,variant:'facet',align:side>0?'end':'start',ref:`aspect:${sideKey}:${family.key}`,tone});
      // A closed family's stars gather at the end of its row, unnamed until pointed at.
      if(tone!=='open')for(const item of family.items)if(!place.has(item.other)&&!inRows.has(item.other)){place.set(item.other,[x,y,front]);sides.set(item.other,side>0?'right':'left');quiet.add(item.other);}
    }});
    for(const family of order)if(closed.has(family.key))header(family,'');
    // The opened family stands under its header, so that it can be closed again; the small families follow in rows.
    const opened=order.find(family=>family.key===shown&&!closed.has(family.key));
    if(opened&&closed.size){header(opened,'open');rowsBlock(byOther(opened.items));}
    const rest=order.filter(family=>!closed.has(family.key)&&!(opened&&closed.size&&family===opened)).flatMap(family=>family.items);
    if(rest.length)rowsBlock(byOther(rest));
    return blocks;
  };
  const standAt=(near,blocks,anchor,side)=>{let top=anchor[1]-blocks.reduce((sum,block)=>sum+block.height,0)/2;for(const block of blocks){block.lay(top,anchor[0]+side*near,side);top+=block.height;}};
  const stand=(blocks,anchor,side)=>standAt(narrow?shape.narrowNear:shape.near,blocks,anchor,side);
  if(focus.kind==='node'){
    const [id]=focus.ids,center=[...(byId.get(id).position??[0,0]).slice(0,2),front];place.set(id,center);sides.set(id,'above');
    // What leads into the star stands on the left, what it leads to on the right; a star linked both ways stands once,
    // on the left, with both its links; a star of an open family stands in its rows.
    const into=new Set(linksOf(id).filter(edge=>edge.to===id).map(edge=>edge.from)),before=[],after=[];
    for(const edge of linksOf(id)){const other=edge.from===id?edge.to:edge.from;(into.has(other)?before:after).push({edge,other});}
    // An opened family is read on its own: its side gathers every other family, and in one column the other side too.
    const limit=narrow?shape.narrowLimit:shape.limit,opened=key=>Boolean(open?.[key]);
    let inBlocks=blocksOf(before,'in',limit,opened('in')||(narrow&&opened('out'))),outBlocks=blocksOf(after,'out',limit,opened('out')||(narrow&&opened('in')));
    // Rows past what the free area holds close into their families.
    // In one column the side with more rows closes first, then the other, while the column is taller than the area.
    if(narrow)for(let pass=0;pass<2&&rowsOf([...inBlocks,...outBlocks])>budget;pass++){
      if(rowsOf(inBlocks)>=rowsOf(outBlocks)&&before.length>inBlocks.length)inBlocks=blocksOf(before,'in',limit,true);else outBlocks=blocksOf(after,'out',limit,true);
    }
    if(!narrow){if(rowsOf(inBlocks)>budget)inBlocks=blocksOf(before,'in',limit,true);if(rowsOf(outBlocks)>budget)outBlocks=blocksOf(after,'out',limit,true);}
    if(narrow){
      // One column right of the star: what leads in, then what leads on, each under its name when both are there.
      const heading=key=>({height:shape.row*.8,lay:(top,x)=>headers.push({id:`aspect:heading:${key}`,center:[x,top+shape.row*.45,front],title:label(LEADS[key]),variant:'facet',align:'end',tone:'heading'})});
      const both=inBlocks.length&&outBlocks.length,anchor=[center[0]-shape.narrowNear*.5,center[1]];
      standAt(shape.narrowNear,[...(both?[heading('in')]:[]),...inBlocks,...(both?[{height:shape.gap,lay:()=>{}},heading('out')]:[]),...outBlocks],anchor,1);
      place.set(id,[anchor[0],anchor[1],front]);
    }else{stand(inBlocks,center,-1);stand(outBlocks,center,1);}
  }else{
    const {edge}=focus,[a,b]=focus.ids.map(id=>byId.get(id).position??[0,0,0]),mid=[(a[0]+b[0])/2,(a[1]+b[1])/2];
    const neighbours=id=>{const out=new Map();for(const item of linksOf(id))if(item.id!==edge.id){const other=item.from===id?item.to:item.from;if(!out.has(other))out.set(other,[]);out.get(other).push(item);}return out;};
    const fromSide=neighbours(edge.from),toSide=neighbours(edge.to);
    const both=[...fromSide.keys()].filter(id=>toSide.has(id)&&id!==edge.from&&id!==edge.to);
    if(narrow){
      // In a narrow area the link stands upright: its first star above its second, its verb beside the line between them,
      // each star's other links in a column to its right; a star both share stands with the first, both links named.
      const items=(map,skip,extra=[])=>[...map].filter(([id])=>id!==skip).flatMap(([other,list])=>list.map(item=>({edge:item,other}))).concat(extra);
      const sharedTo=both.flatMap(id=>toSide.get(id).map(item=>({edge:item,other:id})));
      const upperList=items(fromSide,edge.to,sharedTo),lowerList=items(toSide,edge.from).filter(item=>!both.includes(item.other));
      let upper=blocksOf(upperList,'from',shape.narrowLimit,Boolean(open?.from||open?.to)),lower=blocksOf(lowerList,'to',shape.narrowLimit,Boolean(open?.from||open?.to));
      if(rowsOf([...upper,...lower])>budget){upper=blocksOf(upperList,'from',shape.narrowLimit,true);lower=blocksOf(lowerList,'to',shape.narrowLimit,true);}
      const tall=list=>Math.max(shape.row,list.reduce((sum,block)=>sum+block.height,0)),x=mid[0]-shape.narrowNear*.5,top=mid[1]-(tall(upper)+tall(lower)+shape.row*2)/2;
      const first=[x,top+tall(upper)/2,front],second=[x,top+tall(upper)+shape.row*2+tall(lower)/2,front];
      place.set(edge.from,first);place.set(edge.to,second);sides.set(edge.from,'left');sides.set(edge.to,'left');
      named.set(edge.id,{mid:true,align:'start'});
      stand(upper,first,1);stand(lower,second,1);
    }else{
    const left=[mid[0]-shape.pair,mid[1],front],right=[mid[0]+shape.pair,mid[1],front];
    place.set(edge.from,left);place.set(edge.to,right);sides.set(edge.from,'above');sides.set(edge.to,'above');
    named.set(edge.id,{mid:true});
    // What both stars share stands under the link: its link to the left star named on its left, to the right on its right.
    let y=mid[1]+shape.shared;
    for(const id of both){
      const links=Math.max(fromSide.get(id).length,toSide.get(id).length),tall=links*shape.row*shape.tagged;
      place.set(id,[mid[0],y+tall/2,front]);sides.set(id,'below');
      const stack=(list,align)=>list.forEach((item,index)=>named.set(item.id,{id,align,dy:(index-(list.length-1)/2)*shape.row*shape.tagged}));
      stack(fromSide.get(id),'end');stack(toSide.get(id),'start');y+=tall+shape.row;
    }
    const own=(map,skip)=>[...map].filter(([id])=>!place.has(id)&&id!==skip).flatMap(([other,items])=>items.map(item=>({edge:item,other})));
    stand(blocksOf(own(fromSide,edge.to),'from'),left,-1);stand(blocksOf(own(toSide,edge.from),'to'),right,1);
    }
  }
  const ids=[...place.keys()];
  const nodes=view.nodes.map(node=>place.has(node.id)
    ? {...node,position:place.get(node.id),labelSide:sides.get(node.id),dim:false,quiet:quiet.has(node.id),...(narrow&&!focus.ids.includes(node.id)?{labelMaxWidth:140,labelLines:2}:{})}
    : {...node,position:[node.position?.[0]??0,node.position?.[1]??0,(node.position?.[2]??0)+shape.back],dim:true,quiet:false,labelSide:null});
  const edges=view.edges.map(edge=>named.has(edge.id)?{...edge,muted:false,labelAt:named.get(edge.id),...(narrow?{labelMax:150}:{})}:{...edge,muted:true,labelAt:null});
  return {...view,nodes,edges,clusters:headers,planes:[],aspect:{kind:focus.kind,focus:focus.ids,ids}};
}
