// The bonsai's sentence alignment is a proposed navigation aid, not an
// equivalence claim. Every returned target is checked against its reader unit.
import {createReference} from './model.mjs';
import witness from '../../../../bonsai/zarathustra/witnesses.json';

const files=import.meta.glob('../../../../bonsai/zarathustra/witnesses/*.json',{import:'default'});
const cps=(text,offset)=>Array.from(text.slice(0,offset)).length;

export async function bonsaiCorrespondences({document,version,reference,form,readTarget}){
  if(version.id!==witness.corpus.dta?.versionId)return [];
  const position=version.units.findIndex(unit=>unit.id===reference.unitId);
  if(position<0)return [];
  const order=new Map(version.units.map((unit,index)=>[unit.id,index]));
  const chapter=[...(version.structure??[])].reverse().find(item=>(order.get(item.unitId)??Infinity)<=position);
  const key=chapter?.id?.replace(/^bonsai\.de\./,'');
  const path=Object.keys(files).find(value=>value.endsWith(`/${key}.json`));
  if(!path)return [];
  const data=await files[path](),sourceIndex=data.units.de.findIndex(unit=>unit[0]===reference.unitId);
  if(sourceIndex<0)return [];
  const sourceRaw=data.units.de[sourceIndex][2],{start,end}=reference.selector;
  if(Array.from(sourceRaw).slice(start,end).join('')!==form)return [];
  const output=[],seen=new Set();
  for(const row of data.rows){
    if(!row.de?.includes(sourceIndex))continue;
    for(const lang of ['en','ru']){
      const targetVersion=document.versions.find(item=>item.id===witness.corpus[lang]?.versionId);
      if(!targetVersion)continue;
      for(const [left,right] of row.sentences?.[lang]??[]){
        if(!left.some(([index,a,b])=>index===sourceIndex&&cps(sourceRaw,a)<end&&cps(sourceRaw,b)>start))continue;
        for(const [index,a,b] of right){
          const saved=data.units[lang]?.[index];if(!saved)continue;
          const [unitId,,raw]=saved,first=cps(raw,a),last=cps(raw,b),identity=`${targetVersion.id}:${unitId}:${first}:${last}`;
          if(seen.has(identity)||first>=last)continue;
          const actual=await readTarget(targetVersion,unitId);
          // Gutenberg's units can differ from the retained alignment; never
          // silently present an offset from a different text representation.
          if(actual!==raw)continue;
          seen.add(identity);
          output.push({status:'proposed',reviewStatus:'model_proposal_owner_review_pending',sourceReference:reference,sourceQuote:form,
            targetReference:createReference({source:targetVersion.source,versionId:targetVersion.id,unitId,start:first,end:last}),
            targetQuote:Array.from(raw).slice(first,last).join(''),targetVersionId:targetVersion.id,targetVersionLabel:targetVersion.label,
            targetLanguage:lang,reviewBasis:'bonsai/lib/align.mjs sentence alignment; automatic, not human reviewed.'});
          if(output.length>=8)return output;
        }
      }
    }
  }
  return output;
}
