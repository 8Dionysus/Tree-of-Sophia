// Locate the recorded opening and closing words at complete sentence boundaries.
export function formulaSpan(text,formula){
  if(typeof formula.from!=='string'||!formula.from.trim()||typeof formula.to!=='string'||!formula.to.trim())throw new Error('the formula needs opening and closing words');
  const start=text.indexOf(formula.from),close=start<0?-1:text.indexOf(formula.to,start);
  if(start<0||close<0)throw new Error(`the words of the formula are not in ${formula.unit}`);
  if(text.indexOf(formula.from,start+1)>=0&&text.indexOf(formula.from,start+1)<close)throw new Error('the opening words occur twice before the closing ones');
  const end=close+formula.to.length,before=text.slice(0,start).trimEnd(),last=text.slice(0,end).trimEnd().slice(-1);
  if(end<start+formula.from.length)throw new Error('the closing words end before the opening words');
  if(before&&!/[.;!?]/u.test(before.slice(-1)))throw new Error(`the formula opens inside a sentence (${JSON.stringify(before.slice(-20))})`);
  if(!/[.;!?]/u.test(last))throw new Error('the formula closes inside a sentence');
  return {s:Array.from(text.slice(0,start)).length,e:Array.from(text.slice(0,end)).length};
}
