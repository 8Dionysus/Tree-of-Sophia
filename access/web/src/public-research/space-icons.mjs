// The Tree's icons share one grammar, the constellation: a thin line drawn in the colour of its text, and one to three
// stars — small filled points — where the sense of the sign gathers. The star is the signature of the whole set, kin to the
// constellation tree of the Tree's own mark; it takes the gold of the theme. Every icon stands on a 24 by 24 grid.

const ICONS={
  // ── Lenses: how the space is looked at.
  tree:{d:['M12 20.6V11.4','M12 15.2L7.2 10.4','M12 13L16.8 8.2','M12 11.4V5.6','M12 20.6c-1.3.2-2.5 1-3.5 2','M12 20.6c1.3.2 2.5 1 3.5 2'],stars:[[7.2,10.4,1.5],[16.8,8.2,1.5],[12,5,1.75]]},
  meaning:{d:['M12 12.4L5.6 7.4','M12 12.4L18.6 9.4','M12 12.4L9.8 19.4','M18.6 9.4L9.8 19.4','M5.6 7.4L12.6 4.6'],stars:[[12,12.4,1.9],[5.6,7.4,1.3],[18.6,9.4,1.3],[9.8,19.4,1.2],[12.6,4.6,1]]},
  sources:{d:['M12 8c-2.4-1.6-5.3-2-8-1.2v11.8c2.7-.8 5.6-.4 8 1.2','M12 8c2.4-1.6 5.3-2 8-1.2v11.8c-2.7-.8-5.6-.4-8 1.2','M12 8v11.8'],stars:[[12,4.2,1.6]]},
  tensions:{d:['M12 8.7V20','M8.5 20h7','M4.6 9.6L19.4 7.6','M4.6 9.6l-2.1 5','M4.6 9.6l2.1 5','M2.5 14.6a2.1 2.1 0 0 0 4.2 0','M19.4 7.6l-2.1 5','M19.4 7.6l2.1 5','M17.3 12.6a2.1 2.1 0 0 0 4.2 0'],stars:[[12,8.6,1.6]]},
  images:{d:['M3 19.5h18','M4 19.5a8 8 0 0 1 16 0','M12 19.5a8 8 0 0 0-3.2-6.4'],stars:[[9.3,11.6,1.45],[16,12.2,1.15],[5.2,15.2,1.05],[17.6,4.8,1.35]]},
  time:{d:['M20.5 12a8.5 8.5 0 1 1-17 0a8.5 8.5 0 1 1 17 0','M12 12V7','M12 12l3.6 2.2','M20.5 12h-1.4','M12 20.5v-1.4','M3.5 12h1.4'],stars:[[12,6.2,1.55],[12,12,1.05]]},
  space:{d:['M3 17.4L12 13.9L21 17.4L12 20.9Z','M9.6 17.2V5.8','M15.2 16.6V9.6'],stars:[[9.6,4.8,1.7],[15.2,8.8,1.25],[9.6,17.2,1]]},
  proximity:{d:['M16.5 12a4.5 4.5 0 1 1-9 0a4.5 4.5 0 1 1 9 0','M20.52 8.03A9.4 4 -25 1 1 3.48 15.97A9.4 4 -25 1 1 20.52 8.03'],stars:[[12,12,1.8],[15.2,8.8,1.2],[5.5,17.2,1.2]]},
  history:{d:['M5 17.5c2.5-6 11.5-6 14 0','M5 17.5c2.5-3 11.5-3 14 0'],stars:[[5,17.5,1.5],[19,17.5,1.5],[12,9.4,1.2]]},
  // ── Research: what the reader does.
  routes:{d:['M4.6 18.6C8 18.6 7.2 12 11.6 12S15.6 6 19.4 5.6'],stars:[[4.6,18.6,1.3],[11.6,12,1.5],[19.4,5.6,1.75]]},
  lensInquiry:{d:['M8.7 8.6a3.4 3.4 0 1 1 4.9 3c-1.1.6-1.6 1.4-1.6 2.6v.8'],stars:[[12,18.4,1.55]]},
  grow:{d:['M12 21V10.4','M12 15.8c-3.4 0-5.5-2-5.5-5c3.3 0 5.5 2 5.5 5','M12 13c0-3 2-5 5.5-5c0 3-2.2 5-5.5 5','M8.4 21h7.2'],stars:[[12,8,1.6]]},
  research:{d:[{d:'M20 12a8 8 0 1 1-16 0a8 8 0 1 1 16 0',dash:'2.6 2.3'}],stars:[[12,12,1.85],[15.6,8.6,1.05]]},
  new:{d:['M10 4.2c.4 3 1.6 4.4 4.6 4.8c-3 .4-4.2 1.8-4.6 4.8c-.4-3-1.6-4.4-4.6-4.8c3-.4 4.2-1.8 4.6-4.8z','M17.6 14.4v6.2','M14.5 17.5h6.2'],stars:[]},
  connect:{d:['M6 17.6Q6.8 7.2 18 6.4'],stars:[[6,17.6,1.7],[18,6.4,1.7]]},
  compare:{d:['M14.6 12a5.6 5.6 0 1 1-11.2 0a5.6 5.6 0 1 1 11.2 0','M20.6 12a5.6 5.6 0 1 1-11.2 0a5.6 5.6 0 1 1 11.2 0'],stars:[[12,12,1.4]]},
  measureAction:{d:['M3.6 19.6h16.8','M6 19.6v-4.2','M10 19.6v-7.2','M14 19.6v-10.4','M18 19.6v-6.2'],stars:[[14,6.4,1.6]]},
  // ── The map and the record.
  zoomIn:{d:['M12 6.4v11.2','M6.4 12h11.2'],stars:[]},
  zoomOut:{d:['M6.4 12h11.2'],stars:[]},
  fit:{d:['M4 8.6V4h4.6','M15.4 4H20v4.6','M20 15.4V20h-4.6','M8.6 20H4v-4.6'],stars:[[12,12,1.85]]},
  fullscreen:{d:['M4 9V4h5','M15 4h5v5','M20 15v5h-5','M9 20H4v-5','M4 4l4.4 4.4','M20 4l-4.4 4.4','M20 20l-4.4-4.4','M4 20l4.4-4.4'],stars:[]},
  graphUndo:{d:['M9.2 6.8L5 11l4.2 4.2','M5.4 11h8.4a5 5 0 0 1 0 10h-2.2'],stars:[]},
  graphRedo:{d:['M14.8 6.8L19 11l-4.2 4.2','M18.6 11h-8.4a5 5 0 0 0 0 10h2.2'],stars:[]},
  export:{d:['M12 3.8v11','M7.6 10.6L12 15l4.4-4.4','M4.4 15.8v4h15.2v-4'],stars:[]},
  import:{d:['M12 15V4','M7.6 8.4L12 4l4.4 4.4','M4.4 15.8v4h15.2v-4'],stars:[]},
};
// A lens is drawn by its own sign; Antiquity's readings and differences are the same lenses as Zarathustra's meanings and tensions.
const ALIASES={lens:'tree'};

export const ICON_NAMES=Object.freeze(Object.keys(ICONS));
export const hasIcon=name=>Object.hasOwn(ICONS,ALIASES[name]??name);

/** The icon as SVG markup in the constellation grammar, or '' for a name it does not know. */
export function iconMarkup(name,size=20){
  const icon=ICONS[ALIASES[name]??name];if(!icon)return '';
  const lines=icon.d.map(item=>typeof item==='string'?`<path d="${item}"/>`:`<path d="${item.d}" stroke-dasharray="${item.dash}"/>`).join('');
  const stars=icon.stars.map(([x,y,r])=>`<circle class="ps-icon-star" cx="${x}" cy="${y}" r="${r}"/>`).join('');
  return `<svg class="ps-icon" data-icon="${name}" viewBox="0 0 24 24" width="${size}" height="${size}" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">${lines}${stars}</svg>`;
}

/** The icon as an element inside a span of the given class, or null. */
export function iconElement(doc,name,className='ps-icon-wrap',size=20){
  const markup=iconMarkup(name,size);if(!markup||!doc)return null;
  const node=doc.createElement('span');node.className=className;node.setAttribute('aria-hidden','true');node.innerHTML=markup;return node;
}
