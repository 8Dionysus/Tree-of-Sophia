import {describe,it,expect} from 'vitest';
import {sameSceneView} from './scene-history.mjs';

const packet={source_revision:'a'.repeat(64),fingerprint:'b'.repeat(64)};
const view=()=>({graph:{packet,vertices:[{id:'opaque:1',pos:[1,2,3],target:[1,2,3]}],selectedRelation:null},selected:0,panelOpen:true,
  lens:'constellations',targets:[[1,2,3]],yaw:0,pitch:-.055,zoom:1.08,pan:{x:20,y:0},windowPosition:{x:40,y:190,manual:false},cardTab:'about'});
describe('scene history continuity',()=>{
  it('deduplicates an unchanged view without reading the response payload',()=>{
    const opaque={toJSON(){throw new Error('History must not walk source payloads');}},left=view(),right=view();
    left.graph.packet=right.graph.packet=opaque;
    expect(sameSceneView(left,right)).toBe(true);
  });
  it('preserves distinct response packets and the first entry',()=>{
    const left=view(),right=view();right.graph.packet={...packet};
    expect(sameSceneView(left,right)).toBe(false);
    expect(sameSceneView(left,undefined)).toBe(false);
  });
  it('retains every change needed to restore navigation and reading',()=>{
    const changes=[v=>v.yaw=.3,v=>v.pitch=.2,v=>v.zoom=1.3,v=>v.pan.x=0,v=>v.selected=-1,v=>v.panelOpen=false,
      v=>v.lens='plane',v=>v.targets[0][2]=4,v=>v.graph.vertices[0].pos[0]=2,v=>v.graph.selectedRelation='relation:1',
      v=>v.windowPosition.manual=true,v=>v.cardTab='relations'];
    for(const change of changes){const next=view();change(next);expect(sameSceneView(view(),next)).toBe(false);}
    expect(sameSceneView(view(),view())).toBe(true);
  });
});
