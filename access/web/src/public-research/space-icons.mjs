// The Tree's icons share one grammar, the constellation: a thin line drawn in the colour of its text, and one to three
// stars — small filled points — where the sense of the sign gathers. The star is the signature of the whole set, kin to the
// constellation tree of the Tree's own mark; it takes the gold of the theme. Every icon stands on a 24 by 24 grid.
//
// An icon is drawn in layers. A layer is its lines (d) and its stars (s); a named layer (k) is a part of the sign that
// can move by itself when a hand comes near (space-icons.css): the bonsai sways on its root, the scales' beam tips, the
// eye blinks, the clock's hand goes round, a route draws itself from its first star to its last. A line marked `draw`
// knows its own length, so it can be drawn on; a line marked `dash` is drawn in dashes.

const L=(k,d=[],s=[],c=[])=>({k,d,s,c});
const draw=d=>({d,draw:true});

const ICONS={
  // ── Lenses: how the space is looked at.
  // A bonsai in its pot: a gently bent trunk, a branch to each side with a star at its tip, the crown's star above.
  tree:[L('sway',['M12 18.6c0-2.2-1.6-3.2-1.6-5.2c0-1.8 1.6-2.6 1.6-4.8','M10.5 14.4c-1.6 0-3.2-.8-4.2-2.2','M11.6 11.2c1.6 0 3.4-.6 4.6-1.8'],[[5.8,11.6,1.45],[16.8,8.9,1.45]],[L('glint',[],[[12,6.8,1.8]])]),L(null,['M8 18.6h8l-.9 2.8H8.9z'])],
  // Meanings: a constellation of concepts, drawn out from the one they gather round.
  meaning:[L('draw',[draw('M14.6 12.2L8.8 9.4L4.8 16.6'),draw('M14.6 12.2L19.4 5.4'),draw('M14.6 12.2L17.2 19.2')]),L('pop',[],[[8.8,9.4,1.45],[19.4,5.4,1.35],[17.2,19.2,1.2],[4.8,16.6,1.3]]),L('glint',[],[[14.6,12.2,1.95]])],
  // Sources: an open book; its pages part a little, its star shines.
  sources:[L('left',['M12 8c-2.4-1.6-5.3-2-8-1.2v11.8c2.7-.8 5.6-.4 8 1.2']),L('right',['M12 8c2.4-1.6 5.3-2 8-1.2v11.8c-2.7-.8-5.6-.4-8 1.2']),L(null,['M12 8v11.8']),L('glint',[],[[12,4.2,1.6]])],
  // Tensions: the scales; the beam tips one way and the other before it comes to rest.
  tensions:[L(null,['M12 8.7V20','M8.5 20h7']),L('beam',['M4.6 9.6L19.4 7.6','M4.6 9.6l-2.1 5','M4.6 9.6l2.1 5','M2.5 14.6a2.1 2.1 0 0 0 4.2 0','M19.4 7.6l-2.1 5','M19.4 7.6l2.1 5','M17.3 12.6a2.1 2.1 0 0 0 4.2 0'],[[12,8.6,1.6]])],
  // Images: an eye with a star for its pupil, and a far star it has seen; the eye blinks.
  images:[L('eye',['M2.6 12c2.5-4 5.8-6 9.4-6s6.9 2 9.4 6c-2.5 4-5.8 6-9.4 6s-6.9-2-9.4-6z','M15.3 12a3.3 3.3 0 1 1-6.6 0a3.3 3.3 0 1 1 6.6 0'],[[12,12,1.55]]),L('glint',[],[[19.6,4.4,1.1]])],
  // Time: a clock whose minute hand, tipped with a star, goes round once.
  time:[L(null,['M20.5 12a8.5 8.5 0 1 1-17 0a8.5 8.5 0 1 1 17 0','M12 12l3.6 2.2','M20.5 12h-1.4','M12 20.5v-1.4','M3.5 12h1.4'],[[12,12,1.05]]),L('hand',['M12 12V7.4'],[[12,6.2,1.55]])],
  // Space: a pin with a star in its head, over the ground of a place; it drops and settles.
  space:[L('pin',['M12 20.2c-3.6-3.5-6-6.8-6-9.8a6 6 0 0 1 12 0c0 3-2.4 6.3-6 9.8z'],[[12,10.4,1.85]]),L(null,['M7.6 21.4h8.8'])],
  // Proximity: the chosen star, a near ring, and a far one with a star on it that goes round.
  proximity:[L('ring',['M16.6 12a4.6 4.6 0 1 1-9.2 0a4.6 4.6 0 1 1 9.2 0']),L('orbit',[{d:'M20.6 12a8.6 8.6 0 1 1-17.2 0a8.6 8.6 0 1 1 17.2 0',dash:'2.2 2.6'}],[[18.08,5.92,1.35]]),L('glint',[],[[12,12,1.9]])],
  // Resonances: two stars, each sending its waves towards the other.
  history:[L(null,[],[[4.8,12,1.7],[19.2,12,1.7]]),L('wave1',['M7.1 9.2a3.6 3.6 0 0 1 0 5.6','M16.9 9.2a3.6 3.6 0 0 0 0 5.6']),L('wave2',['M9 6.9a6.6 6.6 0 0 1 0 10.2','M15 6.9a6.6 6.6 0 0 0 0 10.2'])],
  // ── Research: what the reader does.
  // Routes: a way through three stars, drawn from the first to the last.
  routes:[L('draw',[draw('M4.6 18.8C9.4 18.8 8 12 12 12s2.6-6.4 7.4-6.4')]),L('pop',[],[[4.6,18.8,1.3],[12,12,1.4],[19.4,5.6,1.75]])],
  lensInquiry:[L('tilt',['M8.7 8.6a3.4 3.4 0 1 1 4.9 3c-1.1.6-1.6 1.4-1.6 2.6v.8'],[[12,18.4,1.55]])],
  // Growing a branch: a sprout whose leaves open and whose bud rises.
  grow:[L(null,['M12 21V10.4','M8.4 21h7.2']),L('leaves',['M12 15.8c-3.4 0-5.5-2-5.5-5c3.3 0 5.5 2 5.5 5','M12 13c0-3 2-5 5.5-5c0 3-2.2 5-5.5 5']),L('bud',[],[[12,8,1.6]])],
  // My research: a telescope on its tripod, turned to a star; it lifts a little towards it.
  research:[L(null,['M11.2 13.1L8 20.6','M11.2 13.1l3.4 7.5']),L('tube',['M5.1 13.3L14.5 6.9l1.8 2.6l-9.4 6.4z']),L('glint',[],[[19.8,4,1.6]])],
  new:[L('spark',['M10 4.2c.4 3 1.6 4.4 4.6 4.8c-3 .4-4.2 1.8-4.6 4.8c-.4-3-1.6-4.4-4.6-4.8c3-.4 4.2-1.8 4.6-4.8z']),L('plus',['M17.6 14.4v6.2','M14.5 17.5h6.2'])],
  connect:[L('draw',[draw('M6 17.6Q6.8 7.2 18 6.4')]),L('pop',[],[[6,17.6,1.7],[18,6.4,1.7]])],
  compare:[L('l',['M14.6 12a5.6 5.6 0 1 1-11.2 0a5.6 5.6 0 1 1 11.2 0']),L('r',['M20.6 12a5.6 5.6 0 1 1-11.2 0a5.6 5.6 0 1 1 11.2 0']),L('glint',[],[[12,12,1.4]])],
  measureAction:[L(null,['M3.6 19.6h16.8']),L('bars',['M6 19.6v-4.2','M10 19.6v-7.2','M14 19.6v-10.4','M18 19.6v-6.2']),L('glint',[],[[14,6.4,1.6]])],
  // ── The map and the record.
  zoomIn:[L('turn',['M12 6.4v11.2','M6.4 12h11.2'])],
  zoomOut:[L('stretch',['M6.4 12h11.2'])],
  fit:[L('gather',['M4 8.6V4h4.6','M15.4 4H20v4.6','M20 15.4V20h-4.6','M8.6 20H4v-4.6']),L('glint',[],[[12,12,1.85]])],
  fullscreen:[L('ne',['M14 4h6v6','M20 4l-5.6 5.6']),L('sw',['M10 20H4v-6','M4 20l5.6-5.6'])],
  graphUndo:[L('back',['M9.2 6.8L5 11l4.2 4.2','M5.4 11h8.4a5 5 0 0 1 0 10h-2.2'])],
  graphRedo:[L('ahead',['M14.8 6.8L19 11l-4.2 4.2','M18.6 11h-8.4a5 5 0 0 0 0 10h2.2'])],
  export:[L('down',['M12 3.8v11','M7.6 10.6L12 15l4.4-4.4']),L(null,['M4.4 15.8v4h15.2v-4'])],
  import:[L('up',['M12 15V4','M7.6 8.4L12 4l4.4 4.4']),L(null,['M4.4 15.8v4h15.2v-4'])],
  // The workbench: a bench, and a star laid on it.
  bench:[L(null,['M3 12.6h18','M5.4 12.6v7.6','M18.6 12.6v7.6','M5.4 16.8h13.2']),L('land',[],[[12,9.4,1.9]])],
  // The card spread over the whole space, and folded back to the sky: its corners part, or close in.
  benchOpen:[L('tl',['M9 4H4v5']),L('tr',['M15 4h5v5']),L('bl',['M9 20H4v-5']),L('br',['M15 20h5v-5'])],
  benchClose:[L('tl',['M4 9h5V4']),L('tr',['M20 9h-5V4']),L('bl',['M4 15h5v5']),L('br',['M20 15h-5v5'])],
  // How the space is held: a mouse, and a touch with its ring.
  controls:[L(null,['M8 4.6a4.6 4.6 0 0 1 4.6 4.6v5.2a4.6 4.6 0 0 1-9.2 0V9.2A4.6 4.6 0 0 1 8 4.6z']),L('wheel',['M8 7.4v2.4']),L('ring',['M21.8 11.6a3.4 3.4 0 1 1-6.8 0a3.4 3.4 0 1 1 6.8 0']),L('glint',[],[[18.4,11.6,1.45]])],
};
// A lens is drawn by its own sign; Antiquity's readings and differences are the same lenses as Zarathustra's meanings and tensions.
const ALIASES={lens:'tree'};

