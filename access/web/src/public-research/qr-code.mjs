// A QR code for a link (ISO/IEC 18004, model 2): byte mode, UTF-8, error correction L or M, versions 1 to 40, so a
// phone can open the same place of the work. The construction follows the standard step by step; the tests check its
// modules against a reference encoder.

const ECC_PER_BLOCK={
  L:[-1,7,10,15,20,26,18,20,24,30,18,20,24,26,30,22,24,28,30,28,28,28,28,30,30,26,28,30,30,30,30,30,30,30,30,30,30,30,30,30,30],
  M:[-1,10,16,26,18,24,16,18,22,22,26,30,22,22,24,24,28,28,26,26,26,26,28,28,28,28,28,28,28,28,28,28,28,28,28,28,28,28,28,28,28],
};
const BLOCKS={
  L:[-1,1,1,1,1,1,2,2,2,2,4,4,4,4,4,6,6,6,6,7,8,8,9,9,10,12,12,12,13,14,15,16,17,18,19,19,20,21,22,24,25],
  M:[-1,1,1,1,2,2,4,4,4,5,5,5,8,9,9,10,10,11,13,14,16,17,17,18,20,21,23,25,26,28,29,31,33,35,37,38,40,43,45,47,49],
};
const FORMAT_LEVEL={L:1,M:0};

const rawModules=version=>{let result=(16*version+128)*version+64;if(version>=2){const align=Math.floor(version/7)+2;result-=(25*align-10)*align-55;if(version>=7)result-=36;}return result;};
const dataCodewords=(version,level)=>Math.floor(rawModules(version)/8)-ECC_PER_BLOCK[level][version]*BLOCKS[level][version];

function multiply(x,y){let z=0;for(let i=7;i>=0;i--){z=(z<<1)^((z>>>7)*0x11d);z^=((y>>>i)&1)*x;}return z&0xff;}
function divisor(degree){
  const result=new Array(degree).fill(0);result[degree-1]=1;let root=1;
  for(let i=0;i<degree;i++){for(let j=0;j<degree;j++){result[j]=multiply(result[j],root);if(j+1<degree)result[j]^=result[j+1];}root=multiply(root,2);}
  return result;
}
function remainder(data,by){const result=by.map(()=>0);for(const byte of data){const factor=byte^result.shift();result.push(0);by.forEach((coefficient,i)=>{result[i]^=multiply(coefficient,factor);});}return result;}

function alignmentPositions(version){
  if(version===1)return [];
  const count=Math.floor(version/7)+2,size=version*4+17,step=version===32?26:Math.ceil((version*4+4)/(count*2-2))*2,result=[6];
  for(let position=size-7;result.length<count;position-=step)result.splice(1,0,position);
  return result;
}

/** The smallest version whose capacity at `level` holds `bytes` bytes in byte mode. */
export function versionFor(bytes,level='M'){
  for(let version=1;version<=40;version++){const countBits=version<10?8:16;if(4+countBits+bytes*8<=dataCodewords(version,level)*8)return version;}
  return null;
}

/** The codewords of the data with their error correction, interleaved as the symbol carries them. */
export function codewords(text,{level='M',version=null}={}){
  const bytes=[...new TextEncoder().encode(String(text))],chosen=version??versionFor(bytes.length,level);
  if(!chosen)throw new RangeError('The text is too long for a QR code');
  const capacity=dataCodewords(chosen,level)*8,bits=[];
  const push=(value,length)=>{for(let i=length-1;i>=0;i--)bits.push((value>>>i)&1);};
  push(0b0100,4);push(bytes.length,chosen<10?8:16);for(const byte of bytes)push(byte,8);
  if(bits.length>capacity)throw new RangeError('The text does not fit this version');
  push(0,Math.min(4,capacity-bits.length));push(0,(8-bits.length%8)%8);
  for(let pad=0xec;bits.length<capacity;pad^=0xec^0x11)push(pad,8);
  const data=[];for(let i=0;i<bits.length;i+=8)data.push(bits.slice(i,i+8).reduce((value,bit)=>value<<1|bit,0));
  const blocks=BLOCKS[level][chosen],eccLength=ECC_PER_BLOCK[level][chosen],raw=Math.floor(rawModules(chosen)/8);
  const shortBlocks=blocks-raw%blocks,shortLength=Math.floor(raw/blocks),by=divisor(eccLength),lines=[];
  for(let i=0,k=0;i<blocks;i++){
    const own=data.slice(k,k+shortLength-eccLength+(i<shortBlocks?0:1));k+=own.length;
    const ecc=remainder(own,by);if(i<shortBlocks)own.push(0);lines.push([...own,...ecc]);
  }
  const result=[];
  for(let i=0;i<lines[0].length;i++)lines.forEach((line,j)=>{if(i!==shortLength-eccLength||j>=shortBlocks)result.push(line[i]);});
  return {version:chosen,level,codewords:result};
}

const MASKS=[
  (x,y)=>(x+y)%2===0,(x,y)=>y%2===0,x=>x%3===0,(x,y)=>(x+y)%3===0,
  (x,y)=>(Math.floor(x/3)+Math.floor(y/2))%2===0,(x,y)=>x*y%2+x*y%3===0,(x,y)=>(x*y%2+x*y%3)%2===0,(x,y)=>((x+y)%2+x*y%3)%2===0,
];

