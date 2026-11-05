// Before the first paint: the theme the reader chose on any page of the site, else the system's light or dark
// (src/public-research/site-theme.mjs).
(function(){try{var saved=null;try{saved=localStorage.getItem('tos.public-research.theme.v1');}catch(error){}var theme=/^(midnight|noon)$/.test(saved||'')?saved:(typeof matchMedia!=='function'||matchMedia('(prefers-color-scheme: dark)').matches)?'midnight':'noon';document.documentElement.setAttribute('data-site-theme',theme);document.documentElement.style.colorScheme=theme==='midnight'?'dark':'light';}catch(error){}})();
