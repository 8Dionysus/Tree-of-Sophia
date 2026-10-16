// Display coordinates belong to the browser; they assert no tree relationship.
const slots=[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]];
export function lensPosition(id,slot){
  let hash=2166136261;
  for(const char of id)hash=Math.imul(hash^char.codePointAt(0),16777619)>>>0;
  const angle=slot*2.399963229728653;
  return slots[slot]?.slice()||[
    Math.cos(angle)*(260+(slot%4)*58),
    Math.sin(angle)*(160+(slot%3)*47),
    -480+(hash%740),
  ];
}
