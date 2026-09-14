// Literal matches address the supplied strings with JavaScript UTF-16 offsets.
// They do not normalize text or create source/linguistic segmentation claims.
export function findInParagraphs(paragraphs,query,{limit=500}={}){
  if(!Array.isArray(paragraphs)||paragraphs.some(text=>typeof text!=='string')
    ||typeof query!=='string'||!Number.isSafeInteger(limit)||limit<1||limit>500)throw new Error('invalid-input');
  if(query.length>160)throw new Error('limit');
  const matches=[];
  if(!query.trim())return {matches,truncated:false};
  const pattern=new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g,'\\$&'),'giu');
  for(let paragraph=0;paragraph<paragraphs.length;paragraph++){
    pattern.lastIndex=0;
    for(let match;(match=pattern.exec(paragraphs[paragraph]));){
      if(matches.length===limit)return {matches,truncated:true};
      matches.push({paragraph,index:match.index,length:match[0].length});
    }
  }
  return {matches,truncated:false};
}
