// The bonsai of the site's choice: each stand's own tree, drawn by hand (public/trees/<id>.svg), the same tree the
// sky between pages shows at its heart. A stand with no tree of its own yet shows the Tree's mark.

// The trees are served with the build's own files, under its base.
const ASSETS=import.meta.env.BASE_URL;

/** The tree of the stand `id`, the `index`th card; home.css grows it in. */
export function bonsaiTree(id,index){
  const holder=document.createElement('span');
  holder.className='home-tree';holder.style.setProperty('--i',index);
  fetch(`${ASSETS}trees/${id}.svg`)
    .then(response=>{if(!response.ok)throw new Error(String(response.status));return response.text();})
    .then(text=>{
      const tree=new DOMParser().parseFromString(text,'image/svg+xml').documentElement;
      if(tree.localName!=='svg')throw new Error('no tree');
      tree.setAttribute('aria-hidden','true');tree.setAttribute('focusable','false');
      // The pads come in one after another, and each star keeps its own time in the twinkle.
      [...tree.querySelectorAll('.tree-pad')].forEach((pad,at)=>pad.style.setProperty('--p',at));
      [...tree.querySelectorAll('.tree-star')].forEach((star,at)=>star.style.setProperty('--t',((at*.37)%1*3).toFixed(2)));
      holder.append(document.importNode(tree,true));
    })
    .catch(()=>{const mark=document.createElement('img');mark.src=`${ASSETS}tree-mark.svg`;mark.alt='';holder.classList.add('is-mark');holder.append(mark);});
  return holder;
}
