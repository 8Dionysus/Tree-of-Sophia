// The page's theme: kept on the document's root (public/theme.js sets it before the first paint, from
// the reader's saved choice or else the system's light or dark), changed here, and heard by whatever paints colours from code.
import {THEMES,THEME_ATTRIBUTE,THEME_KEY,DEFAULT_THEME,SYSTEM_NAME,SYSTEM_THEME,systemTheme,themeById,themeColor} from './themes.mjs';
import './site-theme.css';

export {THEMES,SYSTEM_NAME,SYSTEM_THEME};
const darkQuery=typeof matchMedia==='function'?matchMedia('(prefers-color-scheme: dark)'):null;
/** The reader's choice: a theme of the site, or the system's. */
export const themeChoice=()=>{try{const id=localStorage.getItem(THEME_KEY);return themeById(id)?id:SYSTEM_THEME;}catch{return SYSTEM_THEME;}};
export const currentTheme=()=>themeById(document.documentElement.getAttribute(THEME_ATTRIBUTE))?.id??DEFAULT_THEME;

/** Show the page in the theme `id`, or in the system's with `system`; the reader's choice is kept for every page of the site. */
export function applyTheme(id,{save=true}={}){
  const follow=!themeById(id),theme=themeById(follow?systemTheme(darkQuery?.matches??true):id)??themeById(DEFAULT_THEME),root=document.documentElement;
  root.setAttribute(THEME_ATTRIBUTE,theme.id);root.style.colorScheme=theme.dark?'dark':'light';
  for(const meta of document.querySelectorAll('meta[name="theme-color"]')){meta.dataset.night??=meta.content;meta.content=theme.dark?meta.dataset.night:theme.palette.ground;}
  if(save)try{if(follow)localStorage.removeItem(THEME_KEY);else localStorage.setItem(THEME_KEY,theme.id);}catch{}
  window.dispatchEvent(new CustomEvent('site-theme',{detail:{theme:theme.id}}));
  return theme.id;
}

// While the reader follows the system, the page turns light or dark with it.
darkQuery?.addEventListener?.('change',()=>{if(themeChoice()===SYSTEM_THEME)applyTheme(SYSTEM_THEME,{save:false});});

/** A colour the night's code paints into the page, in the page's theme. */
export const paint=(color,options)=>themeColor(currentTheme(),color,options);
