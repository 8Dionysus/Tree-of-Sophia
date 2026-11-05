// The page's theme: kept on the document's root (public/theme.js sets it before the first paint, from
// the reader's saved choice), changed here, and heard by whatever paints colours from code.
import {THEMES,THEME_ATTRIBUTE,THEME_KEY,DEFAULT_THEME,themeById,themeColor} from './themes.mjs';
import './site-theme.css';

export {THEMES};
export const currentTheme=()=>themeById(document.documentElement.getAttribute(THEME_ATTRIBUTE))?.id??DEFAULT_THEME;

/** Show the page in the theme `id`; the reader's choice is kept for every page of the site. */
export function applyTheme(id,{save=true}={}){
  const theme=themeById(id)??themeById(DEFAULT_THEME),root=document.documentElement;
  root.setAttribute(THEME_ATTRIBUTE,theme.id);root.style.colorScheme=theme.dark?'dark':'light';
  for(const meta of document.querySelectorAll('meta[name="theme-color"]')){meta.dataset.night??=meta.content;meta.content=theme.dark?meta.dataset.night:theme.palette.ground;}
  if(save)try{localStorage.setItem(THEME_KEY,theme.id);}catch{}
  window.dispatchEvent(new CustomEvent('site-theme',{detail:{theme:theme.id}}));
  return theme.id;
}

/** A colour the night's code paints into the page, in the page's theme. */
export const paint=(color,options)=>themeColor(currentTheme(),color,options);