export const ICON_NAMES=Object.freeze(Object.keys(ICONS));
export const hasIcon=name=>Object.hasOwn(ICONS,ALIASES[name]??name);

const lineMarkup=item=>typeof item==='string'?`<path d="${item}"/>`:`<path d="${item.d}"${item.dash?` stroke-dasharray="${item.dash}"`:''}${item.draw?' pathLength="1"':''}/>`;
const starMarkup=([x,y,r])=>`<circle class="ps-icon-star" cx="${x}" cy="${y}" r="${r}"/>`;
function layerMarkup(layer){
  const inner=layer.d.map(lineMarkup).join('')+layer.s.map(starMarkup).join('')+layer.c.map(layerMarkup).join('');
  return layer.k?`<g class="ps-i-${layer.k}">${inner}</g>`:inner;
}

/** The icon as SVG markup in the constellation grammar, or '' for a name it does not know. */
export function iconMarkup(name,size=20){
  const icon=ICONS[ALIASES[name]??name];if(!icon)return '';
  return `<svg class="ps-icon" data-icon="${ALIASES[name]??name}" viewBox="0 0 24 24" width="${size}" height="${size}" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">${icon.map(layerMarkup).join('')}</svg>`;
}

/** The icon as an element inside a span of the given class, or null. */
export function iconElement(doc,name,className='ps-icon-wrap',size=20){
  const markup=iconMarkup(name,size);if(!markup||!doc)return null;
  const node=doc.createElement('span');node.className=className;node.setAttribute('aria-hidden','true');node.innerHTML=markup;return node;
}