function symbol(version,level,words,mask){
  const size=version*4+17,dark=Array.from({length:size},()=>new Array(size).fill(false)),fixed=Array.from({length:size},()=>new Array(size).fill(false));
  const set=(x,y,value)=>{dark[y][x]=value;fixed[y][x]=true;};
  for(let i=0;i<size;i++){set(6,i,i%2===0);set(i,6,i%2===0);}
  for(const [cx,cy] of [[3,3],[size-4,3],[3,size-4]])for(let dy=-4;dy<=4;dy++)for(let dx=-4;dx<=4;dx++){const x=cx+dx,y=cy+dy;if(x>=0&&x<size&&y>=0&&y<size){const d=Math.max(Math.abs(dx),Math.abs(dy));set(x,y,d!==2&&d!==4);}}
  const align=alignmentPositions(version),n=align.length;
  for(let i=0;i<n;i++)for(let j=0;j<n;j++){if(i===0&&j===0||i===0&&j===n-1||i===n-1&&j===0)continue;for(let dy=-2;dy<=2;dy++)for(let dx=-2;dx<=2;dx++)set(align[i]+dx,align[j]+dy,Math.max(Math.abs(dx),Math.abs(dy))!==1);}
  const format=()=>{
    const data=FORMAT_LEVEL[level]<<3|mask;let rem=data;for(let i=0;i<10;i++)rem=(rem<<1)^((rem>>>9)*0x537);
    const bits=(data<<10|rem)^0x5412,bit=i=>((bits>>>i)&1)!==0;
    for(let i=0;i<=5;i++)set(8,i,bit(i));set(8,7,bit(6));set(8,8,bit(7));set(7,8,bit(8));for(let i=9;i<15;i++)set(14-i,8,bit(i));
    for(let i=0;i<8;i++)set(size-1-i,8,bit(i));for(let i=8;i<15;i++)set(8,size-15+i,bit(i));set(8,size-8,true);
  };
  format();
  if(version>=7){let rem=version;for(let i=0;i<12;i++)rem=(rem<<1)^((rem>>>11)*0x1f25);const bits=version<<12|rem;for(let i=0;i<18;i++){const value=((bits>>>i)&1)!==0,a=size-11+i%3,b=Math.floor(i/3);set(a,b,value);set(b,a,value);}}
  let i=0;
  for(let right=size-1;right>=1;right-=2){
    if(right===6)right=5;
    for(let vert=0;vert<size;vert++)for(let j=0;j<2;j++){
      const x=right-j,upward=((right+1)&2)===0,y=upward?size-1-vert:vert;
      if(!fixed[y][x]&&i<words.length*8){dark[y][x]=((words[i>>>3]>>>(7-(i&7)))&1)!==0;i++;}
    }
  }
  for(let y=0;y<size;y++)for(let x=0;x<size;x++)if(!fixed[y][x]&&MASKS[mask](x,y))dark[y][x]=!dark[y][x];
  return dark;
}

/** The penalty of a symbol by the standard's four rules: runs, blocks, finder-like patterns and balance. */
export function penalty(dark){
  const size=dark.length;let score=0;
  const line=get=>{let run=1,result=0;for(let i=1;i<size;i++){if(get(i)===get(i-1)){run++;if(run===5)result+=3;else if(run>5)result++;}else run=1;}return result;};
  for(let k=0;k<size;k++){score+=line(i=>dark[k][i]);score+=line(i=>dark[i][k]);}
  for(let y=0;y<size-1;y++)for(let x=0;x<size-1;x++){const c=dark[y][x];if(c===dark[y][x+1]&&c===dark[y+1][x]&&c===dark[y+1][x+1])score+=3;}
  const pattern=[true,false,true,true,true,false,true];
  const matches=(get,at)=>pattern.every((value,i)=>get(at+i)===value);
  const clear=(get,from,to)=>{for(let i=from;i<to;i++){if(i>=0&&i<size&&get(i))return false;}return true;};
  for(let k=0;k<size;k++)for(const get of [i=>dark[k][i],i=>dark[i][k]])for(let at=0;at+7<=size;at++)if(matches(get,at)&&(clear(get,at-4,at)||clear(get,at+7,at+11)))score+=40;
  const total=dark.reduce((sum,row)=>sum+row.filter(Boolean).length,0),k=Math.ceil(Math.abs(total*20-size*size*10)/(size*size))-1;
  return score+Math.max(0,k)*10;
}

/** The modules of a QR code for `text`: `size` and `dark[y][x]`; the mask is the one of least penalty unless given. */
export function qrCode(text,{level='M',version=null,mask=null}={}){
  const {version:chosen,codewords:words}=codewords(text,{level,version});
  let best=null;
  for(const candidate of mask===null?[0,1,2,3,4,5,6,7]:[mask]){const dark=symbol(chosen,level,words,candidate),score=mask===null?penalty(dark):0;if(!best||score<best.score)best={dark,score,mask:candidate};}
  return {version:chosen,level,mask:best.mask,size:best.dark.length,dark:best.dark};
}

/** An SVG drawing of a code: dark modules as one path, with the quiet zone of four modules the standard asks for. */
export function qrSvg(code,{quiet=4,label=''}={}){
  const extent=code.size+quiet*2,parts=[];
  for(let y=0;y<code.size;y++)for(let x=0;x<code.size;x++)if(code.dark[y][x])parts.push(`M${x+quiet} ${y+quiet}h1v1h-1z`);
  const title=label?`<title>${label.replace(/[<&>"]/g,c=>({'<':'&lt;','&':'&amp;','>':'&gt;','"':'&quot;'})[c])}</title>`:'';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${extent} ${extent}" shape-rendering="crispEdges" role="img">${title}<rect width="${extent}" height="${extent}" fill="#fff"/><path d="${parts.join('')}" fill="#000"/></svg>`;
}
